use serde::{Deserialize, Serialize};

use crate::model::SlackMessageTimestamp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackHuddleJoinRequest {
    pub conversation_id: String,
    pub room_id: Option<String>,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub region: String,
    pub multidevice: bool,
}

impl SlackHuddleJoinRequest {
    pub fn for_conversation(conversation_id: impl Into<String>) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            room_id: None,
            thread_timestamp: None,
            region: String::new(),
            multidevice: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackHuddleLeaveRequest {
    pub conversation_id: String,
    pub call_id: String,
    pub attendee_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackHuddleJoinReceipt {
    pub call_id: String,
    pub conversation_id: Option<String>,
    pub root_thread_timestamp: Option<SlackMessageTimestamp>,
    pub canvas_file_id: Option<String>,
    pub media: SlackHuddleMediaCredentials,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackHuddleMediaCredentials {
    pub meeting: SlackChimeMeeting,
    pub attendee: SlackChimeAttendee,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SlackChimeMeeting {
    pub meeting_id: String,
    pub external_meeting_id: String,
    pub media_region: String,
    pub media_placement: SlackChimeMediaPlacement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SlackChimeMediaPlacement {
    pub audio_host_url: String,
    pub signaling_url: String,
    pub turn_control_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_fallback_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_ingestion_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_sharing_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_viewing_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SlackChimeAttendee {
    pub attendee_id: String,
    pub external_user_id: String,
    pub join_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<SlackChimeAttendeeCapabilities>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SlackChimeAttendeeCapabilities {
    pub audio: String,
    pub video: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackHuddlePhase {
    NotStarted,
    Starting {
        generation: u64,
        conversation_id: String,
    },
    Pending {
        generation: u64,
        receipt: SlackHuddleJoinReceipt,
    },
    Started {
        generation: u64,
        receipt: SlackHuddleJoinReceipt,
    },
    Ending {
        generation: u64,
        receipt: SlackHuddleJoinReceipt,
    },
    Survey {
        conversation_id: String,
    },
}

impl SlackHuddlePhase {
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::NotStarted | Self::Survey { .. })
    }

    pub fn generation(&self) -> Option<u64> {
        match self {
            Self::Starting { generation, .. }
            | Self::Pending { generation, .. }
            | Self::Started { generation, .. }
            | Self::Ending { generation, .. } => Some(*generation),
            Self::NotStarted | Self::Survey { .. } => None,
        }
    }
}
