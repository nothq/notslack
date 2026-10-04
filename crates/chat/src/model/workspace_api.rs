use std::sync::Arc;

mod conversation;
mod discovery;
mod messages;

use conversation::slack_workspace_conversation_api;
use discovery::slack_workspace_discovery_api;
use messages::slack_workspace_message_api;

pub type SlackRemoteImagePrefetchRequest = (Arc<dyn SlackWorkspaceApi>, Vec<String>);

pub trait SlackWorkspaceApi: Send + Sync + 'static {
    slack_workspace_discovery_api!();
    slack_workspace_conversation_api!();
    slack_workspace_message_api!();
}
