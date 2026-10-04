use serde::{Deserialize, Serialize};

use crate::model::{SlackAttachmentMediaKind, SlackConversationKind, SlackUserPresence};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMediaAttachmentSummary {
    pub attachment_id: String,
    pub file_id: String,
    pub title: String,
    pub kind: SlackAttachmentMediaKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatMediaPlaybackState {
    Loading {
        attachment_id: String,
        file_id: String,
        kind: SlackAttachmentMediaKind,
    },
    Video {
        attachment_id: String,
        file_id: String,
        loading: bool,
        is_playing: bool,
        current_millis: u64,
        duration_millis: Option<u64>,
        muted: bool,
        volume_percent: u8,
        error: Option<ChatMediaPlaybackError>,
    },
    Audio {
        attachment_id: String,
        file_id: String,
        playback: ChatAudioPlaybackState,
        current_millis: u64,
        duration_millis: Option<u64>,
        muted: bool,
        volume_percent: u8,
        error: Option<String>,
    },
    Failed {
        attachment_id: String,
        file_id: String,
        kind: SlackAttachmentMediaKind,
        error: ChatMediaPlaybackError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatAudioPlaybackState {
    Idle,
    Playing,
    Paused,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMediaPlaybackError {
    SourceUnavailable,
    UnsupportedFormat,
    Decoder,
    Preparation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageTiming {
    pub id: String,
    pub timestamp: String,
    pub latest_reply_timestamp: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageRowTiming {
    pub id: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatConversationSummary {
    pub id: String,
    pub label: String,
    pub kind: SlackConversationKind,
    pub user_id: Option<String>,
    pub presence: Option<SlackUserPresence>,
    pub count: Option<u32>,
}
