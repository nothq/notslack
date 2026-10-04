use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::model::{files::SlackFileId, SlackRichTextBody};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackDraftsSentTab {
    #[default]
    Drafts,
    Scheduled,
    Sent,
}

impl SlackDraftsSentTab {
    pub const ALL: [Self; 3] = [Self::Drafts, Self::Scheduled, Self::Sent];

    pub fn label(self) -> &'static str {
        match self {
            Self::Drafts => "Drafts",
            Self::Scheduled => "Scheduled",
            Self::Sent => "Sent",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackDraftsSentCursor(String);

impl SlackDraftsSentCursor {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            return Err("Slack Drafts & sent cursor must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackDraftId(String);

impl SlackDraftId {
    pub fn parse(value: String) -> Result<Self, String> {
        if value.is_empty()
            || value.trim() != value
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(
                "Slack draft id must be non-empty and contain no whitespace or control characters"
                    .to_string(),
            );
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackDraftId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SlackDraftId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackDraftRevision(String);

impl SlackDraftRevision {
    pub fn parse(value: String) -> Result<Self, String> {
        if value.is_empty() || value.trim() != value {
            return Err("Slack draft revision must not be empty or padded".to_string());
        }
        let (seconds, fraction) = value
            .split_once('.')
            .map_or((value.as_str(), None), |(seconds, fraction)| {
                (seconds, Some(fraction))
            });
        if seconds.is_empty()
            || !seconds.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.is_some_and(|fraction| {
                fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(
                "Slack draft revision timestamp must be an unsigned decimal number".to_string(),
            );
        }
        if seconds.bytes().all(|byte| byte == b'0')
            && fraction.is_none_or(|fraction| fraction.bytes().all(|byte| byte == b'0'))
        {
            return Err("Slack draft revision timestamp must be positive".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackDraftRevision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SlackDraftRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackDraftClientMutationTimestamp(String);

impl SlackDraftClientMutationTimestamp {
    /// Captures the client clock Slack uses to version a drafts.update mutation.
    pub fn from_unix_milliseconds(unix_milliseconds: i64) -> Result<Self, String> {
        if unix_milliseconds <= 0 {
            return Err(
                "Slack draft client mutation timestamp must be a positive Unix millisecond value"
                    .to_string(),
            );
        }
        Ok(Self(unix_milliseconds.to_string()))
    }

    /// Converts Slack's authoritative second-based revision to the millisecond
    /// decimal representation required by drafts.delete without floating-point loss.
    pub fn from_loaded_revision(revision: &SlackDraftRevision) -> Self {
        let (seconds, fractional_seconds) = revision
            .as_str()
            .split_once('.')
            .map_or((revision.as_str(), ""), |(seconds, fraction)| {
                (seconds, fraction)
            });
        let millisecond_fraction_end = fractional_seconds.len().min(3);
        let mut milliseconds = String::with_capacity(revision.as_str().len() + 3);
        milliseconds.push_str(seconds);
        milliseconds.push_str(&fractional_seconds[..millisecond_fraction_end]);
        for _ in millisecond_fraction_end..3 {
            milliseconds.push('0');
        }
        let first_nonzero = milliseconds
            .bytes()
            .position(|byte| byte != b'0')
            .unwrap_or(milliseconds.len() - 1);
        let mut value = milliseconds.split_off(first_nonzero);
        let sub_millisecond = fractional_seconds[millisecond_fraction_end..].trim_end_matches('0');
        if !sub_millisecond.is_empty() {
            value.push('.');
            value.push_str(sub_millisecond);
        }
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackDraftClientMutationTimestamp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct SlackDraftTarget {
    draft_id: SlackDraftId,
    revision: SlackDraftRevision,
}

impl SlackDraftTarget {
    pub fn new(draft_id: SlackDraftId, revision: SlackDraftRevision) -> Self {
        Self { draft_id, revision }
    }

    pub fn draft_id(&self) -> &SlackDraftId {
        &self.draft_id
    }

    pub fn revision(&self) -> &SlackDraftRevision {
        &self.revision
    }
}

impl<'de> Deserialize<'de> for SlackDraftTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SlackDraftTargetWire {
            draft_id: SlackDraftId,
            revision: SlackDraftRevision,
        }

        let wire = SlackDraftTargetWire::deserialize(deserializer)?;
        Ok(Self::new(wire.draft_id, wire.revision))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDraftReceipt {
    pub target: SlackDraftTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackScheduledDraftReceipt {
    pub target: SlackDraftTarget,
    pub post_at_unix_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDraftsSentRequest {
    pub team_id: String,
    pub tab: SlackDraftsSentTab,
    pub cursor: Option<SlackDraftsSentCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDraftDestination {
    pub conversation_id: String,
    pub label: String,
    pub avatar_image_url: Option<String>,
    pub user_ids: Vec<String>,
    pub thread_timestamp: Option<String>,
    pub message_timestamp: Option<String>,
    pub broadcast: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDraftsSentItem {
    pub id: SlackDraftId,
    pub team_id: String,
    pub user_id: String,
    pub created_unix_seconds: u64,
    pub revision: SlackDraftRevision,
    pub scheduled_unix_seconds: u64,
    pub client_message_id: String,
    pub body: String,
    pub rich_body: Option<SlackRichTextBody>,
    pub destinations: Vec<SlackDraftDestination>,
    pub file_ids: Vec<SlackFileId>,
}

impl SlackDraftsSentItem {
    pub fn primary_destination(&self) -> Option<&SlackDraftDestination> {
        self.destinations.first()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDraftsSentSnapshot {
    pub team_id: String,
    pub tab: SlackDraftsSentTab,
    pub items: Vec<SlackDraftsSentItem>,
    pub next_cursor: Option<SlackDraftsSentCursor>,
}
