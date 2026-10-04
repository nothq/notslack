mod local_config;
mod parse;
mod support;
#[cfg(test)]
mod tests;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use self::local_config::{local_config_credentials, SidebarRequestCredentials};
use self::parse::cookie_header_pair;
use self::support::{
    authenticated_local_config_timeout_message, binary_cdp_json, cdp_result,
    draft_count_expression, parse_cdp_json, SlackAuthTestResult, SlackDraftCountResult,
};
use super::super::types::SlackWebBuildTimestamp;

const SLACK_CDP_CALL_WAIT: Duration = Duration::from_secs(10);
const SLACK_AUTH_TEST_WAIT: Duration = Duration::from_secs(8);
const SLACK_REQUEST_WAIT: Duration = Duration::from_secs(30);
const LOCAL_CONFIG_KEY: &str = "localConfig_v2";

type CdpStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub(super) struct ExtractedSlackWebSession {
    pub(super) team_id: String,
    pub(super) team_domain: String,
    pub(super) user_id: Option<String>,
    pub(super) xoxc_token: String,
    pub(super) cookie_header: String,
    pub(super) web_build_timestamp: SlackWebBuildTimestamp,
    pub(super) draft_count: u32,
}

struct CdpSocket {
    stream: CdpStream,
    next_id: u64,
}

impl CdpSocket {
    fn new(stream: CdpStream) -> Self {
        Self { stream, next_id: 1 }
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        tokio::time::timeout(SLACK_CDP_CALL_WAIT, async {
            let id = self.send(method, params).await?;
            loop {
                let payload = self.next_json().await?;
                if payload.get("id").and_then(Value::as_u64) == Some(id) {
                    return cdp_result(method, payload);
                }
            }
        })
        .await
        .map_err(|_| {
            format!(
                "Slack Desktop CDP {method} timed out after {} seconds",
                SLACK_CDP_CALL_WAIT.as_secs()
            )
        })?
    }

    async fn send(&mut self, method: &str, params: Value) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        let payload = json!({ "id": id, "method": method, "params": params });
        self.stream
            .send(Message::Text(payload.to_string()))
            .await
            .map_err(|error| format!("Slack Desktop CDP send failed: {error}"))?;
        Ok(id)
    }

    async fn next_json(&mut self) -> Result<Value, String> {
        loop {
            let Some(message) = self.stream.next().await else {
                return Err("Slack Desktop CDP connection closed".to_string());
            };
            match message.map_err(|error| format!("Slack Desktop CDP read failed: {error}"))? {
                Message::Text(text) => return parse_cdp_json(&text),
                Message::Binary(bytes) => return binary_cdp_json(bytes),
                Message::Close(_) => return Err("Slack Desktop CDP connection closed".to_string()),
                _ => {}
            }
        }
    }

    async fn wait_for_authenticated_local_config_sessions(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<SidebarRequestCredentials>, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_auth_error = None;
        loop {
            let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now())
            else {
                return Err(authenticated_local_config_timeout_message(
                    last_auth_error.as_deref(),
                ));
            };
            let local_config =
                match tokio::time::timeout(remaining, self.local_config_sessions()).await {
                    Ok(Ok(local_config)) => local_config,
                    Ok(Err(error)) => {
                        last_auth_error = Some(error);
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        continue;
                    }
                    Err(_) => {
                        return Err(authenticated_local_config_timeout_message(
                            last_auth_error.as_deref(),
                        ));
                    }
                };
            if let Some(sessions) = local_config {
                if self
                    .sessions_are_authenticated(&sessions, deadline, &mut last_auth_error)
                    .await?
                {
                    return Ok(sessions);
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(authenticated_local_config_timeout_message(
                    last_auth_error.as_deref(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn sessions_are_authenticated(
        &mut self,
        sessions: &[SidebarRequestCredentials],
        deadline: tokio::time::Instant,
        last_auth_error: &mut Option<String>,
    ) -> Result<bool, String> {
        for request in sessions {
            let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now())
            else {
                return Err(authenticated_local_config_timeout_message(
                    last_auth_error.as_deref(),
                ));
            };
            let auth_result =
                tokio::time::timeout(remaining, self.validate_authenticated_session(request))
                    .await
                    .map_err(|_| {
                        authenticated_local_config_timeout_message(last_auth_error.as_deref())
                    })?;
            if let Err(error) = auth_result {
                *last_auth_error = Some(format!("team {}: {error}", request.team_id));
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn local_config_sessions(
        &mut self,
    ) -> Result<Option<Vec<SidebarRequestCredentials>>, String> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": format!("localStorage.getItem('{LOCAL_CONFIG_KEY}')"),
                    "returnByValue": true,
                }),
            )
            .await?;
        let Some(raw) = result.pointer("/result/value").and_then(Value::as_str) else {
            return Ok(None);
        };
        let sessions = local_config_credentials(raw)?;
        if sessions.is_empty() {
            return Ok(None);
        }
        Ok(Some(sessions))
    }

    async fn cookie_header(&mut self, team_domain: &str) -> Result<String, String> {
        let urls = vec![
            format!("https://{team_domain}.slack.com"),
            "https://app.slack.com".to_string(),
        ];
        let result = self
            .call("Network.getCookies", json!({ "urls": urls }))
            .await?;
        let cookies = result
            .get("cookies")
            .and_then(Value::as_array)
            .ok_or_else(|| "Slack Desktop did not return cookies".to_string())?;
        let cookie_header = cookies
            .iter()
            .filter_map(cookie_header_pair)
            .collect::<Vec<_>>()
            .join("; ");
        if cookie_header.is_empty() {
            return Err("Slack Desktop did not expose Slack cookies".to_string());
        }
        Ok(cookie_header)
    }

    async fn web_build_timestamp(&mut self) -> Result<SlackWebBuildTimestamp, String> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": "document.documentElement.dataset.versionTs",
                    "returnByValue": true,
                }),
            )
            .await?;
        let raw = result
            .pointer("/result/value")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "Slack Desktop authenticated document did not expose its web build timestamp"
                    .to_string()
            })?;
        SlackWebBuildTimestamp::parse(raw.to_string())
    }

    async fn validate_authenticated_session(
        &mut self,
        request: &SidebarRequestCredentials,
    ) -> Result<(), String> {
        let expression = auth_test_expression(&request.xoxc_token)?;
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "awaitPromise": true,
                    "returnByValue": true,
                }),
            )
            .await?;
        let raw = result
            .pointer("/result/value")
            .and_then(Value::as_str)
            .ok_or_else(|| "Slack Desktop did not return auth.test state".to_string())?;
        let state = serde_json::from_str::<SlackAuthTestResult>(raw)
            .map_err(|error| format!("failed to decode Slack Desktop auth.test state: {error}"))?;
        if !state.ok {
            return Err(state
                .error
                .unwrap_or_else(|| "Slack auth.test rejected the captured session".to_string()));
        }
        if state.team_id.as_deref() != Some(request.team_id.as_str()) {
            return Err(format!(
                "Slack auth.test returned team {} for requested team {}",
                state.team_id.as_deref().unwrap_or("unknown"),
                request.team_id
            ));
        }
        Ok(())
    }

    async fn draft_count(&mut self, team_id: &str, user_id: Option<&str>) -> Result<u32, String> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": draft_count_expression(team_id, user_id),
                    "returnByValue": true,
                }),
            )
            .await?;
        let Some(raw) = result.pointer("/result/value").and_then(Value::as_str) else {
            return Err("Slack Desktop did not return draft sidebar state".to_string());
        };
        let state = serde_json::from_str::<SlackDraftCountResult>(raw)
            .map_err(|error| format!("failed to decode Slack Desktop draft state: {error}"))?;
        if let Some(error) = state.error {
            return Err(error);
        }
        Ok(state.count)
    }
}

