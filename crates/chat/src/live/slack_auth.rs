use crate::live::{SlackApiClient, SlackLiveWorkspaceLoader};

mod channel_resolver;
mod conversation;
mod credentials;
mod desktop_session;
mod runtime_inputs;
mod state;
mod types;

pub use self::runtime_inputs::authenticated_slack_teams;
#[cfg(test)]
use self::runtime_inputs::{
    authenticated_runtime_input, confirm_stored_slack_team, join_slack_runtime_input_tasks,
    SlackRuntimeInputLoadError,
};
pub(super) use self::runtime_inputs::{
    authenticated_slack_runtime_inputs, available_slack_runtime_inputs,
    recapture_slack_runtime_input, SlackAuthenticatedRuntimeInput,
};
use self::{
    channel_resolver::resolve_slack_channel_by_name,
    conversation::resolve_default_conversation_id,
    credentials::{
        capture_and_store_slack_desktop_sessions, ensure_stored_slack_desktop_session,
        load_stored_slack_desktop_sessions, select_slack_desktop_session, slack_loader_inputs,
    },
    state::{
        slack_selection, store_slack_active_team, store_slack_team_conversation,
        SlackSelectionState,
    },
    types::{StoredSlackDesktopSession, StoredSlackDesktopSessions},
};
use crate::model::{ResolvedSlackChannel, SlackNotificationSound};

pub use self::types::SlackWebBuildTimestamp;

