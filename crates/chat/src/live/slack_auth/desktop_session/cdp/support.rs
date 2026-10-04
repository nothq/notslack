use serde::Deserialize;
use serde_json::Value;

pub(super) fn cdp_result(method: &str, payload: Value) -> Result<Value, String> {
    if let Some(error) = payload.get("error") {
        return Err(format!("Slack Desktop CDP {method} failed: {error}"));
    }
    Ok(payload.get("result").cloned().unwrap_or(Value::Null))
}

pub(super) fn binary_cdp_json(bytes: Vec<u8>) -> Result<Value, String> {
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("Slack Desktop CDP sent invalid UTF-8: {error}"))?;
    parse_cdp_json(&text)
}

pub(super) fn parse_cdp_json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text)
        .map_err(|error| format!("failed to decode Slack Desktop CDP JSON: {error}"))
}

#[derive(Deserialize)]
pub(super) struct SlackDraftCountResult {
    pub(super) count: u32,
    #[serde(default)]
    pub(super) error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SlackAuthTestResult {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) team_id: Option<String>,
}

pub(super) fn draft_count_expression(team_id: &str, user_id: Option<&str>) -> String {
    let team_id = serde_json::to_string(team_id).expect("serialize team id");
    let user_id = user_id
        .map(serde_json::to_string)
        .transpose()
        .expect("serialize user id")
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"(() => {{
  const teamId = {team_id};
  const userId = {user_id};
  const prefix = `persist-v1::${{teamId}}::`;
  const suffix = "::drafts";
  const keys = Object.keys(localStorage).filter((key) => {{
    if (!key.startsWith(prefix) || !key.endsWith(suffix)) {{
      return false;
    }}
    return userId === null || key === `${{prefix}}${{userId}}${{suffix}}`;
  }});
  if (keys.length === 0) {{
    return JSON.stringify({{ count: 0 }});
  }}
  if (keys.length > 1) {{
    return JSON.stringify({{
      count: 0,
      error: `Slack Desktop exposed multiple draft stores for team ${{teamId}}`
    }});
  }}
  const raw = localStorage.getItem(keys[0]);
  if (!raw) {{
    return JSON.stringify({{ count: 0 }});
  }}
  let parsed;
  try {{
    parsed = JSON.parse(raw);
  }} catch (_error) {{
    return JSON.stringify({{
      count: 0,
      error: `Slack Desktop draft store for team ${{teamId}} was not valid JSON`
    }});
  }}
  const drafts = parsed && parsed.unifiedDrafts;
  if (!drafts || typeof drafts !== "object" || Array.isArray(drafts)) {{
    return JSON.stringify({{
      count: 0,
      error: `Slack Desktop draft store for team ${{teamId}} did not include unifiedDrafts`
    }});
  }}
  return JSON.stringify({{ count: Object.keys(drafts).length }});
}})()"#
    )
}

pub(super) fn authenticated_local_config_timeout_message(last_auth_error: Option<&str>) -> String {
    let message = "Slack Desktop did not expose authenticated localConfig_v2 web sessions; open Slack Desktop signed in to every workspace and try again";
    match last_auth_error {
        Some(error) => format!("{message}; last auth.test error: {error}"),
        None => message.to_string(),
    }
}
