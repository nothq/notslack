mod read_receipts;

use serde::{Deserialize, Serialize};

use crate::model::{SlackAttachment, SlackReaction};

use super::{
    SlackConversationHistoryCursor, SlackConversationKind, SlackConversationTab,
    SlackDirectMessageUnreadState, SlackDmPeerLocalTimeContext, SlackEmojiPickerSection,
    SlackLastReadTimestamp, SlackMentionSuggestion, SlackMessage, SlackMessageTimestamp,
    SlackRailBadges, SlackSidebarSection, SlackUserPresence,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackWorkspace {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub team_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub conversation_id: String,
    #[serde(default)]
    pub channel_kind: SlackConversationKind,
    pub workspace_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_label: Option<String>,
    pub channel_name: String,
    pub channel_topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<SlackConversationTab>,
    #[serde(default)]
    pub sections: Vec<SlackSidebarSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct_message_unread_states: Vec<SlackDirectMessageUnreadState>,
    #[serde(default)]
    pub rail_badges: SlackRailBadges,
    #[serde(default)]
    pub messages: Vec<SlackMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read: Option<SlackLastReadTimestamp>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub last_read_boundary_loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_next_cursor: Option<SlackConversationHistoryCursor>,
    #[serde(default)]
    pub mention_suggestions: Vec<SlackMentionSuggestion>,
    #[serde(default)]
    pub emoji_picker_sections: Vec<SlackEmojiPickerSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_notice: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub peer_notifications_paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_peer_local_time_context: Option<SlackDmPeerLocalTimeContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_draft_text: Option<String>,
    pub composer_placeholder: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackShellSnapshot {
    pub team_id: String,
    pub workspace_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_logo_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_avatar_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSidebarSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub active_conversation_kind: SlackConversationKind,
    pub sections: Vec<SlackSidebarSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct_message_unread_states: Vec<SlackDirectMessageUnreadState>,
    pub rail_badges: SlackRailBadges,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDmInboxSnapshot {
    pub team_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub self_user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slackbot_conversation_id: Option<String>,
    #[serde(default)]
    pub items: Vec<SlackDmInboxItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDmInboxItem {
    pub conversation_id: String,
    pub kind: SlackConversationKind,
    pub label: String,
    #[serde(default)]
    pub participants: Vec<SlackDmParticipant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_sender_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_sender_label: Option<String>,
    #[serde(default)]
    pub latest_message_text: String,
    pub latest_timestamp: SlackMessageTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unread_state_latest_timestamp: Option<SlackMessageTimestamp>,
    #[serde(default)]
    pub unread: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDmParticipant {
    pub user_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<SlackUserPresence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSearchSnapshot {
    pub query: String,
    pub total: u32,
    pub messages: Vec<SlackSearchMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackSearchMessage {
    pub id: String,
    pub team_id: String,
    pub conversation_id: String,
    pub conversation_name: String,
    pub conversation_kind: SlackConversationKind,
    pub user_id: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_timestamp: Option<String>,
    pub body: String,
    pub permalink: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<SlackAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<SlackReaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_reply_timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reply_user_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reply_user_avatar_image_urls: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    pub channel_kind: SlackConversationKind,
    pub channel_name: String,
    pub channel_topic: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<SlackConversationTab>,
    #[serde(default)]
    pub messages: Vec<SlackMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_read: Option<SlackLastReadTimestamp>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub last_read_boundary_loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_next_cursor: Option<SlackConversationHistoryCursor>,
    #[serde(default)]
    pub mention_suggestions: Vec<SlackMentionSuggestion>,
    #[serde(default)]
    pub emoji_picker_sections: Vec<SlackEmojiPickerSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_notice: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub peer_notifications_paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_peer_local_time_context: Option<SlackDmPeerLocalTimeContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_draft_text: Option<String>,
    pub composer_placeholder: String,
}

fn is_false(value: &bool) -> bool {
    !value
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}
