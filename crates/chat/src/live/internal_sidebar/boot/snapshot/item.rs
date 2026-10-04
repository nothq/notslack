use crate::live::conversation::normalize_conversation_label;
use crate::live::payload::sidebar_dom::SlackSidebarSnapshotItem;
use crate::model::SlackConversationKind;

use super::render::SlackSidebarSnapshotItemInput;
use super::SlackBootIndex;
use crate::live::internal_sidebar::boot::types::{SlackBootConversation, SlackCountConversation};

impl SlackBootIndex {
    pub(super) fn snapshot_item(
        &self,
        id: String,
        is_external_connection: bool,
    ) -> Result<SlackSidebarSnapshotItem, String> {
        let count = self.count_conversation(id.as_str());
        let dm_user = self.dm_users.get(id.as_str());
        let conversation = self.conversations.get(id.as_str());
        let kind = conversation
            .map(SlackBootConversation::snapshot_kind)
            .or_else(|| self.count_conversation_kind(id.as_str()));
        let label = dm_user.map_or_else(
            || self.group_message_label(id.as_str(), conversation),
            |user| user.label.clone(),
        );
        Ok(SlackSidebarSnapshotItem::from_channel_id(
            SlackSidebarSnapshotItemInput {
                id,
                user_id: dm_user.map(|user| user.user_id.clone()),
                label,
                secondary_context: dm_user.and_then(|user| user.secondary_context.clone()),
                avatar_image_url: dm_user.and_then(|user| user.avatar_image_url.clone()),
                is_external_connection,
                presence: dm_user
                    .and_then(|user| user.presence.map(|presence| presence.as_model())),
                kind: kind.clone(),
                unread: count.is_some_and(SlackCountConversation::unread),
                count: display_count(count, kind.as_deref()),
                latest_message_timestamp: count
                    .map(SlackCountConversation::latest_message_timestamp)
                    .transpose()?
                    .flatten(),
            },
        ))
    }

    fn count_conversation(&self, id: &str) -> Option<&SlackCountConversation> {
        self.counts
            .conversations()
            .find(|conversation| conversation.id == id)
    }

    fn count_conversation_kind(&self, id: &str) -> Option<String> {
        if self
            .counts
            .ims
            .iter()
            .any(|conversation| conversation.id == id)
        {
            return Some("direct_message".to_string());
        }
        self.counts
            .mpims
            .iter()
            .any(|conversation| conversation.id == id)
            .then(|| "group_message".to_string())
    }

    fn group_message_label(
        &self,
        id: &str,
        conversation: Option<&SlackBootConversation>,
    ) -> String {
        self.group_message_labels
            .get(id)
            .map(|label| normalize_group_message_label(label))
            .or_else(|| {
                conversation
                    .filter(|conversation| conversation.is_mpim)
                    .map(|conversation| normalize_group_message_label(conversation.display_name()))
            })
            .unwrap_or_default()
    }
}

fn display_count(count: Option<&SlackCountConversation>, kind: Option<&str>) -> Option<u32> {
    count.and_then(|count| {
        if matches!(kind, Some("direct_message") | Some("group_message")) {
            count.display_count()
        } else {
            count.mention_display_count()
        }
    })
}

fn normalize_group_message_label(label: &str) -> String {
    normalize_conversation_label(label, SlackConversationKind::GroupMessage)
}
