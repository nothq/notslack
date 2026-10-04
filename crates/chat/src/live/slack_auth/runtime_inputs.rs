use crate::live::{api::SlackApiRequestError, SlackLiveWorkspaceLoader};

use super::{
    credentials::{
        capture_and_store_slack_desktop_sessions, load_cached_slack_desktop_sessions,
        load_stored_slack_desktop_sessions, slack_loader_inputs, SlackCachedSessionsLoadError,
    },
    required_team_id, should_recapture_slack_desktop_session,
    state::{slack_selection, SlackSelectionState},
    types::{StoredSlackDesktopSessionRecord, StoredSlackDesktopSessions},
    SlackAuthenticatedTeam, SlackDesktopIntegrationUnavailable,
};

pub(in crate::live) struct SlackAuthenticatedRuntimeInput {
    pub(in crate::live) team: SlackAuthenticatedTeam,
    pub(in crate::live) loader: SlackLiveWorkspaceLoader,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SlackRuntimeInputLoadError {
    CachedSession(String),
    Runtime(String),
}

impl SlackRuntimeInputLoadError {
    fn into_message(self) -> String {
        match self {
            Self::CachedSession(message) | Self::Runtime(message) => message,
        }
    }

    pub(super) fn into_unavailable(self) -> SlackDesktopIntegrationUnavailable {
        match self {
            Self::CachedSession(message) => {
                SlackDesktopIntegrationUnavailable::CachedSessionError(message)
            }
            Self::Runtime(message) => SlackDesktopIntegrationUnavailable::RuntimeError(message),
        }
    }

    fn merge_preserving_runtime(self, later: Self) -> Self {
        match (self, later) {
            (Self::Runtime(first), _) => Self::Runtime(first),
            (Self::CachedSession(first), Self::CachedSession(_)) => Self::CachedSession(first),
            (Self::CachedSession(first), Self::Runtime(later)) => Self::Runtime(format!(
                "{later}; earlier Slack cached-session failure: {first}"
            )),
        }
    }
}

impl From<SlackCachedSessionsLoadError> for SlackRuntimeInputLoadError {
    fn from(error: SlackCachedSessionsLoadError) -> Self {
        match error {
            SlackCachedSessionsLoadError::Storage(message) => Self::Runtime(message),
            SlackCachedSessionsLoadError::CachedSession(message) => Self::CachedSession(message),
        }
    }
}

impl From<SlackApiRequestError> for SlackRuntimeInputLoadError {
    fn from(error: SlackApiRequestError) -> Self {
        match error {
            SlackApiRequestError::AuthenticationRejected(message) => Self::CachedSession(message),
            SlackApiRequestError::Runtime(message) => Self::Runtime(message),
        }
    }
}

type SlackRuntimeInputTask<'scope, T> =
    std::thread::ScopedJoinHandle<'scope, Result<T, SlackRuntimeInputLoadError>>;

pub(super) fn join_slack_runtime_input_tasks<'scope, T>(
    tasks: Vec<SlackRuntimeInputTask<'scope, T>>,
) -> Result<Vec<T>, SlackRuntimeInputLoadError> {
    let mut values = Vec::with_capacity(tasks.len());
    let mut first_error: Option<SlackRuntimeInputLoadError> = None;
    for task in tasks {
        let result = task.join().unwrap_or_else(|_| {
            Err(SlackRuntimeInputLoadError::Runtime(
                "authenticated Slack team loader panicked".to_string(),
            ))
        });
        match result {
            Ok(value) => values.push(value),
            Err(error) => {
                first_error = Some(match first_error.take() {
                    Some(first) => first.merge_preserving_runtime(error),
                    None => error,
                });
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(values),
    }
}

pub(super) fn confirm_stored_slack_team(
    authentication: &serde_json::Value,
    stored_team_id: &str,
) -> Result<(), SlackRuntimeInputLoadError> {
    let authenticated_team_id = authentication
        .get("team_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            SlackRuntimeInputLoadError::Runtime(
                "Slack auth.test response did not include a team id".to_string(),
            )
        })?;
    if authenticated_team_id != stored_team_id {
        return Err(SlackRuntimeInputLoadError::CachedSession(format!(
            "Slack auth.test confirmed team {authenticated_team_id}, not stored team {stored_team_id}",
        )));
    }
    Ok(())
}

pub fn authenticated_slack_teams() -> Result<Vec<SlackAuthenticatedTeam>, String> {
    authenticated_slack_runtime_inputs()
        .map(|inputs| inputs.into_iter().map(|input| input.team).collect())
}

