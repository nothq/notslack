use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::model::SlackNotificationSound;

pub(super) struct SlackDesktopWorkspaceDirectory {
    pub(super) selected_team_id: Option<String>,
    pub(super) ordered: Vec<SlackDesktopWorkspaceIdentity>,
}

pub(super) struct SlackDesktopWorkspaceIdentity {
    pub(super) team_id: String,
    pub(super) workspace_name: String,
    pub(super) team_domain: String,
    pub(super) workspace_logo_url: Option<String>,
    pub(super) notification_playback: Option<super::super::SlackNotificationPlayback>,
    pub(super) notification_sound: Option<SlackNotificationSound>,
}

struct SlackDesktopNotificationPreferences {
    playback: Option<super::super::SlackNotificationPlayback>,
    sound_by_team_id: BTreeMap<String, SlackNotificationSound>,
}

#[derive(Debug)]
pub(super) enum SlackDesktopRootStatePathError {
    ApplicationSupportUnavailable,
    #[cfg(target_os = "macos")]
    ApplicationSupportHasNoLibraryDirectory(PathBuf),
    InspectionFailed {
        path: PathBuf,
        source: io::Error,
    },
    Missing,
    #[cfg(target_os = "macos")]
    Ambiguous {
        standard: PathBuf,
        sandboxed: PathBuf,
    },
}

impl fmt::Display for SlackDesktopRootStatePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApplicationSupportUnavailable => {
                formatter.write_str("failed to resolve the application data directory")
            }
            #[cfg(target_os = "macos")]
            Self::ApplicationSupportHasNoLibraryDirectory(path) => write!(
                formatter,
                "macOS application support directory {} has no parent Library directory",
                path.display()
            ),
            Self::InspectionFailed { path, source } => write!(
                formatter,
                "failed to inspect Slack Desktop workspace state {}: {source}",
                path.display()
            ),
            Self::Missing => formatter.write_str(
                "Slack Desktop workspace state was not found; open Slack Desktop and sign in first",
            ),
            #[cfg(target_os = "macos")]
            Self::Ambiguous {
                standard,
                sandboxed,
            } => write!(
                formatter,
                "Slack Desktop workspace state exists in both {} and {}; notslack cannot choose between them",
                standard.display(),
                sandboxed.display()
            ),
        }
    }
}

#[derive(Debug)]
pub(super) enum SlackDesktopWorkspaceStateError {
    RootStatePath(SlackDesktopRootStatePathError),
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Decode {
        path: PathBuf,
        source: serde_json::Error,
    },
    NoWorkspaces,
    Invalid(String),
}

impl fmt::Display for SlackDesktopWorkspaceStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootStatePath(error) => error.fmt(formatter),
            Self::Read { path, source } => write!(
                formatter,
                "failed to read Slack Desktop workspace state {}: {source}",
                path.display()
            ),
            Self::Decode { path, source } => write!(
                formatter,
                "failed to decode Slack Desktop workspace state {}: {source}",
                path.display()
            ),
            Self::NoWorkspaces => {
                formatter.write_str("Slack Desktop workspace state did not include any workspaces")
            }
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl SlackDesktopNotificationPreferences {
    fn sound(&self, team_id: &str) -> Option<SlackNotificationSound> {
        self.sound_by_team_id.get(team_id).copied()
    }
}

#[derive(Deserialize)]
struct SlackDesktopRootState {
    workspaces: BTreeMap<String, SlackDesktopRootWorkspace>,
    #[serde(rename = "workspacesMeta")]
    workspaces_meta: SlackDesktopRootWorkspacesMeta,
    #[serde(default)]
    settings: SlackDesktopRootSettings,
    #[serde(default)]
    webapp: SlackDesktopRootWebapp,
}

#[derive(Default, Deserialize)]
struct SlackDesktopRootSettings {
    #[serde(default, rename = "notificationPlayback")]
    notification_playback: Option<String>,
}

#[derive(Default, Deserialize)]
struct SlackDesktopRootWebapp {
    #[serde(default)]
    teams: BTreeMap<String, SlackDesktopRootWebappTeam>,
}

#[derive(Default, Deserialize)]
struct SlackDesktopRootWebappTeam {
    #[serde(default, rename = "notificationPrefs")]
    notification_preferences: SlackDesktopRootNotificationPreferences,
}

