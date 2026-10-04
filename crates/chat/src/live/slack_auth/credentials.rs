use std::sync::{Mutex, OnceLock};

use secret_store::SecretStore;

use crate::live::{internal_sidebar::SlackTeamDomain, SlackWebSessionCredentials};

use super::{
    desktop_session::capture_slack_desktop_sessions,
    types::{StoredSlackDesktopSession, StoredSlackDesktopSessions},
    SLACK_DESKTOP_SESSIONS_CACHE_KEY, SLACK_DESKTOP_SESSIONS_V2_CACHE_KEY,
    SLACK_DESKTOP_SESSION_CACHE_KEY,
};

static SLACK_DESKTOP_CAPTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SlackCachedSessionsLoadError {
    Storage(String),
    CachedSession(String),
}

impl SlackCachedSessionsLoadError {
    pub(super) fn into_message(self) -> String {
        match self {
            Self::Storage(message) | Self::CachedSession(message) => message,
        }
    }
}

pub(crate) fn ensure_stored_slack_desktop_session(
    preferred_team_id: Option<&str>,
) -> Result<StoredSlackDesktopSession, String> {
    if let Some(sessions) = load_stored_slack_desktop_sessions()? {
        if preferred_team_id.is_none_or(|team_id| sessions.session(team_id).is_some()) {
            return select_slack_desktop_session(&sessions, preferred_team_id);
        }
    }

    let sessions = capture_and_store_slack_desktop_sessions()?;
    select_slack_desktop_session(&sessions, preferred_team_id)
}

pub(crate) fn capture_and_store_slack_desktop_sessions(
) -> Result<StoredSlackDesktopSessions, String> {
    let _capture = SLACK_DESKTOP_CAPTURE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Slack Desktop capture mutex poisoned".to_string())?;
    let sessions = capture_slack_desktop_sessions()?;
    store_slack_desktop_sessions(&sessions)?;
    Ok(sessions)
}

pub(crate) fn load_stored_slack_desktop_sessions(
) -> Result<Option<StoredSlackDesktopSessions>, String> {
    if let Some(sessions) =
        load_cached_slack_desktop_sessions().map_err(|error| error.into_message())?
    {
        return Ok(Some(sessions));
    }
    if deprecated_slack_desktop_session_cache_exists()? {
        return capture_and_store_slack_desktop_sessions().map(Some);
    }
    Ok(None)
}

pub(super) fn parse_cached_slack_desktop_sessions(
    raw: &str,
) -> Result<StoredSlackDesktopSessions, String> {
    serde_json::from_str::<StoredSlackDesktopSessions>(raw)
        .map_err(|error| format!("failed to decode stored Slack Desktop sessions: {error}"))?
        .validate()
}

pub(crate) fn load_cached_slack_desktop_sessions(
) -> Result<Option<StoredSlackDesktopSessions>, SlackCachedSessionsLoadError> {
    load_cached_slack_desktop_sessions_from(|| {
        read_secret(
            SLACK_DESKTOP_SESSIONS_CACHE_KEY,
            "stored Slack Desktop sessions",
        )
    })
}

pub(super) fn load_cached_slack_desktop_sessions_from(
    read: impl FnOnce() -> Result<Option<String>, String>,
) -> Result<Option<StoredSlackDesktopSessions>, SlackCachedSessionsLoadError> {
    let Some(raw) = read().map_err(SlackCachedSessionsLoadError::Storage)? else {
        return Ok(None);
    };
    parse_cached_slack_desktop_sessions(&raw)
        .map(Some)
        .map_err(SlackCachedSessionsLoadError::CachedSession)
}

pub(crate) fn select_slack_desktop_session(
    sessions: &StoredSlackDesktopSessions,
    preferred_team_id: Option<&str>,
) -> Result<StoredSlackDesktopSession, String> {
    let team_id = preferred_team_id
        .map(str::trim)
        .filter(|team_id| !team_id.is_empty())
        .or_else(|| sessions.selected_team_id())
        .unwrap_or_else(|| sessions.first_team_id());
    sessions
        .session(team_id)
        .cloned()
        .ok_or_else(|| requested_workspace_missing_message(team_id))
}

fn store_slack_desktop_sessions(sessions: &StoredSlackDesktopSessions) -> Result<(), String> {
    let store = SecretStore::notslack().map_err(|error| error.to_string())?;
    let raw = serde_json::to_string(sessions)
        .map_err(|error| format!("failed to encode stored Slack Desktop sessions: {error}"))?;
    store
        .upsert_secret(SLACK_DESKTOP_SESSIONS_CACHE_KEY, &raw)
        .map_err(|error| {
            format!("failed to store Slack Desktop sessions in ~/.notslack/auth.json: {error}")
        })
}

fn read_secret(key: &str, label: &str) -> Result<Option<String>, String> {
    SecretStore::notslack()
        .map_err(|error| error.to_string())?
        .read_secret(key)
        .map_err(|error| format!("failed to read {label}: {error}"))
}

pub(crate) fn slack_loader_inputs(
    session: &StoredSlackDesktopSession,
    expected_team_id: &str,
) -> Result<(SlackTeamDomain, SlackWebSessionCredentials), String> {
    let expected_team_id = non_empty_session_field(expected_team_id, "Slack team id")?;
    if session.team_id != expected_team_id {
        return Err(format!(
            "stored Slack Desktop session team {} did not match requested team {expected_team_id}",
            session.team_id
        ));
    }
    let team_domain = SlackTeamDomain::parse(non_empty_session_field(
        &session.team_domain,
        "Slack team domain",
    )?)?;
    let draft_count = session.draft_count.ok_or_else(|| {
        "stored Slack Desktop session is missing required sidebar draft state".to_string()
    })?;
    let web_session = SlackWebSessionCredentials::new(
        session.xoxc_token.clone(),
        session.cookie_header.clone(),
        session.web_build_timestamp.clone(),
    )?
    .with_draft_count(draft_count);
    Ok((team_domain, web_session))
}

fn deprecated_slack_desktop_session_cache_exists() -> Result<bool, String> {
    if read_secret(
        SLACK_DESKTOP_SESSIONS_V2_CACHE_KEY,
        "stored Slack Desktop v2 sessions",
    )?
    .is_some()
    {
        return Ok(true);
    }
    read_secret(
        SLACK_DESKTOP_SESSION_CACHE_KEY,
        "stored legacy Slack Desktop session",
    )
    .map(|session| session.is_some())
}

fn non_empty_session_field(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("stored Slack Desktop session is missing {label}"));
    }
    Ok(value.to_string())
}

fn requested_workspace_missing_message(team_id: &str) -> String {
    format!(
        "Slack Desktop is not signed in to requested workspace {team_id}; open that workspace in Slack Desktop and try again"
    )
}