const SLACK_DESKTOP_SESSION_CACHE_KEY: &str = "slack|desktop-session";
const SLACK_DESKTOP_SESSIONS_V2_CACHE_KEY: &str = "slack|desktop-sessions-v2";
const SLACK_DESKTOP_SESSIONS_CACHE_KEY: &str = "slack|desktop-sessions-v3";
#[cfg(not(target_os = "macos"))]
const SLACK_DESKTOP_CONNECT_UNSUPPORTED: &str =
    "Connecting Slack Desktop is supported only on macOS";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackLiveLaunchContext {
    pub team_id: String,
    pub channel_id: String,
    pub tab_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackAuthenticatedTeam {
    pub team_id: String,
    pub workspace_name: String,
    pub workspace_logo_url: Option<String>,
    pub notification_playback: Option<SlackNotificationPlayback>,
    pub notification_sound: Option<SlackNotificationSound>,
    pub startup_conversation_id: Option<String>,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackDesktopIntegrationUnavailable {
    UnsupportedPlatform,
    NativeAppNotInstalled,
    NoCachedSession,
    NoWorkspace,
    CachedSessionError(String),
    RuntimeError(String),
}

impl std::fmt::Display for SlackDesktopIntegrationUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("Slack Desktop integration is unsupported on this platform")
            }
            Self::NativeAppNotInstalled => formatter.write_str("Slack Desktop is not installed"),
            Self::NoCachedSession => formatter
                .write_str("Slack Desktop is installed, but Slack is not connected to notslack"),
            Self::NoWorkspace => formatter.write_str("Slack Desktop has no signed-in workspace"),
            Self::CachedSessionError(message) => {
                write!(
                    formatter,
                    "Slack Desktop cached session is unavailable: {message}"
                )
            }
            Self::RuntimeError(message) => {
                write!(formatter, "Slack Desktop runtime is unavailable: {message}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackDesktopIntegrationStatus {
    Available,
    Unavailable(SlackDesktopIntegrationUnavailable),
}

#[cfg(target_os = "macos")]
pub fn check_slack_desktop_app() -> Result<(), SlackDesktopIntegrationUnavailable> {
    desktop_session::check_slack_desktop_app()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackNotificationPlayback {
    System,
    Web,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlackNotificationAudioPreferences {
    pub playback: Option<SlackNotificationPlayback>,
    pub sound: Option<SlackNotificationSound>,
}

#[cfg(test)]
pub(in crate::live) fn parse_cached_slack_desktop_sessions_for_test(
    raw: &str,
) -> Result<(), String> {
    credentials::parse_cached_slack_desktop_sessions(raw).map(drop)
}

fn with_supported_slack_desktop_capture<T>(
    _capture: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    #[cfg(target_os = "macos")]
    {
        _capture()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(SLACK_DESKTOP_CONNECT_UNSUPPORTED.to_string())
    }
}

fn capture_slack_desktop_credentials_from(
    capture_and_store: impl FnOnce() -> Result<StoredSlackDesktopSessions, String>,
) -> Result<(), String> {
    capture_and_store().map(drop)
}

pub fn capture_slack_desktop_credentials() -> Result<(), String> {
    with_supported_slack_desktop_capture(|| {
        capture_slack_desktop_credentials_from(capture_and_store_slack_desktop_sessions)
    })
}

pub fn connect_slack_desktop() -> Result<SlackLiveLaunchContext, String> {
    with_supported_slack_desktop_capture(|| {
        let selection = slack_selection()?;
        let sessions = capture_and_store_slack_desktop_sessions()?;
        launch_context_from_sessions(&sessions, &selection)
    })
}

pub fn default_slack_launch_context() -> Result<SlackLiveLaunchContext, String> {
    let selection = slack_selection()?;
    let session = stored_slack_session_for_launch(&selection)?;
    match launch_context_from_session(session, &selection) {
        Ok(launch_context) => Ok(launch_context),
        Err(error) if should_recapture_slack_desktop_session(error.as_str()) => {
            let sessions = capture_and_store_slack_desktop_sessions()?;
            launch_context_from_sessions(&sessions, &selection).map_err(|retry_error| {
                format!("{error}; Slack Desktop recapture failed: {retry_error}")
            })
        }
        Err(error) => Err(error),
    }
}

pub fn slack_live_loader(team_id: &str) -> Result<SlackLiveWorkspaceLoader, String> {
    let team_id = required_team_id(team_id)?;
    let session = ensure_stored_slack_desktop_session(Some(team_id))?;
    let (team_domain, web_session) = slack_loader_inputs(&session, team_id)?;
    SlackLiveWorkspaceLoader::new_with_team_domain(team_id, team_domain, web_session)
}

pub(super) fn resolve_default_conversation_for_loader(
    loader: &SlackLiveWorkspaceLoader,
) -> Result<String, String> {
    resolve_default_conversation_id(&loader.api_client(), None)
}

pub fn remember_slack_active_team(team_id: &str) -> Result<(), String> {
    store_slack_active_team(team_id)
}

pub fn remember_slack_team_conversation(
    team_id: &str,
    conversation_id: &str,
) -> Result<(), String> {
    store_slack_team_conversation(team_id, conversation_id)
}

pub fn resolve_slack_channel(channel_name: &str) -> Result<ResolvedSlackChannel, String> {
    let selection = slack_selection()?;
    let session = stored_slack_session_for_launch(&selection)?;
    resolve_slack_channel_for_team(&session.team_id, channel_name)
}

pub fn resolve_slack_channel_for_team(
    team_id: &str,
    channel_name: &str,
) -> Result<ResolvedSlackChannel, String> {
    let team_id = required_team_id(team_id)?;
    let session = ensure_stored_slack_desktop_session(Some(team_id))?;
    resolve_slack_channel_for_session(&session, channel_name)
}

fn resolve_slack_channel_for_session(
    session: &StoredSlackDesktopSession,
    channel_name: &str,
) -> Result<ResolvedSlackChannel, String> {
    let (_, web_session) = slack_loader_inputs(session, session.team_id.as_str())?;
    let api = SlackApiClient::desktop(&web_session)?;
    resolve_slack_channel_by_name(&api, &session.team_id, channel_name)
}

pub fn should_recapture_slack_desktop_session(error: &str) -> bool {
    error.contains("not_authed")
        || error.contains("invalid_auth")
        || error.contains("missing_scope")
        || error.contains("token_revoked")
        || error.contains("token_expired")
        || error.contains("expired_token")
        || error.contains("account_inactive")
        || (error.contains("Slack internal API")
            && (error.contains("HTTP 401") || error.contains("HTTP 403")))
}

fn launch_context_from_session(
    session: StoredSlackDesktopSession,
    selection: &SlackSelectionState,
) -> Result<SlackLiveLaunchContext, String> {
    let channel_id = match selection
        .conversation_id(&session.team_id)
        .map(str::trim)
        .filter(|conversation_id| !conversation_id.is_empty())
    {
        Some(conversation_id) => conversation_id.to_string(),
        None => {
            let (team_domain, web_session) =
                slack_loader_inputs(&session, session.team_id.as_str())?;
            let loader = SlackLiveWorkspaceLoader::new_with_team_domain(
                &session.team_id,
                team_domain,
                web_session,
            )?;
            resolve_default_conversation_id(&loader.api_client(), None)?
        }
    };
    store_slack_active_team(&session.team_id)?;
    store_slack_team_conversation(&session.team_id, &channel_id)?;
    Ok(SlackLiveLaunchContext {
        team_id: session.team_id,
        channel_id,
        tab_id: None,
    })
}

fn launch_context_from_sessions(
    sessions: &StoredSlackDesktopSessions,
    selection: &SlackSelectionState,
) -> Result<SlackLiveLaunchContext, String> {
    let preferred_team_id = selection
        .active_team_id()
        .filter(|team_id| sessions.session(team_id).is_some());
    let session = select_slack_desktop_session(sessions, preferred_team_id)?;
    launch_context_from_session(session, selection)
}

fn stored_slack_session_for_launch(
    selection: &SlackSelectionState,
) -> Result<StoredSlackDesktopSession, String> {
    if let Some(sessions) = load_stored_slack_desktop_sessions()? {
        let preferred_team_id = selection
            .active_team_id()
            .filter(|team_id| sessions.session(team_id).is_some());
        return select_slack_desktop_session(&sessions, preferred_team_id);
    }
    let sessions = capture_and_store_slack_desktop_sessions()?;
    let preferred_team_id = selection
        .active_team_id()
        .filter(|team_id| sessions.session(team_id).is_some());
    select_slack_desktop_session(&sessions, preferred_team_id)
}

fn required_team_id(team_id: &str) -> Result<&str, String> {
    let team_id = team_id.trim();
    if team_id.is_empty() {
        return Err("Slack team id must not be empty".to_string());
    }
    Ok(team_id)
}

#[cfg(test)]
mod tests;