fn auth_test_expression(xoxc_token: &str) -> Result<String, String> {
    let token = serde_json::to_string(xoxc_token)
        .map_err(|error| format!("failed to serialize Slack client token: {error}"))?;
    let timeout_ms = SLACK_AUTH_TEST_WAIT.as_millis();
    Ok(format!(
        r#"void 0; (async () => {{
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), {timeout_ms});
  try {{
    const response = await fetch("/api/auth.test", {{
      method: "POST",
      credentials: "include",
      signal: controller.signal,
      headers: {{ "content-type": "application/x-www-form-urlencoded" }},
      body: new URLSearchParams({{ token: {token} }}).toString()
    }});
    const payload = await response.json();
    return JSON.stringify({{
      ok: payload.ok === true,
      error: typeof payload.error === "string" ? payload.error : null,
      team_id: typeof payload.team_id === "string" ? payload.team_id : null
    }});
  }} catch (error) {{
    return JSON.stringify({{ ok: false, error: String(error), team_id: null }});
  }} finally {{
    clearTimeout(timeoutId);
  }}
}})()"#
    ))
}

pub(super) async fn capture_slack_web_sessions(
    web_socket_url: String,
) -> Result<Vec<ExtractedSlackWebSession>, String> {
    let (stream, _) =
        tokio::time::timeout(SLACK_CDP_CALL_WAIT, connect_async(web_socket_url.as_str()))
            .await
            .map_err(|_| {
                format!(
                    "Slack Desktop debugger connection timed out after {} seconds",
                    SLACK_CDP_CALL_WAIT.as_secs()
                )
            })?
            .map_err(|error| format!("failed to connect to Slack Desktop debugger: {error}"))?;
    let mut cdp = CdpSocket::new(stream);
    cdp.call("Network.enable", json!({})).await?;
    cdp.call("Runtime.enable", json!({})).await?;
    let requests = cdp
        .wait_for_authenticated_local_config_sessions(SLACK_REQUEST_WAIT)
        .await?;
    let web_build_timestamp = cdp.web_build_timestamp().await?;
    let mut sessions = Vec::with_capacity(requests.len());
    for request in requests {
        let cookie_header = cdp.cookie_header(&request.team_domain).await?;
        let draft_count = cdp
            .draft_count(&request.team_id, request.user_id.as_deref())
            .await?;
        sessions.push(ExtractedSlackWebSession {
            team_id: request.team_id,
            team_domain: request.team_domain,
            user_id: request.user_id,
            xoxc_token: request.xoxc_token,
            cookie_header,
            web_build_timestamp: web_build_timestamp.clone(),
            draft_count,
        });
    }
    Ok(sessions)
}