#[derive(Default, Deserialize)]
struct SlackDesktopRootNotificationPreferences {
    #[serde(default, rename = "notificationSound")]
    notification_sound: Option<String>,
}

#[derive(Deserialize)]
struct SlackDesktopRootWorkspace {
    id: String,
    name: String,
    domain: String,
    order: u64,
    #[serde(default)]
    icon: SlackDesktopRootWorkspaceIcon,
}

#[derive(Default, Deserialize)]
struct SlackDesktopRootWorkspaceIcon {
    #[serde(default)]
    image_88: Option<String>,
    #[serde(default)]
    image_68: Option<String>,
}

#[derive(Deserialize)]
struct SlackDesktopRootWorkspacesMeta {
    #[serde(default, rename = "selectedWorkspaceId")]
    selected_workspace_id: Option<String>,
}

pub(super) fn load_slack_desktop_workspace_directory(
) -> Result<SlackDesktopWorkspaceDirectory, String> {
    load_slack_desktop_workspace_directory_checked().map_err(|error| error.to_string())
}

fn load_slack_desktop_workspace_directory_checked(
) -> Result<SlackDesktopWorkspaceDirectory, SlackDesktopWorkspaceStateError> {
    let state = load_slack_desktop_root_state()?;
    if state.workspaces.is_empty() {
        return Err(SlackDesktopWorkspaceStateError::NoWorkspaces);
    }
    let notification_preferences = notification_preferences_from_state(&state)
        .map_err(SlackDesktopWorkspaceStateError::Invalid)?;

    let ordered = ordered_workspace_identities(state.workspaces, &notification_preferences)
        .map_err(SlackDesktopWorkspaceStateError::Invalid)?;

    let selected_team_id = state
        .workspaces_meta
        .selected_workspace_id
        .as_deref()
        .map(|team_id| required_field(team_id, "selected workspace id"))
        .transpose()
        .map_err(SlackDesktopWorkspaceStateError::Invalid)?;
    if let Some(selected_team_id) = selected_team_id.as_deref() {
        if !ordered
            .iter()
            .any(|workspace| workspace.team_id == selected_team_id)
        {
            return Err(SlackDesktopWorkspaceStateError::Invalid(format!(
                "Slack Desktop selected workspace {selected_team_id} was absent from workspace state"
            )));
        }
    }
    Ok(SlackDesktopWorkspaceDirectory {
        selected_team_id,
        ordered,
    })
}

fn ordered_workspace_identities(
    workspaces: BTreeMap<String, SlackDesktopRootWorkspace>,
    notification_preferences: &SlackDesktopNotificationPreferences,
) -> Result<Vec<SlackDesktopWorkspaceIdentity>, String> {
    let mut ordered = Vec::with_capacity(workspaces.len());
    let mut ordered_records = workspaces.into_iter().collect::<Vec<_>>();
    ordered_records.sort_by(|(left_team_id, left), (right_team_id, right)| {
        left.order
            .cmp(&right.order)
            .then_with(|| left_team_id.cmp(right_team_id))
    });
    for adjacent in ordered_records.windows(2) {
        if adjacent[0].1.order == adjacent[1].1.order {
            return Err(format!(
                "Slack Desktop workspace state repeated order {}",
                adjacent[0].1.order
            ));
        }
    }
    for (map_team_id, workspace) in ordered_records {
        let team_id = required_field(&workspace.id, "team id")?;
        if map_team_id != team_id {
            return Err(format!(
                "Slack Desktop workspace state key {map_team_id} did not match team {team_id}"
            ));
        }
        let notification_sound = notification_preferences.sound(&team_id);
        ordered.push(SlackDesktopWorkspaceIdentity {
            team_id,
            workspace_name: required_field(&workspace.name, "workspace name")?,
            team_domain: required_field(&workspace.domain, "workspace domain")?,
            workspace_logo_url: first_non_empty([
                workspace.icon.image_88.as_deref(),
                workspace.icon.image_68.as_deref(),
            ]),
            notification_playback: notification_preferences.playback,
            notification_sound,
        });
    }

    Ok(ordered)
}