pub(in crate::live) fn authenticated_slack_runtime_inputs(
) -> Result<Vec<SlackAuthenticatedRuntimeInput>, String> {
    let selection = slack_selection()?;
    let Some(sessions) = load_stored_slack_desktop_sessions()? else {
        return capture_authenticated_runtime_inputs(&selection);
    };
    match authenticated_runtime_inputs_from_sessions(&sessions, &selection, true) {
        Ok(inputs) => Ok(inputs),
        Err(error) if should_recapture_slack_desktop_session(&error) => {
            capture_authenticated_runtime_inputs(&selection).map_err(|retry_error| {
                format!("{error}; Slack Desktop recapture failed: {retry_error}")
            })
        }
        Err(error) => Err(error),
    }
}

pub(in crate::live) fn available_slack_runtime_inputs(
) -> Result<Vec<SlackAuthenticatedRuntimeInput>, SlackDesktopIntegrationUnavailable> {
    let Some(sessions) = load_cached_slack_desktop_sessions()
        .map_err(|error| SlackRuntimeInputLoadError::from(error).into_unavailable())?
    else {
        return Ok(Vec::new());
    };
    let selection = slack_selection().map_err(SlackDesktopIntegrationUnavailable::RuntimeError)?;
    slack_runtime_inputs_from_sessions(&sessions, &selection, true)
        .map_err(SlackRuntimeInputLoadError::into_unavailable)
}

pub(in crate::live) fn recapture_slack_runtime_input(
    team_id: &str,
) -> Result<SlackAuthenticatedRuntimeInput, String> {
    let team_id = required_team_id(team_id)?;
    let selection = slack_selection()?;
    capture_authenticated_runtime_inputs(&selection)?
        .into_iter()
        .find(|input| input.team.team_id == team_id)
        .ok_or_else(|| {
            format!(
                "Slack Desktop recapture did not include requested workspace {team_id}; open that workspace in Slack Desktop and try again"
            )
        })
}

fn capture_authenticated_runtime_inputs(
    selection: &SlackSelectionState,
) -> Result<Vec<SlackAuthenticatedRuntimeInput>, String> {
    let sessions = capture_and_store_slack_desktop_sessions()?;
    slack_runtime_inputs_from_sessions(&sessions, selection, false)
        .map_err(SlackRuntimeInputLoadError::into_message)
}

fn authenticated_runtime_inputs_from_sessions(
    sessions: &StoredSlackDesktopSessions,
    selection: &SlackSelectionState,
    validate_authentication: bool,
) -> Result<Vec<SlackAuthenticatedRuntimeInput>, String> {
    slack_runtime_inputs_from_sessions(sessions, selection, validate_authentication)
        .map_err(SlackRuntimeInputLoadError::into_message)
}

fn slack_runtime_inputs_from_sessions(
    sessions: &StoredSlackDesktopSessions,
    selection: &SlackSelectionState,
    validate_authentication: bool,
) -> Result<Vec<SlackAuthenticatedRuntimeInput>, SlackRuntimeInputLoadError> {
    let default_team_id = selection
        .active_team_id()
        .filter(|team_id| sessions.session(team_id).is_some())
        .or_else(|| sessions.selected_team_id())
        .unwrap_or_else(|| sessions.first_team_id())
        .to_string();
    std::thread::scope(|scope| {
        let tasks = sessions
            .ordered_records()
            .map(|record| {
                let is_default = record.session.team_id == default_team_id;
                scope.spawn(move || {
                    authenticated_runtime_input(
                        record,
                        selection,
                        is_default,
                        validate_authentication,
                    )
                })
            })
            .collect::<Vec<_>>();
        join_slack_runtime_input_tasks(tasks)
    })
}

pub(super) fn authenticated_runtime_input(
    record: &StoredSlackDesktopSessionRecord,
    selection: &SlackSelectionState,
    is_default: bool,
    validate_authentication: bool,
) -> Result<SlackAuthenticatedRuntimeInput, SlackRuntimeInputLoadError> {
    let session = &record.session;
    let (team_domain, web_session) = slack_loader_inputs(session, &session.team_id)
        .map_err(SlackRuntimeInputLoadError::CachedSession)?;
    let loader =
        SlackLiveWorkspaceLoader::new_with_team_domain(&session.team_id, team_domain, web_session)
            .map_err(SlackRuntimeInputLoadError::Runtime)?;
    if validate_authentication {
        let authentication = loader
            .api_client()
            .post_with_error_provenance("auth.test", &[])
            .map_err(SlackRuntimeInputLoadError::from)?;
        confirm_stored_slack_team(&authentication, &session.team_id)?;
    }
    let startup_conversation_id = selection
        .conversation_id(&session.team_id)
        .map(str::trim)
        .filter(|conversation_id| !conversation_id.is_empty())
        .map(str::to_string);
    Ok(SlackAuthenticatedRuntimeInput {
        team: SlackAuthenticatedTeam {
            team_id: session.team_id.clone(),
            workspace_name: record.workspace_name.clone(),
            workspace_logo_url: record.workspace_logo_url.clone(),
            notification_playback: record.notification_playback,
            notification_sound: record.notification_sound,
            startup_conversation_id,
            is_default,
        },
        loader,
    })
}
