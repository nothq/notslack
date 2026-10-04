mod item;
mod render;

use std::collections::{HashMap, HashSet};

use crate::live::internal_sidebar::sections::{SlackChannelSection, SlackChannelSectionsResponse};
use crate::live::payload::sidebar_dom::SlackSidebarSnapshot;

use self::render::{
    boot_conversations, channel_sort_key, direct_message_sort_label, direct_message_sort_rank,
    snapshot_section,
};
use super::types::{
    SlackBootConversation, SlackBootState, SlackClientCountsResponse, SlackCountConversation,
    SlackDirectMessagePresence, SlackDirectMessageUser,
};

struct SlackBootIndex {
    conversations: HashMap<String, SlackBootConversation>,
    channel_priority: HashMap<String, f64>,
    starred: Vec<String>,
    section_conversation_ids: HashSet<String>,
    counts: SlackClientCountsResponse,
    dm_users: HashMap<String, SlackDirectMessageUser>,
    group_message_labels: HashMap<String, String>,
    admin_visible: bool,
    self_presence: Option<SlackDirectMessagePresence>,
    self_notifications_paused: bool,
}

#[derive(Clone, Copy)]
enum ChannelBucket {
    Normal,
    SlackConnect,
}

pub(in crate::live::internal_sidebar) fn snapshot_from_boot_state(
    sections: SlackChannelSectionsResponse,
    state: SlackBootState,
) -> Result<SlackSidebarSnapshot, String> {
    let section_conversation_ids = sections
        .channel_sections
        .iter()
        .flat_map(|section| section.channel_ids_page.channel_ids.iter().cloned())
        .collect();
    let boot_index = SlackBootIndex::new(state, section_conversation_ids)?;
    let snapshot_sections = sections
        .channel_sections
        .into_iter()
        .map(|section| snapshot_section(section, &boot_index))
        .collect::<Result<Vec<_>, _>>()?;
    if snapshot_sections
        .iter()
        .all(|section| section.items.is_empty())
    {
        return Err(
            "Slack internal sidebar APIs returned no channel ids for any section".to_string(),
        );
    }
    let direct_message_unread_states = boot_index.counts.direct_message_unread_states()?;
    Ok(SlackSidebarSnapshot {
        draft_count: None,
        activity_count: boot_index.activity_count(),
        dms_unread_messages: boot_index.dms_unread_messages()?,
        admin_visible: boot_index.admin_visible,
        admin_attention: false,
        self_presence: boot_index
            .self_presence
            .map(SlackDirectMessagePresence::as_model),
        self_notifications_paused: boot_index.self_notifications_paused,
        direct_message_unread_states,
        sections: snapshot_sections,
    })
}

impl SlackBootIndex {
    fn new(
        state: SlackBootState,
        section_conversation_ids: HashSet<String>,
    ) -> Result<Self, String> {
        let admin_visible = state
            .boot
            .self_user
            .as_ref()
            .ok_or_else(|| "Slack client.userBoot response missing self".to_string())?
            .is_admin;
        state.boot.prefs.ensure_supported()?;
        let channel_priority = state.boot.channels_priority;
        let starred = state.boot.starred;
        let conversations =
            boot_conversations(state.boot.channels, state.boot.ims, state.boot.mpims);
        Ok(Self {
            conversations,
            channel_priority,
            starred,
            section_conversation_ids,
            counts: state.counts,
            dm_users: state.dm_users,
            group_message_labels: state.group_message_labels,
            admin_visible,
            self_presence: state.self_presence,
            self_notifications_paused: state.self_notifications_paused,
        })
    }

    fn section_ids(&self, section: &SlackChannelSection) -> Vec<String> {
        match section.kind.as_str() {
            "channels" => self.channel_section_ids(section, ChannelBucket::Normal),
            "slack_connect" => self.slack_connect_section_ids(section),
            "direct_messages" => self.direct_message_section_ids(section),
            "recent_apps" => self.recent_app_section_ids(section),
            "stars" => self.starred_section_ids(section),
            _ => section.channel_ids_page.channel_ids.clone(),
        }
    }

    fn starred_section_ids(&self, section: &SlackChannelSection) -> Vec<String> {
        if section.channel_ids_page.channel_ids.is_empty() {
            return self.starred.clone();
        }
        section.channel_ids_page.channel_ids.clone()
    }

    fn recent_app_section_ids(&self, section: &SlackChannelSection) -> Vec<String> {
        if !section.channel_ids_page.channel_ids.is_empty() {
            return section.channel_ids_page.channel_ids.clone();
        }
        self.counts
            .ims
            .iter()
            .filter(|conversation| conversation.mention_display_count().is_some())
            .filter(|conversation| {
                !self.section_conversation_ids.contains(&conversation.id)
                    && !self.channel_priority.contains_key(&conversation.id)
                    && !self.starred.contains(&conversation.id)
                    && self
                        .conversations
                        .get(&conversation.id)
                        .is_none_or(|conversation| !conversation.is_slack_connect())
            })
            .map(|conversation| conversation.id.clone())
            .collect()
    }

