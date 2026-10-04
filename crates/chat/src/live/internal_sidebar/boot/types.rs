use std::collections::HashMap;

use crate::model::SlackUserPresence;
use serde::Deserialize;
use serde_json::Value;

use crate::live::payload::{
    select_slack_avatar_image_url, SlackAvatarImageUrls, SlackAvatarPurpose,
};

#[derive(Debug, Default, Deserialize)]
pub(in crate::live::internal_sidebar) struct SlackClientBootResponse {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default, rename = "self")]
    pub(super) self_user: Option<SlackBootSelfUser>,
    #[serde(default)]
    pub(super) dnd: Option<SlackBootDnd>,
    #[serde(default)]
    pub(super) channels: Vec<SlackBootConversation>,
    #[serde(default)]
    pub(super) ims: Vec<SlackBootConversation>,
    #[serde(default)]
    pub(super) mpims: Vec<SlackBootConversation>,
    #[serde(default)]
    pub(super) prefs: SlackClientPrefs,
    #[serde(default)]
    pub(super) channels_priority: HashMap<String, f64>,
    #[serde(default)]
    pub(super) starred: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlackBootSelfUser {
    pub(super) id: String,
    pub(super) is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlackBootDnd {
    pub(super) dnd_enabled: bool,
    pub(super) snooze_enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
pub(in crate::live::internal_sidebar) struct SlackClientCountsResponse {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) activity_v2: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub(super) channels: Vec<SlackCountConversation>,
    #[serde(default)]
    pub(super) ims: Vec<SlackCountConversation>,
    #[serde(default)]
    pub(super) mpims: Vec<SlackCountConversation>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackClientPrefs {
    #[serde(default)]
    pub(super) channel_sort: Option<String>,
    #[serde(default)]
    pub(super) separate_shared_channels: Option<bool>,
    #[serde(default)]
    pub(super) separate_private_channels: Option<bool>,
    #[serde(default)]
    pub(super) sidebar_behavior: Option<String>,
    #[serde(default)]
    pub(super) hide_muted_channels_from_sidebar: Option<bool>,
    #[serde(default)]
    pub(super) remove_sidebar_customizations: Option<bool>,
    #[serde(default)]
    pub(super) undo_channel_intermix: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackBootConversation {
    pub(super) id: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) name_normalized: Option<String>,
    #[serde(default)]
    pub(super) user: Option<String>,
    #[serde(default)]
    pub(super) is_channel: bool,
    #[serde(default)]
    pub(super) is_group: bool,
    #[serde(default)]
    pub(super) is_private: bool,
    #[serde(default)]
    pub(super) is_im: bool,
    #[serde(default)]
    pub(super) is_mpim: bool,
    #[serde(default)]
    pub(super) is_shared: bool,
    #[serde(default)]
    pub(super) is_ext_shared: bool,
    #[serde(default)]
    pub(super) is_org_shared: bool,
    #[serde(default)]
    pub(super) context_team_id: Option<String>,
    #[serde(default)]
    pub(super) connected_team_ids: Vec<String>,
    #[serde(default)]
    pub(super) members: Vec<String>,
    #[serde(default)]
    pub(super) is_archived: bool,
    #[serde(default)]
    pub(super) is_general: bool,
    #[serde(default)]
    pub(super) unlinked: u64,
    #[serde(default)]
    pub(super) properties: SlackBootConversationProperties,
    #[serde(default)]
    pub(super) purpose: SlackBootTextField,
    #[serde(default)]
    pub(super) topic: SlackBootTextField,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackBootConversationProperties {
    #[serde(default)]
    pub(super) is_dormant: bool,
    #[serde(default)]
    pub(super) meeting_notes: Option<SlackBootMeetingNotes>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackBootTextField {
    #[serde(default)]
    pub(super) value: String,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackBootMeetingNotes {}

#[derive(Debug, Deserialize)]
pub(super) struct SlackCountConversation {
    pub(super) id: String,
    #[serde(default)]
    pub(super) unread_count: Option<Value>,
    #[serde(default)]
    pub(super) unread_count_display: Option<Value>,
    #[serde(default)]
    pub(super) mention_count: Option<Value>,
    #[serde(default)]
    pub(super) has_unreads: Option<bool>,
    #[serde(default)]
    pub(super) is_unread: Option<bool>,
    #[serde(default)]
    pub(super) latest: Option<Value>,
    #[serde(default)]
    pub(super) last_read: Option<Value>,
}

pub(in crate::live::internal_sidebar) struct SlackBootState {
    pub(super) boot: SlackClientBootResponse,
    pub(super) counts: SlackClientCountsResponse,
    pub(super) dm_users: HashMap<String, SlackDirectMessageUser>,
    pub(super) group_message_labels: HashMap<String, String>,
    pub(super) self_presence: Option<SlackDirectMessagePresence>,
    pub(super) self_notifications_paused: bool,
}

pub(super) struct SlackDirectMessageUser {
    pub(super) user_id: String,
    pub(super) label: String,
    pub(super) avatar_image_url: Option<String>,
    pub(super) secondary_context: Option<String>,
    pub(super) presence: Option<SlackDirectMessagePresence>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::live::internal_sidebar) enum SlackDirectMessagePresence {
    Active,
    Away,
}

impl SlackDirectMessagePresence {
    pub(super) fn as_model(self) -> SlackUserPresence {
        match self {
            Self::Active => SlackUserPresence::Active,
            Self::Away => SlackUserPresence::Away,
        }
    }

    pub(in crate::live::internal_sidebar) fn parse(
        value: &str,
        user_id: &str,
    ) -> Result<Self, String> {
        match value {
            "active" => Ok(Self::Active),
            "away" => Ok(Self::Away),
            _ => Err(format!(
                "unsupported Slack presence value for {user_id}: {value}"
            )),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackUserInfoResponse {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) user: Option<SlackUserInfoUser>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackUserInfoUser {
    #[serde(default)]
    pub(super) real_name: Option<String>,
    #[serde(default)]
    pub(super) profile: SlackUserInfoProfile,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackUserInfoProfile {
    #[serde(default)]
    pub(super) real_name: Option<String>,
    #[serde(default)]
    pub(super) display_name: Option<String>,
    #[serde(default)]
    pub(super) image_24: Option<String>,
    #[serde(default)]
    pub(super) image_32: Option<String>,
    #[serde(default)]
    pub(super) image_48: Option<String>,
    #[serde(default)]
    pub(super) image_72: Option<String>,
    #[serde(default)]
    pub(super) image_192: Option<String>,
    #[serde(default)]
    pub(super) image_512: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackTeamInfoResponse {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) team: Option<SlackTeamInfoTeam>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlackTeamInfoTeam {
    pub(super) id: String,
    pub(super) name: String,
}

impl SlackUserInfoUser {
    pub(super) fn display_name(&self) -> Option<String> {
        self.profile
            .real_name
            .as_deref()
            .or(self.real_name.as_deref())
            .or(self.profile.display_name.as_deref())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
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
}
