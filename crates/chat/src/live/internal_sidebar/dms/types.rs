use crate::model::{SlackMessageTimestamp, SlackUserPresence};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::live::internal_sidebar::boot::SlackDirectMessagePresence;
use crate::live::payload::{
    select_slack_avatar_image_url, SlackAvatarImageUrls, SlackAvatarPurpose,
};

pub(super) fn decode_response<T>(method: &str, body: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de> + SlackInternalResponse,
{
    let response = serde_json::from_str::<T>(body)
        .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
    if !response.ok() {
        return Err(format!(
            "Slack {method} failed: {}",
            response.error().unwrap_or("unknown_error")
        ));
    }
    Ok(response)
}

pub(super) trait SlackInternalResponse {
    fn ok(&self) -> bool;
    fn error(&self) -> Option<&str>;
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackClientDmsResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    pub(super) ims: Vec<SlackClientDm>,
    #[serde(default)]
    pub(super) mpims: Vec<SlackClientDm>,
    #[serde(default)]
    pub(super) response_metadata: SlackResponseMetadata,
}

impl SlackInternalResponse for SlackClientDmsResponse {
    fn ok(&self) -> bool {
        self.ok
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct SlackClientDm {
    pub(super) id: String,
    pub(super) latest: String,
    pub(super) message: SlackDmMessage,
    pub(super) channel: SlackDmChannel,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct SlackDmMessage {
    #[serde(default)]
    pub(super) user: Option<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    blocks: Vec<Value>,
    #[serde(default)]
    pub(super) files: Vec<Value>,
    #[serde(default)]
    pub(super) attachments: Vec<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackDmChannel {
    #[serde(default)]
    pub(super) user: Option<String>,
    #[serde(default)]
    pub(super) members: Vec<String>,
    #[serde(default)]
    pub(super) last_read: Option<String>,
    #[serde(default)]
    pub(super) is_mpim: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct SlackClientCountsResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    channels: Vec<SlackDmCount>,
    #[serde(default)]
    ims: Vec<SlackDmCount>,
    #[serde(default)]
    mpims: Vec<SlackDmCount>,
}

impl SlackClientCountsResponse {
    pub(super) fn conversations(&self) -> impl Iterator<Item = &SlackDmCount> {
        self.channels
            .iter()
            .chain(self.ims.iter())
            .chain(self.mpims.iter())
    }
}

impl SlackInternalResponse for SlackClientCountsResponse {
    fn ok(&self) -> bool {
        self.ok
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct SlackDmCount {
    pub(super) id: String,
    #[serde(default)]
    unread_count: Option<u32>,
    #[serde(default)]
    unread_count_display: Option<u32>,
    #[serde(default)]
    mention_count: Option<u32>,
    #[serde(default)]
    has_unreads: Option<bool>,
    #[serde(default)]
    is_unread: Option<bool>,
    #[serde(default)]
    latest: Option<String>,
    #[serde(default)]
    last_read: Option<String>,
}

impl SlackDmCount {
    pub(super) fn unread(&self) -> bool {
        self.unread_count.unwrap_or_default() > 0
            || self.unread_count_display.unwrap_or_default() > 0
            || self.mention_count.unwrap_or_default() > 0
            || self.has_unreads == Some(true)
            || self.is_unread == Some(true)
            || self
                .latest
                .as_deref()
                .is_some_and(|latest| timestamp_after(latest, self.last_read.as_deref()))
    }

    pub(super) fn display_count(&self) -> Option<u32> {
        self.unread_count_display
            .filter(|count| *count > 0)
            .or_else(|| self.mention_count.filter(|count| *count > 0))
    }

    pub(super) fn latest_message_timestamp(&self) -> Result<Option<SlackMessageTimestamp>, String> {
        self.latest
            .as_deref()
            .map(SlackMessageTimestamp::parse)
            .transpose()
            .map_err(|error| format!("Slack client.counts latest for {}: {error}", self.id))
    }
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackUsersListResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    pub(super) members: Vec<SlackDmUser>,
    #[serde(default)]
    pub(super) response_metadata: SlackResponseMetadata,
}

impl SlackInternalResponse for SlackUsersListResponse {
    fn ok(&self) -> bool {
        self.ok
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct SlackDmUser {
    pub(super) id: String,
    team_id: String,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    presence: Option<String>,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    is_bot: bool,
    #[serde(default)]
    is_app_user: bool,
    #[serde(default)]
    tz: Option<String>,
    #[serde(default)]
    tz_label: Option<String>,
    #[serde(default)]
    profile: SlackDmUserProfile,
}

impl SlackDmUser {
    pub(super) fn display_name(&self) -> String {
        [
            self.profile.real_name.as_deref(),
            self.real_name.as_deref(),
            self.profile.display_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .unwrap_or(self.id.as_str())
        .to_string()
    }

    pub(super) fn avatar_image_url(&self, purpose: SlackAvatarPurpose) -> Option<String> {
        select_slack_avatar_image_url(
            purpose,
            SlackAvatarImageUrls {
                image_24: self.profile.image_24.as_deref(),
                image_32: self.profile.image_32.as_deref(),
                image_48: self.profile.image_48.as_deref(),
                image_72: self.profile.image_72.as_deref(),
                image_192: self.profile.image_192.as_deref(),
                image_512: self.profile.image_512.as_deref(),
            },
        )
    }

    pub(super) fn presence(&self) -> Option<SlackUserPresence> {
        match self.presence.as_deref() {
            Some("active") => Some(SlackUserPresence::Active),
            Some("away") => Some(SlackUserPresence::Away),
            _ => None,
        }
    }

    pub(super) fn sidebar_presence(&self) -> Result<Option<SlackDirectMessagePresence>, String> {
        self.presence
            .as_deref()
            .map(|presence| SlackDirectMessagePresence::parse(presence, self.id.as_str()))
            .transpose()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SlackDmUserProfile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status_text: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    image_24: Option<String>,
    #[serde(default)]
    image_32: Option<String>,
    #[serde(default)]
    image_48: Option<String>,
    #[serde(default)]
    image_72: Option<String>,
    #[serde(default)]
    image_192: Option<String>,
    #[serde(default)]
    image_512: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackResponseMetadata {
    #[serde(default)]
    pub(super) next_cursor: String,
}

pub(super) fn timestamp_after(latest: &str, last_read: Option<&str>) -> bool {
    slack_timestamp_sort_key(latest) > slack_timestamp_sort_key(last_read.unwrap_or_default())
}

pub(super) fn slack_timestamp_sort_key(timestamp: &str) -> u128 {
    timestamp
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse::<u128>()
        .unwrap_or_default()
}

pub(super) fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}
