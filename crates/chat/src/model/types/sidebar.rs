use serde::{Deserialize, Serialize};

use super::{SlackConversationKind, SlackMessageTimestamp, SlackUserPresence};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackWorkspaceShell {
    pub team_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSidebarSection {
    pub label: String,
    #[serde(default)]
    pub items: Vec<SlackSidebarItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDirectMessageUnreadState {
    pub conversation_id: String,
    #[serde(default)]
    pub unread: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_message_timestamp: Option<SlackMessageTimestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSidebarItem {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_context: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_external_connection: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target_id: String,
    #[serde(default)]
    pub target_kind: SlackConversationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<SlackUserPresence>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub unread: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_message_timestamp: Option<SlackMessageTimestamp>,
    #[serde(default)]
    pub muted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

fn is_false(value: &bool) -> bool {
    !value
}
