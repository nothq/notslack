use std::{
    collections::BTreeMap,
    fs, io,
    net::TcpListener,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use reqwest::blocking::Client;
use serde::Deserialize;

use super::types::{
    StoredSlackDesktopSession, StoredSlackDesktopSessionRecord, StoredSlackDesktopSessions,
};
#[cfg(target_os = "macos")]
use super::SlackDesktopIntegrationUnavailable;
use crate::live::{SlackApiClient, SlackWebSessionCredentials};

mod cdp;
mod desktop_state;

const SLACK_BUNDLE_PATH: &str = "/Applications/Slack.app";
const SLACK_BROWSER_WAIT: Duration = Duration::from_secs(60);
const SLACK_QUIT_WAIT: Duration = Duration::from_secs(5);

pub(crate) fn capture_slack_desktop_sessions() -> Result<StoredSlackDesktopSessions, String> {
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

#[cfg(target_os = "macos")]
pub(super) fn check_slack_desktop_app() -> Result<(), SlackDesktopIntegrationUnavailable> {
    match installed_slack_app_path() {
        Ok(_) => Ok(()),
        Err(SlackDesktopAppError::NotInstalled) => {
            Err(SlackDesktopIntegrationUnavailable::NativeAppNotInstalled)
        }
        Err(error) => Err(SlackDesktopIntegrationUnavailable::RuntimeError(
            error.to_string(),
        )),
    }
}

fn validate_captured_session(session: &cdp::ExtractedSlackWebSession) -> Result<(), String> {
    let credentials = SlackWebSessionCredentials::new(
        session.xoxc_token.clone(),
        session.cookie_header.clone(),
        session.web_build_timestamp.clone(),
    )?;
    let auth = SlackApiClient::desktop(&credentials)?.post("auth.test", &[])?;
    if auth.get("team_id").and_then(serde_json::Value::as_str) != Some(session.team_id.as_str()) {
        return Err(format!(
            "Slack auth.test did not confirm captured team {}",
            session.team_id
        ));
    }
    Ok(())
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
        let target = self.wait_for_slack_target()?;
        let web_socket_url = target
            .web_socket_url
            .ok_or_else(|| "Slack Desktop did not expose a debugger websocket".to_string())?;
        tokio::runtime::Runtime::new()
            .map_err(|error| format!("failed to start Slack Desktop inspector runtime: {error}"))?
            .block_on(cdp::capture_slack_web_sessions(web_socket_url))
    }

    fn wait_for_slack_target(&self) -> Result<CdpTarget, String> {
        let deadline = Instant::now() + SLACK_BROWSER_WAIT;
        loop {
            if let Some(target) = self
                .cdp_targets()?
                .into_iter()
                .filter_map(|target| {
                    slack_app_target_priority(&target).map(|priority| (priority, target))
                })
                .max_by_key(|(priority, _)| *priority)
                .map(|(_, target)| target)
            {
                return Ok(target);
            }
            if Instant::now() >= deadline {
                return Err(slack_target_timeout_message(self.port));
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

fn relaunch_slack_with_debugging(port: u16) -> Result<(), String> {
    let app_path = installed_slack_app_path().map_err(|error| error.to_string())?;
    quit_slack_desktop();
    thread::sleep(Duration::from_millis(1200));
    let status = Command::new("open")
        .arg("-na")
        .arg(&app_path)
        .arg("--args")
        .arg(format!("--remote-debugging-port={port}"))
        .arg("--remote-debugging-address=127.0.0.1")
        .status()
        .map_err(|error| format!("failed to launch Slack Desktop: {error}"))?;
    if status.success() {
        open_slack_desktop_window();
        return Ok(());
    }
    Err(format!(
        "failed to launch Slack Desktop with status {status}"
    ))
}

#[derive(Debug)]
enum SlackDesktopAppError {
    NotInstalled,
    HomeUnavailable,
    InspectionFailed { path: PathBuf, source: io::Error },
    InvalidBundle(PathBuf),
}

impl std::fmt::Display for SlackDesktopAppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => formatter.write_str(&slack_app_missing_message()),
            Self::HomeUnavailable => {
                formatter.write_str("failed to resolve the home directory for Slack Desktop")
            }
            Self::InspectionFailed { path, source } => write!(
                formatter,
                "failed to inspect Slack Desktop application {}: {source}",
                path.display()
            ),
            Self::InvalidBundle(path) => write!(
                formatter,
                "Slack Desktop application {} is not a valid application bundle",
                path.display()
            ),
        }
    }
}

fn installed_slack_app_path() -> Result<PathBuf, SlackDesktopAppError> {
    let system_path = PathBuf::from(SLACK_BUNDLE_PATH);
    if let Some(path) = inspect_slack_app_bundle(system_path)? {
        return Ok(path);
    }
    let home_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(SlackDesktopAppError::HomeUnavailable)?;
    let user_path = home_path.join("Applications/Slack.app");
    inspect_slack_app_bundle(user_path)?.ok_or(SlackDesktopAppError::NotInstalled)
}

fn inspect_slack_app_bundle(path: PathBuf) -> Result<Option<PathBuf>, SlackDesktopAppError> {
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(SlackDesktopAppError::InspectionFailed { path, source }),
    };
    if !metadata.is_dir() {
        return Err(SlackDesktopAppError::InvalidBundle(path));
    }
    let info_path = path.join("Contents/Info.plist");
    let info_metadata = fs::metadata(&info_path).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            SlackDesktopAppError::InvalidBundle(path.clone())
        } else {
            SlackDesktopAppError::InspectionFailed {
                path: info_path,
                source,
            }
        }
    })?;
    if !info_metadata.is_file() {
        return Err(SlackDesktopAppError::InvalidBundle(path));
    }
    Ok(Some(path))
}

fn quit_slack_desktop() {
    force_quit_slack_desktop();
    let _ = wait_for_slack_exit(SLACK_QUIT_WAIT);
}

fn wait_for_slack_exit(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !slack_process_running() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn slack_process_running() -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg("Slack")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn force_quit_slack_desktop() {
    let _ = Command::new("killall")
        .arg("Slack")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn open_slack_desktop_window() {
    let _ = Command::new("open")
        .arg("slack://")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
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

fn slack_app_missing_message() -> String {
    "Slack Desktop is required to import Slack web session credentials automatically".to_string()
}

fn slack_target_timeout_message(port: u16) -> String {
    format!(
        "Slack Desktop did not expose a debuggable Slack page on 127.0.0.1:{port}; \
open Slack Desktop signed in to the workspace and try again"
    )
}
