use serde::{Deserialize, Serialize};

use crate::model::SlackMessage;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPinsRequest {
    pub team_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPinnedMessage {
    pub id: String,
    pub conversation_id: String,
    pub created_unix_seconds: u64,
    pub created_by_user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_timestamp: Option<String>,
    pub message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPinnedFile {
    pub id: String,
    pub conversation_id: String,
    pub created_unix_seconds: u64,
    pub created_by_user_id: String,
    pub file_id: String,
    pub file_message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPinnedFileComment {
    pub id: String,
    pub conversation_id: String,
    pub created_unix_seconds: u64,
    pub created_by_user_id: String,
    pub file_id: String,
    pub comment_id: String,
    pub comment_message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SlackPinnedItem {
    Message(SlackPinnedMessage),
    File(SlackPinnedFile),
    FileComment(SlackPinnedFileComment),
}

impl SlackPinnedItem {
    pub fn id(&self) -> &str {
        match self {
            Self::Message(item) => &item.id,
            Self::File(item) => &item.id,
            Self::FileComment(item) => &item.id,
        }
    }

    pub fn message(&self) -> &SlackMessage {
        match self {
            Self::Message(item) => &item.message,
            Self::File(item) => &item.file_message,
            Self::FileComment(item) => &item.comment_message,
        }
    }

    pub fn accessibility_kind_label(&self) -> &'static str {
        match self {
            Self::Message(_) => "Pinned message",
            Self::File(_) => "Pinned file",
            Self::FileComment(_) => "Pinned file comment",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackPinsSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    pub items: Vec<SlackPinnedItem>,
}
