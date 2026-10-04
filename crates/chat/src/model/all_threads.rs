use serde::{Deserialize, Deserializer, Serialize};

use crate::model::{
    SlackConversationKind, SlackIanaTimezone, SlackMessage, SlackMessageTimestamp,
    SlackUserPresence,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SlackAllThreadsCursor(String);

impl SlackAllThreadsCursor {
    pub fn new(value: String) -> Result<Self, String> {
        validate_message_timestamp(&value, "Slack All Threads cursor")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackAllThreadsCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAllThread {
    pub id: String,
    pub conversation_id: String,
    pub conversation_name: String,
    pub conversation_kind: SlackConversationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_message_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_message_presence: Option<SlackUserPresence>,
    pub thread_timestamp: String,
    pub parent: SlackMessage,
    pub visible_replies: Vec<SlackMessage>,
    pub reply_count: u32,
    pub unread_reply_timestamps: Vec<SlackMessageTimestamp>,
    pub participant_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAllThreadsSnapshot {
    pub team_id: String,
    pub timezone: SlackIanaTimezone,
    pub threads: Vec<SlackAllThread>,
    pub next_cursor: Option<SlackAllThreadsCursor>,
    pub total_unread_replies: Option<u32>,
}

fn validate_message_timestamp(value: &str, label: &str) -> Result<(), String> {
    let (seconds, fractional_seconds) = value
        .split_once('.')
        .ok_or_else(|| format!("{label} must contain a fractional separator"))?;
    if seconds.is_empty()
        || fractional_seconds.is_empty()
        || fractional_seconds.len() > 9
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional_seconds.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "{label} must use seconds.fractional-seconds digits"
        ));
    }
    Ok(())
}
