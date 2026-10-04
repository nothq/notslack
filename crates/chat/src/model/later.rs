use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::model::{SlackAttachment, SlackConversationKind, SlackMessage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterFilter {
    #[default]
    Saved,
    Archived,
    Completed,
}

impl SlackLaterFilter {
    pub const ALL: [Self; 3] = [Self::Saved, Self::Archived, Self::Completed];

    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Archived => "archived",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackLaterCursor(String);

impl SlackLaterCursor {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            return Err("Slack Later cursor must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackLaterItemKey(String);

impl SlackLaterItemKey {
    pub fn new(value: String) -> Result<Self, String> {
        if value.is_empty()
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        {
            return Err("Slack Later item key must be non-empty and contain no whitespace".into());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterState {
    InProgress,
    Archived,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterDates {
    pub created: u64,
    pub updated: u64,
    pub due: u64,
    pub snoozed_until: u64,
    pub completed: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterCounts {
    pub in_progress: u32,
    pub overdue: u32,
    pub archived: u32,
    pub completed: u32,
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackLaterMessageReference {
    pub conversation_id: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackLaterFileReference {
    pub file_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SlackReminderId(String);

impl SlackReminderId {
    pub fn parse(value: String) -> Result<Self, String> {
        if !value.starts_with("Sa")
            || value.len() < 3
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(format!("invalid Slack reminder id: {value}"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackReminderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackReminderClientId(Uuid);

impl SlackReminderClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_hyphenated_string(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl Default for SlackReminderClientId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SlackLaterReminder {
    reminder_id: SlackReminderId,
    description: String,
}

impl SlackLaterReminder {
    pub fn new(reminder_id: SlackReminderId, description: String) -> Result<Self, String> {
        let description = description.trim().to_string();
        if description.is_empty() {
            return Err("Slack reminder description must not be empty".to_string());
        }
        Ok(Self {
            reminder_id,
            description,
        })
    }

    pub fn reminder_id(&self) -> &SlackReminderId {
        &self.reminder_id
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

impl<'de> Deserialize<'de> for SlackLaterReminder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireReminder {
            reminder_id: SlackReminderId,
            description: String,
        }

        let reminder = WireReminder::deserialize(deserializer)?;
        Self::new(reminder.reminder_id, reminder.description).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackReminderDraft {
    description: String,
    due_at: u64,
}

impl SlackReminderDraft {
    pub fn new(description: String, due_at: u64) -> Result<Self, String> {
        let description = description.trim().to_string();
        if description.is_empty() {
            return Err("Reminder text is required.".to_string());
        }
        if due_at == 0 {
            return Err("Reminder due time is required.".to_string());
        }
        Ok(Self {
            description,
            due_at,
        })
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn due_at(&self) -> u64 {
        self.due_at
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackReminderMutation {
    Create {
        client_id: SlackReminderClientId,
        draft: SlackReminderDraft,
    },
    Edit {
        reminder_id: SlackReminderId,
        draft: SlackReminderDraft,
    },
    Complete {
        reminder_id: SlackReminderId,
    },
    Delete {
        reminder_id: SlackReminderId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterTombstoneKind {
    Message,
    File,
    Reminder,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterReferenceContent {
    Message(SlackLaterMessageReference),
    File(SlackLaterFileReference),
    Reminder(SlackLaterReminder),
    Tombstone { kind: SlackLaterTombstoneKind },
    Unsupported { kind: String, item_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterItem {
    pub key: SlackLaterItemKey,
    pub item_id: String,
    pub state: SlackLaterState,
    pub dates: SlackLaterDates,
    pub content: SlackLaterReferenceContent,
}

impl SlackLaterItem {
    pub fn hydration_target(&self) -> Option<SlackLaterHydrationTarget> {
        match &self.content {
            SlackLaterReferenceContent::Message(reference) => {
                Some(SlackLaterHydrationTarget::Message {
                    key: self.key.clone(),
                    reference: reference.clone(),
                })
            }
            SlackLaterReferenceContent::File(reference) => Some(SlackLaterHydrationTarget::File {
                key: self.key.clone(),
                reference: reference.clone(),
            }),
            SlackLaterReferenceContent::Reminder(_)
            | SlackLaterReferenceContent::Tombstone { .. }
            | SlackLaterReferenceContent::Unsupported { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterHydrationTarget {
    Message {
        key: SlackLaterItemKey,
        reference: SlackLaterMessageReference,
    },
    File {
        key: SlackLaterItemKey,
        reference: SlackLaterFileReference,
    },
}

impl SlackLaterHydrationTarget {
    pub fn key(&self) -> &SlackLaterItemKey {
        match self {
            Self::Message { key, .. } | Self::File { key, .. } => key,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterMessage {
    pub conversation_id: String,
    pub conversation_kind: SlackConversationKind,
    pub conversation_name: String,
    pub timestamp: String,
    pub thread_timestamp: String,
    pub permalink: String,
    pub message: SlackMessage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterFile {
    pub file_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_label: Option<String>,
    pub attachment: SlackAttachment,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackLaterContent {
    Message(Box<SlackLaterMessage>),
    File(Box<SlackLaterFile>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterHydratedItem {
    pub key: SlackLaterItemKey,
    pub content: SlackLaterContent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLaterSnapshot {
    pub filter: SlackLaterFilter,
    pub items: Vec<SlackLaterItem>,
    pub counts: SlackLaterCounts,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<SlackLaterCursor>,
}
