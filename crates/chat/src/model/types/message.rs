use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::model::{SlackAttachment, SlackLaterState};

use super::SlackRichTextBody;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackMessageClientId(String);

impl SlackMessageClientId {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            return Err("Slack client message id must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().hyphenated().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackMessageClientId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMessage {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_message_id: Option<SlackMessageClientId>,
    pub author: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_mimetype: Option<String>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_body: Option<Box<SlackRichTextBody>>,
    #[serde(default)]
    pub table_rows: Vec<SlackTableRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_divider_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_label: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::model::attachments::deserialize_slack_attachments"
    )]
    pub attachments: Vec<SlackAttachment>,
    #[serde(default)]
    pub reactions: Vec<SlackReaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_state: Option<SlackLaterState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_reply_timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reply_participants: Vec<SlackReplyParticipant>,
    #[serde(default)]
    pub replies: Vec<SlackMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackReplyParticipant {
    pub user_id: String,
    pub display_name: String,
    pub avatar_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMentionSuggestion {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_mimetype: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackEmojiPickerSection {
    pub title: String,
    #[serde(default)]
    pub rows: Vec<SlackEmojiPickerRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackEmojiPickerRow {
    pub shortcode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_mimetype: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackTableRow {
    pub index: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackReaction {
    pub emoji: String,
    pub count: u32,
    #[serde(default)]
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlackProfile {
    pub user_id: String,
    pub display_name: String,
    pub real_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_mimetype: Option<String>,
}
