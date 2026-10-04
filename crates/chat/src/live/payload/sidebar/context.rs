use std::collections::HashMap;

use crate::model::{SlackConversationKind, SlackSidebarItem, SlackUserPresence};
use serde_json::Value;

use super::direct_message_sidebar_item;
use crate::live::payload::sidebar_dom::SlackSidebarSnapshotItem;
use crate::live::payload::util::{
    slack_conversation_kind, slack_conversation_label, slack_unread_display_count, string_at,
};

pub(super) struct DirectMessageSidebarItemState {
    pub(super) label: String,
    pub(super) secondary_context: Option<String>,
    pub(super) is_external_connection: bool,
    pub(super) avatar_image_url: Option<String>,
    pub(super) presence: Option<SlackUserPresence>,
    pub(super) unread: bool,
    pub(super) count: Option<u32>,
    pub(super) latest_message_timestamp: Option<crate::model::SlackMessageTimestamp>,
}

pub(super) fn unread_count_option(channel: &Value) -> Option<u32> {
    slack_unread_display_count(channel)
}

pub(super) fn apply_snapshot_context(
    item: &mut SlackSidebarItem,
    snapshot_item: &SlackSidebarSnapshotItem,
) {
    item.secondary_context = snapshot_item.secondary_context.clone();
    item.is_external_connection = snapshot_item.is_external_connection;
    item.latest_message_timestamp = snapshot_item.latest_message_timestamp.clone();
}

pub(super) fn snapshot_direct_message_label(
    snapshot_item: &SlackSidebarSnapshotItem,
    channel: &Value,
    users: &HashMap<String, Value>,
) -> String {
    if !snapshot_item.label.trim().is_empty() {
        return snapshot_item.label.clone();
    }
    slack_conversation_label(channel, users)
        .unwrap_or_else(|| snapshot_item_label(snapshot_item, channel))
}

pub(super) fn snapshot_item_label(
    snapshot_item: &SlackSidebarSnapshotItem,
    channel: &Value,
) -> String {
    if !snapshot_item.label.trim().is_empty() {
        return snapshot_item.label.clone();
    }
    string_at(channel, &["name"])
        .or_else(|| string_at(channel, &["id"]))
        .unwrap_or_else(|| snapshot_item.id.clone())
}

pub(super) fn channels_by_id(channels: &[Value]) -> HashMap<String, &Value> {
    channels
        .iter()
        .filter_map(|channel| string_at(channel, &["id"]).map(|id| (id, channel)))
        .collect()
}

pub(super) fn append_missing_active_direct_message_item(
    items: &mut Vec<SlackSidebarItem>,
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) {
    if items
        .iter()
        .any(|item| item.target_id == active_conversation_id)
    {
        return;
    }
    let Some(channel) = channels.iter().find(|channel| {
        string_at(channel, &["id"]).as_deref() == Some(active_conversation_id)
            && matches!(
                slack_conversation_kind(channel),
                SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
            )
    }) else {
        return;
    };
    items.push(direct_message_sidebar_item(
        channel,
        active_conversation_id,
        users,
    ));
}
