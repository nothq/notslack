use std::{
    collections::BTreeMap,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use reqwest::blocking::Client;
use serde::Deserialize;

use super::types::{
    StoredSlackDesktopSession, StoredSlackDesktopSessionRecord, StoredSlackDesktopSessions,
};
use super::SlackDesktopIntegrationUnavailable;
use crate::live::{SlackApiClient, SlackWebSessionCredentials};

#[cfg(target_os = "linux")]
mod browser;
mod cdp;
mod desktop_app;
mod desktop_state;

use desktop_app::{installed_slack_app_path, relaunch_slack_with_debugging, SlackDesktopAppError};

const SLACK_BROWSER_WAIT: Duration = Duration::from_secs(60);
/// Long enough to sign in to Slack by hand in the browser notslack opened.
#[cfg(target_os = "linux")]
const SLACK_BROWSER_SIGN_IN_WAIT: Duration = Duration::from_secs(300);

pub(crate) fn capture_slack_desktop_sessions() -> Result<StoredSlackDesktopSessions, String> {
    #[cfg(target_os = "linux")]
    if let Some(browser) = fallback_browser() {
        return capture_slack_browser_sessions(&browser);
    }
    let directory = desktop_state::load_slack_desktop_workspace_directory()?;
    let extracted_sessions = SlackDesktopSessionExtractor::new()?.extract_all()?;
    let mut extracted_by_team_id = extracted_sessions
        .into_iter()
        .map(|session| (session.team_id.clone(), session))
        .collect::<BTreeMap<_, _>>();
    let mut ordered_team_ids = Vec::with_capacity(extracted_by_team_id.len());
    let mut sessions_by_team_id = BTreeMap::new();

    for identity in directory.ordered {
        let Some(session) = extracted_by_team_id.remove(&identity.team_id) else {
            continue;
        };
        if session.team_domain != identity.team_domain {
            return Err(format!(
                "Slack Desktop metadata domain {} did not match authenticated domain {} for team {}",
                identity.team_domain, session.team_domain, identity.team_id
            ));
        }
        validate_captured_session(&session)?;
        ordered_team_ids.push(identity.team_id.clone());
        sessions_by_team_id.insert(
            identity.team_id,
            StoredSlackDesktopSessionRecord {
                workspace_name: identity.workspace_name,
                workspace_logo_url: identity.workspace_logo_url,
                notification_playback: identity.notification_playback,
                notification_sound: identity.notification_sound,
                session: StoredSlackDesktopSession {
                    team_id: session.team_id,
                    team_domain: session.team_domain,
                    user_id: session.user_id,
                    xoxc_token: session.xoxc_token,
                    cookie_header: session.cookie_header,
                    web_build_timestamp: session.web_build_timestamp,
                    draft_count: Some(session.draft_count),
                },
            },
        );
    }

    if !extracted_by_team_id.is_empty() {
        return Err(format!(
            "Slack Desktop authenticated workspace metadata was missing teams {}",
            extracted_by_team_id
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let selected_team_id = directory
        .selected_team_id
        .filter(|team_id| sessions_by_team_id.contains_key(team_id));
    StoredSlackDesktopSessions::new(ordered_team_ids, selected_team_id, sessions_by_team_id)
}

/// The browser to sign in with when Slack Desktop is not installed.
#[cfg(target_os = "linux")]
fn fallback_browser() -> Option<std::path::PathBuf> {
    match installed_slack_app_path() {
        Err(SlackDesktopAppError::NotInstalled) => browser::installed_browser(),
        Ok(_) | Err(_) => None,
    }
}

/// Looked up once: the connect screen asks on every frame.
pub(super) fn connects_through_browser() -> bool {
    #[cfg(target_os = "linux")]
    {
        static THROUGH_BROWSER: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *THROUGH_BROWSER.get_or_init(|| fallback_browser().is_some())
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Slack Desktop keeps the workspace names, logos and order on disk; a browser
/// does not, so Slack is asked for them and the workspaces sort by team id.
#[cfg(target_os = "linux")]
fn capture_slack_browser_sessions(
    browser: &std::path::Path,
) -> Result<StoredSlackDesktopSessions, String> {
    let extracted_sessions = SlackDesktopSessionExtractor::new()?.extract_from_browser(browser)?;
    let mut ordered_team_ids = Vec::with_capacity(extracted_sessions.len());
    let mut sessions_by_team_id = BTreeMap::new();
    for session in extracted_sessions {
        let (workspace_name, workspace_logo_url) = captured_workspace_identity(&session)?;
        ordered_team_ids.push(session.team_id.clone());
        sessions_by_team_id.insert(
            session.team_id.clone(),
            StoredSlackDesktopSessionRecord {
                workspace_name,
                workspace_logo_url,
                notification_playback: None,
                notification_sound: None,
                session: StoredSlackDesktopSession {
                    team_id: session.team_id,
                    team_domain: session.team_domain,
                    user_id: session.user_id,
                    xoxc_token: session.xoxc_token,
                    cookie_header: session.cookie_header,
                    web_build_timestamp: session.web_build_timestamp,
                    draft_count: Some(session.draft_count),
                },
            },
        );
    }
    ordered_team_ids.sort();
    StoredSlackDesktopSessions::new(ordered_team_ids, None, sessions_by_team_id)
}

/// The workspace name from auth.test and, when Slack returns one, its logo.
#[cfg(target_os = "linux")]
fn captured_workspace_identity(
    session: &cdp::ExtractedSlackWebSession,
) -> Result<(String, Option<String>), String> {
    let (client, auth) = confirm_captured_session(session)?;
    let workspace_name = auth
        .get("team")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&session.team_domain)
        .to_string();
    let workspace_logo_url = client.post("team.info", &[]).ok().and_then(|info| {
        info.pointer("/team/icon/image_88")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });
    Ok((workspace_name, workspace_logo_url))
}

pub(super) fn check_slack_desktop_app() -> Result<(), SlackDesktopIntegrationUnavailable> {
    match installed_slack_app_path() {
        Ok(_) => Ok(()),
        #[cfg(target_os = "linux")]
        Err(SlackDesktopAppError::NotInstalled) if browser::installed_browser().is_some() => Ok(()),
        Err(SlackDesktopAppError::NotInstalled) => {
            Err(SlackDesktopIntegrationUnavailable::NativeAppNotInstalled)
        }
        Err(error) => Err(SlackDesktopIntegrationUnavailable::RuntimeError(
            error.to_string(),
        )),
    }
}

fn validate_captured_session(session: &cdp::ExtractedSlackWebSession) -> Result<(), String> {
    confirm_captured_session(session).map(drop)
}

/// Confirms the captured session with auth.test, returning the client and
/// Slack's answer.
fn confirm_captured_session(
    session: &cdp::ExtractedSlackWebSession,
) -> Result<(SlackApiClient, serde_json::Value), String> {
    let credentials = SlackWebSessionCredentials::new(
        session.xoxc_token.clone(),
        session.cookie_header.clone(),
        session.web_build_timestamp.clone(),
    )?;
    let client = SlackApiClient::desktop(&credentials)?;
    let auth = client.post("auth.test", &[])?;
    if auth.get("team_id").and_then(serde_json::Value::as_str) != Some(session.team_id.as_str()) {
        return Err(format!(
            "Slack auth.test did not confirm captured team {}",
            session.team_id
        ));
    }
    Ok((client, auth))
}

struct SlackDesktopSessionExtractor {
    client: Client,
    port: u16,
}

#[derive(Clone, Debug, Deserialize)]
struct CdpTarget {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "webSocketDebuggerUrl")]
    web_socket_url: Option<String>,
}

impl SlackDesktopSessionExtractor {
    fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| format!("failed to build Slack Desktop inspector client: {error}"))?;
        Ok(Self {
            client,
            port: available_local_port()?,
        })
    }

    fn extract_all(&self) -> Result<Vec<cdp::ExtractedSlackWebSession>, String> {
        relaunch_slack_with_debugging(self.port)?;
        let target = self
            .wait_for_slack_target(SLACK_BROWSER_WAIT, slack_app_target_priority)?
            .ok_or_else(|| slack_target_timeout_message(self.port))?;
        self.capture(target)
    }

    /// Opens the browser, waits for the user to reach a signed-in Slack
    /// workspace, captures it, and closes the browser again.
    #[cfg(target_os = "linux")]
    fn extract_from_browser(
        &self,
        browser: &std::path::Path,
    ) -> Result<Vec<cdp::ExtractedSlackWebSession>, String> {
        let _browser = browser::launch_browser_with_debugging(browser, self.port)?;
        let target = self
            .wait_for_slack_target(SLACK_BROWSER_SIGN_IN_WAIT, |target| {
                slack_app_target_priority(target).filter(|priority| *priority == 2)
            })?
            .ok_or_else(browser_sign_in_timeout_message)?;
        self.capture(target)
    }

    fn capture(&self, target: CdpTarget) -> Result<Vec<cdp::ExtractedSlackWebSession>, String> {
        let web_socket_url = target
            .web_socket_url
            .ok_or_else(|| "Slack Desktop did not expose a debugger websocket".to_string())?;
        tokio::runtime::Runtime::new()
            .map_err(|error| format!("failed to start Slack Desktop inspector runtime: {error}"))?
            .block_on(cdp::capture_slack_web_sessions(web_socket_url))
    }

    /// The best Slack page the debugger exposes, or `None` once `timeout` passes.
    fn wait_for_slack_target(
        &self,
        timeout: Duration,
        priority: impl Fn(&CdpTarget) -> Option<u8>,
    ) -> Result<Option<CdpTarget>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(target) = self
                .cdp_targets()?
                .into_iter()
                .filter_map(|target| priority(&target).map(|priority| (priority, target)))
                .max_by_key(|(priority, _)| *priority)
                .map(|(_, target)| target)
            {
                return Ok(Some(target));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn cdp_targets(&self) -> Result<Vec<CdpTarget>, String> {
        let url = format!("http://127.0.0.1:{}/json/list", self.port);
        match self.client.get(url).send() {
            Ok(response) if response.status().is_success() => response
                .json::<Vec<CdpTarget>>()
                .map_err(|error| format!("failed to decode Slack Desktop targets: {error}")),
            Ok(_) | Err(_) => Ok(Vec::new()),
        }
    }
}

fn available_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("failed to allocate Slack Desktop inspector port: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("failed to read Slack Desktop inspector port: {error}"))
}

fn slack_app_target_priority(target: &CdpTarget) -> Option<u8> {
    if target.kind != "page" || target.web_socket_url.is_none() {
        return None;
    }
    let url = url::Url::parse(&target.url).ok()?;
    if url.scheme() != "https" || url.host_str() != Some("app.slack.com") {
        return None;
    }
    Some(if url.path().starts_with("/client/") {
        2
    } else {
        1
    })
}

fn slack_target_timeout_message(port: u16) -> String {
    format!(
        "Slack Desktop did not expose a debuggable Slack page on 127.0.0.1:{port}; \
open Slack Desktop signed in to the workspace and try again"
    )
}

#[cfg(target_os = "linux")]
fn browser_sign_in_timeout_message() -> String {
    format!(
        "no signed-in Slack workspace appeared in the browser within {} minutes; \
sign in to Slack in the window notslack opens and try again",
        SLACK_BROWSER_SIGN_IN_WAIT.as_secs() / 60
    )
}
