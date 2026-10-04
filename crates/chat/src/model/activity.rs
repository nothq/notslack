use serde::{Deserialize, Deserializer, Serialize};

use crate::model::{SlackMessage, SlackMessageTimestamp};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SlackActivityCursor(String);

impl SlackActivityCursor {
    pub fn new(value: String) -> Result<Self, String> {
        if value.is_empty() {
            return Err("Slack Activity cursor must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackActivityCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivitySnapshot {
    pub activity_views_date_updated: String,
    pub items: Vec<SlackActivityItem>,
    pub next_cursor: Option<SlackActivityCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityItem {
    pub key: String,
    pub feed_timestamp: String,
    pub version: String,
    pub archived: bool,
    pub unread: bool,
    pub bot: Option<bool>,
    pub content: SlackActivityContent,
}

impl SlackActivityItem {
    pub fn read_target(&self) -> Option<SlackActivityReadTarget> {
        let (kind, timestamp) = match &self.content {
            SlackActivityContent::ThreadV2(activity) => {
                (SlackActivityReadKind::ThreadV2, &activity.latest_timestamp)
            }
            SlackActivityContent::AtUser(activity) => {
                (SlackActivityReadKind::AtUser, &activity.message_timestamp)
            }
            SlackActivityContent::Dm(activity) => (
                SlackActivityReadKind::Dm,
                &activity.latest_message_timestamp,
            ),
            SlackActivityContent::BotDmBundle(activity) => (
                SlackActivityReadKind::BotDmBundle,
                &activity.message_timestamp,
            ),
            SlackActivityContent::MessageReaction(_) | SlackActivityContent::Unsupported { .. } => {
                return None;
            }
        };
        Some(SlackActivityReadTarget {
            kind,
            timestamp: timestamp.clone(),
            feed_timestamp: self.feed_timestamp.clone(),
            key: self.key.clone(),
        })
    }

    pub fn archive_target(&self) -> Option<SlackActivityArchiveTarget> {
        let (kind, timestamp) = match &self.content {
            SlackActivityContent::MessageReaction(activity) => (
                SlackActivityArchiveKind::MessageReaction,
                &activity.message_timestamp,
            ),
            SlackActivityContent::ThreadV2(activity) => (
                SlackActivityArchiveKind::ThreadV2,
                &activity.latest_timestamp,
            ),
            SlackActivityContent::AtUser(activity) => (
                SlackActivityArchiveKind::AtUser,
                &activity.message_timestamp,
            ),
            SlackActivityContent::Dm(activity) => (
                SlackActivityArchiveKind::Dm,
                &activity.latest_message_timestamp,
            ),
            SlackActivityContent::BotDmBundle(activity) => (
                SlackActivityArchiveKind::BotDmBundle,
                &activity.message_timestamp,
            ),
            SlackActivityContent::Unsupported { .. } => return None,
        };
        Some(SlackActivityArchiveTarget {
            kind,
            key: self.key.clone(),
            timestamp: timestamp.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackActivityReadKind {
    ThreadV2,
    AtUser,
    Dm,
    BotDmBundle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackActivityReadTarget {
    kind: SlackActivityReadKind,
    timestamp: SlackMessageTimestamp,
    feed_timestamp: String,
    key: String,
}

impl SlackActivityReadTarget {
    pub fn kind(&self) -> SlackActivityReadKind {
        self.kind
    }

    pub fn feed_timestamp(&self) -> &str {
        &self.feed_timestamp
    }

    pub fn timestamp(&self) -> &SlackMessageTimestamp {
        &self.timestamp
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackActivityArchiveKind {
    MessageReaction,
    ThreadV2,
    AtUser,
    Dm,
    BotDmBundle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackActivityArchiveTarget {
    kind: SlackActivityArchiveKind,
    key: String,
    timestamp: SlackMessageTimestamp,
}

impl SlackActivityArchiveTarget {
    pub fn kind(&self) -> SlackActivityArchiveKind {
        self.kind
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn timestamp(&self) -> &SlackMessageTimestamp {
        &self.timestamp
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum SlackActivityContent {
    MessageReaction(SlackActivityMessageReaction),
    ThreadV2(SlackActivityThreadV2),
    AtUser(SlackActivityAtUser),
    Dm(SlackActivityDm),
    BotDmBundle(SlackActivityBotDmBundle),
    Unsupported { kind: String },
}

impl SlackActivityContent {
    pub fn message(&self) -> Option<&SlackMessage> {
        match self {
            Self::MessageReaction(activity) => Some(&activity.message),
            Self::ThreadV2(activity) => Some(&activity.message),
            Self::AtUser(activity) => Some(&activity.message),
            Self::Dm(activity) => Some(&activity.message),
            Self::BotDmBundle(activity) => Some(&activity.message),
            Self::Unsupported { .. } => None,
        }
    }

    pub fn message_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        match self {
            Self::MessageReaction(activity) => Some(&activity.message_timestamp),
            Self::ThreadV2(activity) => Some(&activity.latest_timestamp),
            Self::AtUser(activity) => Some(&activity.message_timestamp),
            Self::Dm(activity) => Some(&activity.latest_message_timestamp),
            Self::BotDmBundle(activity) => Some(&activity.message_timestamp),
            Self::Unsupported { .. } => None,
        }
    }

    pub fn thread_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        match self {
            Self::MessageReaction(activity) => activity.thread_timestamp.as_ref(),
            Self::ThreadV2(activity) => activity.thread_timestamp.as_ref(),
            Self::AtUser(activity) => activity.thread_timestamp.as_ref(),
            Self::Dm(activity) => activity.thread_timestamp.as_ref(),
            Self::BotDmBundle(activity) => activity.thread_timestamp.as_ref(),
            Self::Unsupported { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityMessageReaction {
    pub channel_id: String,
    pub message_timestamp: SlackMessageTimestamp,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub message: SlackMessage,
    pub reaction_name: String,
    pub actor: SlackActivityActor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityActor {
    pub user_id: String,
    pub display_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityThreadV2 {
    pub channel_id: String,
    pub latest_timestamp: SlackMessageTimestamp,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub message: SlackMessage,
    pub unread_message_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityAtUser {
    pub channel_id: String,
    pub broadcast: bool,
    pub message_timestamp: SlackMessageTimestamp,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityDm {
    pub channel_id: String,
    pub latest_message_timestamp: SlackMessageTimestamp,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackActivityBotDmBundle {
    pub channel_id: String,
    pub message_timestamp: SlackMessageTimestamp,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub message: SlackMessage,
    pub unread_count: u32,
}
