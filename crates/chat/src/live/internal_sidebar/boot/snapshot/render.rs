use std::collections::HashMap;

use crate::live::internal_sidebar::sections::SlackChannelSection;
use crate::live::payload::sidebar_dom::{SlackSidebarSnapshotItem, SlackSidebarSnapshotSection};

use super::{ChannelBucket, SlackBootIndex};
use crate::live::internal_sidebar::boot::types::{
    SlackBootConversation, SlackBootTextField, SlackDirectMessagePresence, SlackDirectMessageUser,
};

pub(super) struct SlackSidebarSnapshotItemInput {
    pub(super) id: String,
    pub(super) user_id: Option<String>,
    pub(super) label: String,
    pub(super) secondary_context: Option<String>,
    pub(super) avatar_image_url: Option<String>,
    pub(super) is_external_connection: bool,
    pub(super) presence: Option<crate::model::SlackUserPresence>,
    pub(super) kind: Option<String>,
    pub(super) unread: bool,
    pub(super) count: Option<u32>,
    pub(super) latest_message_timestamp: Option<crate::model::SlackMessageTimestamp>,
}

impl SlackBootConversation {
    fn is_channel_like(&self) -> bool {
        self.is_channel || self.is_group || (!self.is_im && !self.is_mpim)
    }

    pub(super) fn is_renderable_channel_like(&self) -> bool {
        self.is_channel_like() && !self.is_archived && self.unlinked == 0
    }

    pub(super) fn is_direct_message_like(&self) -> bool {
        self.is_im || self.is_mpim
    }

    pub(super) fn is_dormant(&self) -> bool {
        self.properties.is_dormant
    }

    pub(super) fn visible_dormant_channel(&self) -> bool {
        self.is_dormant()
            && !self.is_group
            && !self.is_private
            && self.properties.meeting_notes.is_some()
            && self.purpose.is_empty()
            && self.topic.is_empty()
    }

    pub(super) fn in_bucket(&self, bucket: ChannelBucket) -> bool {
        match bucket {
            ChannelBucket::Normal => !self.is_slack_connect(),
            ChannelBucket::SlackConnect => self.is_slack_connect(),
        }
    }

    pub(super) fn is_slack_connect(&self) -> bool {
        self.is_shared || self.is_ext_shared || self.is_org_shared
    }

    pub(super) fn snapshot_kind(&self) -> String {
        if self.is_im {
            "direct_message"
        } else if self.is_mpim {
            "group_message"
        } else if self.is_private {
            "private_channel"
        } else {
            "channel"
        }
        .to_string()
    }

    pub(super) fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .or(self.name_normalized.as_deref())
            .unwrap_or(self.id.as_str())
    }
}

impl SlackBootTextField {
    fn is_empty(&self) -> bool {
        self.value.trim().is_empty()
    }
}

impl SlackSidebarSnapshotItem {
    pub(super) fn from_channel_id(input: SlackSidebarSnapshotItemInput) -> Self {
        Self {
            id: input.id,
            label: input.label,
            secondary_context: input.secondary_context,
            avatar_image_url: input.avatar_image_url,
            is_external_connection: input.is_external_connection,
            user_id: input.user_id,
            presence: input.presence,
            kind: input.kind,
            selected: false,
            unread: input.unread,
            count: input.count,
            latest_message_timestamp: input.latest_message_timestamp,
        }
    }
}

pub(super) fn snapshot_section(
    section: SlackChannelSection,
    boot_index: &SlackBootIndex,
) -> Result<SlackSidebarSnapshotSection, String> {
    if section.channel_section_id.trim().is_empty() {
        return Err("Slack internal sidebar section missing channel_section_id".to_string());
    }
    let items = boot_index
        .section_ids(&section)
        .into_iter()
        .map(|id| boot_index.snapshot_item(id, section.kind == "slack_connect"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SlackSidebarSnapshotSection {
        key: section_key(&section.kind),
        label: section_label(&section),
        items,
    })
}

pub(super) fn boot_conversations(
    channels: Vec<SlackBootConversation>,
    ims: Vec<SlackBootConversation>,
    mpims: Vec<SlackBootConversation>,
) -> HashMap<String, SlackBootConversation> {
    channels
        .into_iter()
        .chain(ims)
        .chain(mpims)
        .map(|conversation| (conversation.id.clone(), conversation))
        .collect()
}

pub(super) fn channel_sort_key(conversation: &SlackBootConversation) -> String {
    conversation.display_name().to_ascii_lowercase()
}

fn presence_sort_rank(presence: Option<SlackDirectMessagePresence>) -> u8 {
    match presence {
        Some(SlackDirectMessagePresence::Active) => 0,
        Some(SlackDirectMessagePresence::Away) => 1,
        None => 2,
    }
}

pub(super) fn direct_message_sort_rank(
    conversation: &SlackBootConversation,
    presence: Option<SlackDirectMessagePresence>,
) -> u8 {
    if conversation.is_mpim {
        0
    } else {
        presence_sort_rank(presence) + 1
    }
}

pub(super) fn direct_message_sort_label<'a>(
    conversation: &'a SlackBootConversation,
    user: Option<&'a SlackDirectMessageUser>,
) -> &'a str {
    user.map_or_else(|| conversation.display_name(), |user| user.label.as_str())
}

fn section_key(kind: &str) -> String {
    match kind {
        "direct_messages" => "direct_messages".to_string(),
        "channels" => "channels".to_string(),
        "slack_connect" => "external_connections".to_string(),
        value => value.to_string(),
    }
}

fn section_label(section: &SlackChannelSection) -> String {
    if section.kind == "slack_connect" {
        return default_section_label(&section.kind);
    }
    section
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_section_label(&section.kind))
}

fn default_section_label(kind: &str) -> String {
    match kind {
        "stars" => "Starred",
        "slack_connect" => "External connections",
        "salesforce_records" => "Salesforce",
        "channels" => "Channels",
        "direct_messages" => "Direct messages",
        "recent_apps" => "Apps",
        "agents" => "Agents",
        value => value,
    }
    .to_string()
}
