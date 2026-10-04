use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref};

use super::SlackMessage;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackMessageTimestamp {
    value: String,
    seconds: u64,
    nanoseconds: u32,
}

impl SlackMessageTimestamp {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (seconds, fractional_seconds) = value.split_once('.').ok_or_else(|| {
            "Slack message timestamp must contain a fractional separator".to_string()
        })?;
        if seconds.is_empty()
            || fractional_seconds.is_empty()
            || fractional_seconds.len() > 9
            || !seconds.bytes().all(|byte| byte.is_ascii_digit())
            || !fractional_seconds.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "Slack message timestamp must use seconds.fractional-seconds digits".to_string(),
            );
        }
        let seconds = seconds
            .parse::<u64>()
            .map_err(|_| "Slack message timestamp seconds exceed u64".to_string())?;
        let fractional_digits = fractional_seconds.len();
        let fractional_seconds = fractional_seconds
            .parse::<u32>()
            .map_err(|_| "Slack message timestamp fractional seconds exceed u32".to_string())?;
        let nanoseconds = fractional_seconds
            .checked_mul(
                10_u32.pow(
                    u32::try_from(9 - fractional_digits)
                        .expect("validated Slack message timestamp precision must fit u32"),
                ),
            )
            .ok_or_else(|| {
                "Slack message timestamp fractional seconds exceed nanoseconds".to_string()
            })?;
        Ok(Self {
            value: value.to_string(),
            seconds,
            nanoseconds,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn sort_key(&self) -> (u64, u32) {
        (self.seconds, self.nanoseconds)
    }
}

impl Deref for SlackMessageTimestamp {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for SlackMessageTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for SlackMessageTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for SlackMessageTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackLastReadTimestamp {
    value: String,
    seconds: u64,
    nanoseconds: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackConversationReadReceipt {
    pub team_id: String,
    pub conversation_id: String,
    pub last_read: SlackLastReadTimestamp,
}

impl SlackLastReadTimestamp {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (seconds, fractional) = value.split_once('.').ok_or_else(|| {
            "Slack last_read timestamp must contain a fractional separator".to_string()
        })?;
        if seconds.is_empty()
            || fractional.is_empty()
            || fractional.len() > 9
            || !seconds.bytes().all(|byte| byte.is_ascii_digit())
            || !fractional.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(
                "Slack last_read timestamp must use seconds.fractional-seconds digits".to_string(),
            );
        }
        let seconds = seconds
            .parse::<u64>()
            .map_err(|_| "Slack last_read timestamp seconds exceed u64".to_string())?;
        let fractional_digits = fractional.len();
        let fractional = fractional
            .parse::<u32>()
            .map_err(|_| "Slack last_read timestamp fractional seconds exceed u32".to_string())?;
        let nanoseconds = fractional
            .checked_mul(
                10_u32.pow(
                    u32::try_from(9 - fractional_digits)
                        .expect("validated Slack last_read precision must fit u32"),
                ),
            )
            .ok_or_else(|| {
                "Slack last_read timestamp fractional seconds exceed nanoseconds".to_string()
            })?;
        Ok(Self {
            value: value.to_string(),
            seconds,
            nanoseconds,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn sort_key(&self) -> (u64, u32) {
        (self.seconds, self.nanoseconds)
    }
}

impl Serialize for SlackLastReadTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.value)
    }
}

impl<'de> Deserialize<'de> for SlackLastReadTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackConversationHistoryCursor(String);

impl SlackConversationHistoryCursor {
    pub fn new(value: String) -> Result<Self, String> {
        if value.is_empty() {
            return Err("Slack conversation history cursor must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackConversationHistoryCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationHistoryPage {
    pub team_id: String,
    pub conversation_id: String,
    pub messages: Vec<SlackMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<SlackConversationHistoryCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackThreadSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    pub conversation_name: String,
    pub thread_timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SlackMessage>,
    #[serde(default)]
    pub replies: Vec<SlackMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackThreadReadMetadata {
    pub subscribed: bool,
    pub last_read: Option<SlackMessageTimestamp>,
    pub unread_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackThreadLoad {
    pub snapshot: SlackThreadSnapshot,
    pub read_metadata: Option<SlackThreadReadMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackThreadReadTarget {
    conversation_id: String,
    thread_timestamp: SlackMessageTimestamp,
    message_timestamp: SlackMessageTimestamp,
}

impl SlackThreadReadTarget {
    pub fn new(
        conversation_id: impl Into<String>,
        thread_timestamp: SlackMessageTimestamp,
        message_timestamp: SlackMessageTimestamp,
    ) -> Result<Self, String> {
        let conversation_id = conversation_id.into();
        if conversation_id.trim().is_empty() {
            return Err("Slack thread read target requires a conversation id".to_string());
        }
        if message_timestamp.sort_key() <= thread_timestamp.sort_key() {
            return Err(
                "Slack thread read target must identify a reply after its parent".to_string(),
            );
        }
        Ok(Self {
            conversation_id,
            thread_timestamp,
            message_timestamp,
        })
    }

    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn thread_timestamp(&self) -> &SlackMessageTimestamp {
        &self.thread_timestamp
    }

    pub fn message_timestamp(&self) -> &SlackMessageTimestamp {
        &self.message_timestamp
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackThreadReplyReceipt {
    pub team_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    pub thread_timestamp: String,
    #[serde(default)]
    pub broadcast: bool,
    pub reply: SlackMessage,
}

/// Where a thread reply is posted, and whether it is also broadcast to its conversation.
#[derive(Clone, Copy, Debug)]
pub struct SlackThreadReplyTarget<'a> {
    pub conversation_id: &'a str,
    pub thread_timestamp: &'a SlackMessageTimestamp,
    pub broadcast: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMessageSendReceipt {
    pub team_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_timezone_id: Option<String>,
    pub timestamp: String,
    pub self_user_id: String,
    pub message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackMessageForwardReceipt {
    pub destination_conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}