fn notification_preferences_from_state(
    state: &SlackDesktopRootState,
) -> Result<SlackDesktopNotificationPreferences, String> {
    let playback = state
        .settings
        .notification_playback
        .as_deref()
        .map(parse_notification_playback)
        .transpose()?;
    let sound_by_team_id = state
        .webapp
        .teams
        .iter()
        .filter_map(|(team_id, team)| {
            team.notification_preferences
                .notification_sound
                .as_deref()
                .map(str::trim)
                .filter(|sound| !sound.is_empty())
                .map(|sound| {
                    SlackNotificationSound::parse(sound)
                        .map(|sound| (team_id.clone(), sound))
                        .map_err(|error| {
                            format!(
                                "Slack Desktop workspace state has an invalid notification sound for team {team_id}: {error}"
                            )
                        })
                })
        })
        .collect::<Result<_, _>>()?;
    Ok(SlackDesktopNotificationPreferences {
        playback,
        sound_by_team_id,
    })
}

fn load_slack_desktop_root_state() -> Result<SlackDesktopRootState, SlackDesktopWorkspaceStateError>
{
    let path =
        slack_desktop_root_state_path().map_err(SlackDesktopWorkspaceStateError::RootStatePath)?;
    let contents =
        fs::read_to_string(&path).map_err(|source| SlackDesktopWorkspaceStateError::Read {
            path: path.clone(),
            source,
        })?;
    serde_json::from_str::<SlackDesktopRootState>(&contents)
        .map_err(|source| SlackDesktopWorkspaceStateError::Decode { path, source })
}

fn parse_notification_playback(
    playback: &str,
) -> Result<super::super::SlackNotificationPlayback, String> {
    match playback.trim() {
        "system" => Ok(super::super::SlackNotificationPlayback::System),
        "web" => Ok(super::super::SlackNotificationPlayback::Web),
        playback => Err(format!(
            "Slack Desktop workspace state has unsupported notification playback {playback}"
        )),
    }
}

pub(super) fn slack_desktop_root_state_path() -> Result<PathBuf, SlackDesktopRootStatePathError> {
    let data_dir =
        dirs::data_dir().ok_or(SlackDesktopRootStatePathError::ApplicationSupportUnavailable)?;
    let standard = data_dir.join("Slack/storage/root-state.json");

    #[cfg(target_os = "macos")]
    {
        let library_dir = data_dir.parent().ok_or_else(|| {
            SlackDesktopRootStatePathError::ApplicationSupportHasNoLibraryDirectory(
                data_dir.clone(),
            )
        })?;
        let sandboxed = library_dir.join(
            "Containers/com.tinyspeck.slackmacgap/Data/Library/Application Support/Slack/storage/root-state.json",
        );
        match (
            state_path_exists(&standard)?,
            state_path_exists(&sandboxed)?,
        ) {
            (true, false) => Ok(standard),
            (false, true) => Ok(sandboxed),
            (false, false) => Err(SlackDesktopRootStatePathError::Missing),
            (true, true) => Err(SlackDesktopRootStatePathError::Ambiguous {
                standard,
                sandboxed,
            }),
        }
    }

    #[cfg(not(target_os = "macos"))]
    newest_state_path(other_slack_root_state_paths(standard))
}

#[cfg(target_os = "windows")]
fn other_slack_root_state_paths(standard: PathBuf) -> Vec<PathBuf> {
    vec![standard]
}

/// Electron keeps Slack's data under ~/.config on Linux rather than the data
/// directory, so look there, then in the Snap and Flatpak locations.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn other_slack_root_state_paths(_standard: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("Slack/storage/root-state.json"));
    }
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join("snap/slack/current/.config/Slack/storage/root-state.json"));
        paths.push(home.join(".var/app/com.slack.Slack/config/Slack/storage/root-state.json"));
    }
    paths
}

/// Picks the most recently written state when Slack was installed more than once.
#[cfg(not(target_os = "macos"))]
fn newest_state_path(paths: Vec<PathBuf>) -> Result<PathBuf, SlackDesktopRootStatePathError> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for path in paths {
        if !state_path_exists(&path)? {
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .map_err(|source| SlackDesktopRootStatePathError::InspectionFailed {
                path: path.clone(),
                source,
            })?;
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    newest
        .map(|(_, path)| path)
        .ok_or(SlackDesktopRootStatePathError::Missing)
}

fn state_path_exists(path: &Path) -> Result<bool, SlackDesktopRootStatePathError> {
    path.try_exists()
        .map_err(|source| SlackDesktopRootStatePathError::InspectionFailed {
            path: path.to_path_buf(),
            source,
        })
}

fn required_field(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("Slack Desktop workspace state is missing {label}"));
    }
    Ok(value.to_string())
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_string)
}