    fn slack_connect_section_ids(&self, section: &SlackChannelSection) -> Vec<String> {
        if section.channel_ids_page.channel_ids.is_empty() {
            let visible_ids = self.visible_direct_message_ids();
            let mut ids = self.slack_connect_direct_message_ids(&visible_ids);
            ids.extend(self.channel_section_ids(section, ChannelBucket::SlackConnect));
            return ids;
        }
        section
            .channel_ids_page
            .channel_ids
            .iter()
            .filter_map(|id| self.visible_slack_connect_conversation_id(id))
            .collect()
    }

    fn visible_slack_connect_conversation_id(&self, id: &str) -> Option<String> {
        let conversation = self.conversations.get(id)?;
        (conversation.is_slack_connect()
            && (conversation.is_direct_message_like() || conversation.is_renderable_channel_like()))
        .then(|| id.to_string())
    }

    fn channel_section_ids(
        &self,
        section: &SlackChannelSection,
        bucket: ChannelBucket,
    ) -> Vec<String> {
        if !section.channel_ids_page.channel_ids.is_empty() {
            return section
                .channel_ids_page
                .channel_ids
                .iter()
                .filter_map(|id| self.visible_channel_id(id, bucket))
                .collect();
        }
        let mut channels = self
            .conversations
            .values()
            .filter(|conversation| self.visible_channel(conversation, bucket))
            .collect::<Vec<_>>();
        channels.sort_by_key(|conversation| channel_sort_key(conversation));
        channels
            .into_iter()
            .map(|conversation| conversation.id.clone())
            .collect()
    }

    fn direct_message_section_ids(&self, section: &SlackChannelSection) -> Vec<String> {
        let visible_ids = self.visible_direct_message_ids();
        if !section.channel_ids_page.channel_ids.is_empty() {
            return section
                .channel_ids_page
                .channel_ids
                .iter()
                .filter(|id| {
                    visible_ids.contains(id.as_str())
                        && self
                            .conversations
                            .get(id.as_str())
                            .is_some_and(|conversation| {
                                conversation.is_direct_message_like()
                                    && !conversation.is_slack_connect()
                            })
                })
                .cloned()
                .collect();
        }
        self.priority_direct_message_ids(&visible_ids)
    }

    fn visible_channel_id(&self, id: &str, bucket: ChannelBucket) -> Option<String> {
        let conversation = self.conversations.get(id)?;
        self.visible_channel(conversation, bucket)
            .then(|| id.to_string())
    }

    fn visible_channel(&self, conversation: &SlackBootConversation, bucket: ChannelBucket) -> bool {
        if self.count_channel(conversation.id.as_str()).is_none() {
            return false;
        }
        conversation.is_renderable_channel_like()
            && conversation.in_bucket(bucket)
            && (self.channel_priority.contains_key(conversation.id.as_str())
                || conversation.is_general
                || !conversation.is_dormant()
                || conversation.visible_dormant_channel())
    }

    fn count_channel(&self, id: &str) -> Option<&SlackCountConversation> {
        self.counts
            .channels
            .iter()
            .find(|conversation| conversation.id == id)
    }

    fn visible_direct_message_ids(&self) -> HashSet<&str> {
        self.counts
            .mpims
            .iter()
            .chain(self.counts.ims.iter())
            .map(|conversation| conversation.id.as_str())
            .collect()
    }

    fn priority_direct_message_ids(&self, visible_ids: &HashSet<&str>) -> Vec<String> {
        let mut items = self
            .conversations
            .iter()
            .filter(|(id, conversation)| {
                visible_ids.contains(id.as_str())
                    && conversation.is_direct_message_like()
                    && !conversation.is_slack_connect()
                    && (conversation.is_mpim || self.channel_priority.contains_key(id.as_str()))
            })
            .map(|(id, conversation)| (id, conversation, self.dm_users.get(id.as_str())))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            direct_message_sort_rank(left.1, left.2.and_then(|user| user.presence))
                .cmp(&direct_message_sort_rank(
                    right.1,
                    right.2.and_then(|user| user.presence),
                ))
                .then_with(|| {
                    direct_message_sort_label(left.1, left.2)
                        .to_ascii_lowercase()
                        .cmp(&direct_message_sort_label(right.1, right.2).to_ascii_lowercase())
                })
                .then_with(|| left.0.cmp(right.0))
        });
        items.into_iter().map(|(id, _, _)| id.clone()).collect()
    }

    fn slack_connect_direct_message_ids(&self, visible_ids: &HashSet<&str>) -> Vec<String> {
        let mut items = self
            .dm_users
            .iter()
            .filter(|(id, _)| visible_ids.contains(id.as_str()))
            .filter(|(id, _)| {
                self.conversations
                    .get(id.as_str())
                    .is_some_and(|conversation| {
                        conversation.is_direct_message_like() && conversation.is_slack_connect()
                    })
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            match (
                self.channel_priority.get(left.0.as_str()),
                self.channel_priority.get(right.0.as_str()),
            ) {
                (Some(left_priority), Some(right_priority)) => {
                    left_priority.total_cmp(right_priority)
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| {
                left.1
                    .label
                    .to_ascii_lowercase()
                    .cmp(&right.1.label.to_ascii_lowercase())
            })
            .then_with(|| left.0.cmp(right.0))
        });
        items.into_iter().map(|(id, _)| id.clone()).collect()
    }

    fn activity_count(&self) -> Option<u32> {
        self.counts.activity_count()
    }

    fn dms_unread_messages(&self) -> Result<Option<u32>, String> {
        self.counts.dms_unread_messages()
    }
}
