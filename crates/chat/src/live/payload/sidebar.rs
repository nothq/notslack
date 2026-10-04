mod context;

use std::collections::HashMap;

use crate::model::{
    SlackConversationKind, SlackSidebarItem, SlackSidebarSection, SlackUserPresence,
};
use serde_json::Value;
use time::OffsetDateTime;

use super::sidebar_dom::{
    SlackSidebarSnapshot, SlackSidebarSnapshotItem, SlackSidebarSnapshotSection,
};
use super::util::{
    has_slack_unread_messages, slack_conversation_kind, slack_conversation_label,
    slack_user_avatar_image_url, string_at, SlackAvatarPurpose,
};

use context::{
    append_missing_active_direct_message_item, apply_snapshot_context, channels_by_id,
    snapshot_direct_message_label, snapshot_item_label, unread_count_option,
    DirectMessageSidebarItemState,
};

pub(super) fn slack_sections(
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Vec<SlackSidebarSection> {
    slack_sections_at(
        channels,
        active_conversation_id,
        users,
        OffsetDateTime::now_utc(),
        sidebar_snapshot,
    )
}

fn slack_sections_at(
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
    _now: OffsetDateTime,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Vec<SlackSidebarSection> {
    let snapshot = sidebar_snapshot
        .expect("Slack live sidebar rendering requires users.channelSections.list snapshot");
    let sections = snapshot
        .sections
        .iter()
        .filter_map(|section| {
            snapshot_sidebar_section(section, channels, active_conversation_id, users)
        })
        .collect::<Vec<_>>();
    if sections.is_empty() {
        panic!("Slack internal sidebar API returned no renderable sections");
    }
    sections
}

fn snapshot_sidebar_section(
    section: &SlackSidebarSnapshotSection,
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> Option<SlackSidebarSection> {
    let items = match section.key.as_str() {
        "direct_messages" => snapshot_direct_message_sidebar_items(
            &section.items,
            channels,
            active_conversation_id,
            users,
        ),
        "external_connections" | "recent_apps" | "stars" => {
            snapshot_mixed_sidebar_items(&section.items, channels, active_conversation_id, users)
        }
        "channels" => {
            sorted_snapshot_channel_sidebar_items(&section.items, channels, active_conversation_id)
        }
        _ => snapshot_channel_sidebar_items(&section.items, channels, active_conversation_id),
    };
    if items.is_empty() {
        return None;
    }
    Some(SlackSidebarSection {
        label: section.label.clone(),
        items,
    })
}

fn channel_sidebar_item_with_state(
    channel: &Value,
    active_conversation_id: &str,
    label: String,
    unread: bool,
    count: Option<u32>,
) -> SlackSidebarItem {
    let target_kind = slack_conversation_kind(channel);
    let target_id = string_at(channel, &["id"]).unwrap_or_default();
    SlackSidebarItem {
        label,
        secondary_context: None,
        is_external_connection: false,
        icon: Some(
            if matches!(target_kind, SlackConversationKind::PrivateChannel) {
                "lock"
            } else {
                "hash"
            }
            .to_string(),
        ),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: target_id.clone(),
        target_kind,
        user_id: None,
        presence: None,
        active: target_id == active_conversation_id,
        unread,
        latest_message_timestamp: None,
        muted: channel.get("is_muted").and_then(Value::as_bool) == Some(true),
        count,
    }
}

fn direct_message_sidebar_item(
    channel: &Value,
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> SlackSidebarItem {
    let label =
        slack_conversation_label(channel, users).unwrap_or_else(|| "direct-message".to_string());
    direct_message_sidebar_item_with_label(
        channel,
        active_conversation_id,
        label,
        channel
            .get("user")
            .and_then(Value::as_str)
            .and_then(|user_id| users.get(user_id))
            .and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Sidebar)),
        None,
    )
}

fn direct_message_sidebar_item_with_label(
    channel: &Value,
    active_conversation_id: &str,
    label: String,
    avatar_image_url: Option<String>,
    presence: Option<SlackUserPresence>,
) -> SlackSidebarItem {
    direct_message_sidebar_item_with_state(
        channel,
        active_conversation_id,
        DirectMessageSidebarItemState {
            label,
            secondary_context: None,
            is_external_connection: false,
            avatar_image_url,
            presence,
            unread: has_slack_unread_messages(channel),
            count: unread_count_option(channel),
            latest_message_timestamp: None,
        },
    )
}

fn direct_message_sidebar_item_with_state(
    channel: &Value,
    active_conversation_id: &str,
    state: DirectMessageSidebarItemState,
) -> SlackSidebarItem {
    let target_kind = slack_conversation_kind(channel);
    let user_id = channel
        .get("user")
        .and_then(Value::as_str)
        .map(str::to_string);
    let target_id = string_at(channel, &["id"]).unwrap_or_default();
    SlackSidebarItem {
        label: state.label,
        secondary_context: state.secondary_context,
        is_external_connection: state.is_external_connection,
        icon: Some("person".to_string()),
        avatar_image_url: state.avatar_image_url,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: target_id.clone(),
        target_kind,
        user_id,
        presence: state.presence,
        active: target_id == active_conversation_id,
        unread: state.unread,
        latest_message_timestamp: state.latest_message_timestamp,
        muted: channel.get("is_muted").and_then(Value::as_bool) == Some(true),
        count: state.count,
    }
}

fn sorted_snapshot_channel_sidebar_items(
    snapshot_items: &[SlackSidebarSnapshotItem],
    channels: &[Value],
    active_conversation_id: &str,
) -> Vec<SlackSidebarItem> {
    let mut items =
        snapshot_channel_sidebar_items(snapshot_items, channels, active_conversation_id);
    items.sort_by_cached_key(|item| item.label.to_ascii_lowercase());
    items
}

fn snapshot_channel_sidebar_items(
    snapshot_items: &[SlackSidebarSnapshotItem],
    channels: &[Value],
    active_conversation_id: &str,
) -> Vec<SlackSidebarItem> {
    let channels_by_id = channels_by_id(channels);
    snapshot_items
        .iter()
        .filter_map(|snapshot_item| {
            let channel = channels_by_id.get(snapshot_item.id.as_str())?;
            slack_conversation_kind(channel)
                .is_channel()
                .then(|| snapshot_channel_item(snapshot_item, channel, active_conversation_id))
        })
        .collect()
}

fn snapshot_channel_item(
    snapshot_item: &SlackSidebarSnapshotItem,
    channel: &Value,
    active_conversation_id: &str,
) -> SlackSidebarItem {
    let mut item = channel_sidebar_item_with_state(
        channel,
        active_conversation_id,
        snapshot_item_label(snapshot_item, channel),
        snapshot_item.unread,
        snapshot_item.count,
    );
    apply_snapshot_context(&mut item, snapshot_item);
    item
}

fn snapshot_mixed_sidebar_items(
    snapshot_items: &[SlackSidebarSnapshotItem],
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> Vec<SlackSidebarItem> {
    let channels_by_id = channels_by_id(channels);
    snapshot_items
        .iter()
        .filter_map(|snapshot_item| {
            if let Some(channel) = channels_by_id.get(snapshot_item.id.as_str()) {
                return match slack_conversation_kind(channel) {
                    SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => Some(
                        snapshot_channel_item(snapshot_item, channel, active_conversation_id),
                    ),
                    SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
                        snapshot_direct_message_item(
                            snapshot_item,
                            channel,
                            active_conversation_id,
                            users,
                        )
                    }
                    SlackConversationKind::Unknown => None,
                };
            }
            match snapshot_item.kind.as_deref() {
                Some("direct_message") => Some(snapshot_direct_message_item_without_channel(
                    snapshot_item,
                    SlackConversationKind::DirectMessage,
                    active_conversation_id,
                    users,
                )),
                Some("group_message") => Some(snapshot_direct_message_item_without_channel(
                    snapshot_item,
                    SlackConversationKind::GroupMessage,
                    active_conversation_id,
                    users,
                )),
                _ => None,
            }
        })
        .collect()
}

fn snapshot_direct_message_item_without_channel(
    snapshot_item: &SlackSidebarSnapshotItem,
    target_kind: SlackConversationKind,
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> SlackSidebarItem {
    let user_avatar = snapshot_item
        .user_id
        .as_deref()
        .and_then(|user_id| users.get(user_id))
        .and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Sidebar));
    SlackSidebarItem {
        label: if snapshot_item.label.trim().is_empty() {
            snapshot_item.id.clone()
        } else {
            snapshot_item.label.clone()
        },
        secondary_context: snapshot_item.secondary_context.clone(),
        is_external_connection: snapshot_item.is_external_connection,
        icon: Some("person".to_string()),
        avatar_image_url: snapshot_item.avatar_image_url.clone().or(user_avatar),
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: snapshot_item.id.clone(),
        target_kind,
        user_id: snapshot_item.user_id.clone(),
        presence: snapshot_item.presence,
        active: snapshot_item.id == active_conversation_id,
        unread: snapshot_item.unread,
        latest_message_timestamp: snapshot_item.latest_message_timestamp.clone(),
        muted: false,
        count: snapshot_item.count,
    }
}

fn snapshot_direct_message_sidebar_items(
    snapshot_items: &[SlackSidebarSnapshotItem],
    channels: &[Value],
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> Vec<SlackSidebarItem> {
    let channels_by_id = channels_by_id(channels);
    let mut items = snapshot_items
        .iter()
        .filter_map(|snapshot_item| {
            if let Some(channel) = channels_by_id.get(snapshot_item.id.as_str()) {
                return snapshot_direct_message_item(
                    snapshot_item,
                    channel,
                    active_conversation_id,
                    users,
                );
            }
            match snapshot_item.kind.as_deref() {
                Some("direct_message") => Some(snapshot_direct_message_item_without_channel(
                    snapshot_item,
                    SlackConversationKind::DirectMessage,
                    active_conversation_id,
                    users,
                )),
                Some("group_message") => Some(snapshot_direct_message_item_without_channel(
                    snapshot_item,
                    SlackConversationKind::GroupMessage,
                    active_conversation_id,
                    users,
                )),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    append_missing_active_direct_message_item(&mut items, channels, active_conversation_id, users);
    items
}

fn snapshot_direct_message_item(
    snapshot_item: &SlackSidebarSnapshotItem,
    channel: &Value,
    active_conversation_id: &str,
    users: &HashMap<String, Value>,
) -> Option<SlackSidebarItem> {
    if !matches!(
        slack_conversation_kind(channel),
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
    ) {
        return None;
    }
    Some(direct_message_sidebar_item_with_state(
        channel,
        active_conversation_id,
        DirectMessageSidebarItemState {
            label: snapshot_direct_message_label(snapshot_item, channel, users),
            secondary_context: snapshot_item.secondary_context.clone(),
            is_external_connection: snapshot_item.is_external_connection,
            avatar_image_url: snapshot_item.avatar_image_url.clone().or_else(|| {
                channel
                    .get("user")
                    .and_then(Value::as_str)
                    .and_then(|user_id| users.get(user_id))
                    .and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Sidebar))
            }),
            presence: snapshot_item.presence,
            unread: snapshot_item.unread,
            count: snapshot_item.count,
            latest_message_timestamp: snapshot_item.latest_message_timestamp.clone(),
        },
    ))
}

#[cfg(test)]
mod tests;
