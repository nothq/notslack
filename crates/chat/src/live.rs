mod api;
mod archive;
mod archive_runtime;
mod cache;
mod connection;
mod conversation;
mod events;
mod internal_sidebar;
mod local_file;
mod payload;
mod runtime;
mod runtime_registry;
mod slack_auth;
mod slack_connection;

pub use api::SlackApiClient;
pub use archive::load_slack_archive;
#[cfg(test)]
pub(crate) use archive_runtime::append_archived_slack_message;
pub use archive_runtime::{archived_workspace_for_conversation, SlackArchiveWorkspaceRuntime};
pub use cache::{default_slack_cache_root_dir, SlackWorkspaceCacheStore};
pub use connection::production_slack_connection_api;
pub use internal_sidebar::SlackWebSessionCredentials;
pub use local_file::production_slack_local_file_api;
pub use payload::{
    load_slack_live_workspace, load_slack_remote_image, SlackAttachmentPreview,
    SlackLiveWorkspaceLoader,
};
pub use runtime::SlackWorkspaceRuntime;
pub use runtime_registry::{
    SlackHostEvent, SlackHostEventStream, SlackHostRuntime, SlackNotificationReadReceipt,
    SlackNotificationReplyRequest,
};
#[cfg(target_os = "macos")]
pub use slack_auth::check_slack_desktop_app;
pub use slack_auth::{
    authenticated_slack_teams, capture_slack_desktop_credentials, connect_slack_desktop,
    default_slack_launch_context, remember_slack_active_team, remember_slack_team_conversation,
    resolve_slack_channel, should_recapture_slack_desktop_session, slack_live_loader,
    SlackAuthenticatedTeam, SlackDesktopIntegrationStatus, SlackDesktopIntegrationUnavailable,
    SlackLiveLaunchContext, SlackNotificationAudioPreferences, SlackNotificationPlayback,
    SlackWebBuildTimestamp,
};
pub use slack_connection::{
    SlackDesktopRecovery, SLACK_DESKTOP_CONNECTION_DISCLOSURE, SLACK_DESKTOP_CONNECTION_SUCCESS,
};
pub fn default_launch_route() -> Result<crate::model::ChatLaunchRoute, String> {
    let launch = default_slack_launch_context()?;
    crate::model::ChatLaunchRoute::try_from_parts(launch.team_id, launch.channel_id, launch.tab_id)
}

pub fn load_live_workspace(
    route: &crate::model::ChatLaunchRoute,
) -> Result<crate::model::SlackWorkspace, String> {
    slack_live_loader(route.team_id().as_str())?.load_workspace(route.channel_id().as_str())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLiveConfig {
    pub workspace_url: String,
}
