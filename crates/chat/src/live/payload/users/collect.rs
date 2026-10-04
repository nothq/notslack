use std::collections::{BTreeSet, HashMap};

use serde_json::Value;
use time::{Duration, OffsetDateTime};

use super::WorkspaceUserLoadInput;
use crate::live::payload::{
    sidebar_dom::SlackSidebarSnapshot,
    util::{has_slack_unread_messages, slack_conversation_kind, string_at},
    SLACK_CONVERSATION_HISTORY_PAGE_SIZE,
};

pub(super) fn collect_workspace_user_ids(input: &WorkspaceUserLoadInput<'_>) -> BTreeSet<String> {
    let channels = input
        .conversations
        .get("channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let messages = input
        .history
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut user_ids = BTreeSet::new();
    if input.sidebar_snapshot.is_none() {
        collect_visible_dm_user_ids(
            &channels,
            input.active_conversation_id,
            input.now,
            &mut user_ids,
        );
    } else if let Some(snapshot) = input.sidebar_snapshot {
        collect_sidebar_snapshot_dm_user_ids(snapshot, &channels, &mut user_ids);
    }
    collect_channel_user_id(input.channel_info, &mut user_ids);
    collect_message_user_ids(&messages, &mut user_ids);
    user_ids
}

pub(super) fn conversation_history_user_ids(history: &Value) -> Result<BTreeSet<String>, String> {
    let messages = history
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack conversations.history response missing messages array".to_string())?;
    if messages.len() > SLACK_CONVERSATION_HISTORY_PAGE_SIZE {
        return Err(format!(
            "Slack conversations.history returned {} messages for a page limited to {SLACK_CONVERSATION_HISTORY_PAGE_SIZE}",
            messages.len()
        ));
    }
    let mut user_ids = BTreeSet::new();
    collect_message_user_ids(messages, &mut user_ids);
    Ok(user_ids)
}

pub(super) fn collect_sidebar_snapshot_dm_user_ids(
    snapshot: &SlackSidebarSnapshot,
    channels: &[Value],
    user_ids: &mut BTreeSet<String>,
) {
    let channels_by_id = channels
        .iter()
        .filter_map(|channel| string_at(channel, &["id"]).map(|id| (id, channel)))
        .collect::<HashMap<_, _>>();
    for item in snapshot
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
    {
        let Some(channel) = channels_by_id.get(item.id.as_str()) else {
            continue;
        };
        if !matches!(
            slack_conversation_kind(channel),
            crate::model::SlackConversationKind::DirectMessage
                | crate::model::SlackConversationKind::GroupMessage
        ) {
            continue;
        }
        if let Some(user_id) = item.user_id.as_deref() {
            user_ids.insert(user_id.to_string());
            continue;
        }
        if let Some(user_id) = channel.get("user").and_then(Value::as_str) {
            user_ids.insert(user_id.to_string());
        }
    }
}

pub(super) fn collect_channel_user_id(channel_info: &Value, user_ids: &mut BTreeSet<String>) {
    if let Some(user_id) = channel_info
        .get("channel")
        .and_then(|channel| channel.get("user"))
        .and_then(Value::as_str)
    {
        user_ids.insert(user_id.to_string());
    }
}

fn collect_visible_dm_user_ids(
    channels: &[Value],
    active_conversation_id: &str,
    now: OffsetDateTime,
    user_ids: &mut BTreeSet<String>,
) {
    for channel in channels {
        if channel.get("is_im").and_then(Value::as_bool) != Some(true)
            || !channel_is_visible_sidebar_entry(channel, active_conversation_id, now)
        {
            continue;
        }
        if let Some(user_id) = channel.get("user").and_then(Value::as_str) {
            user_ids.insert(user_id.to_string());
        }
    }
}

fn collect_message_user_ids(messages: &[Value], user_ids: &mut BTreeSet<String>) {
    for message in messages {
        if let Some(user_id) = message.get("user").and_then(Value::as_str) {
            user_ids.insert(user_id.to_string());
        }
        if let Some(reply_users) = message.get("reply_users").and_then(Value::as_array) {
            user_ids.extend(
                reply_users
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|user_id| !user_id.is_empty())
                    .map(str::to_string),
            );
        }
        if let Some(blocks) = message.get("blocks").and_then(Value::as_array) {
            for block in blocks {
                collect_rich_text_user_ids(block, user_ids);
            }
        }
    }
}

fn collect_rich_text_user_ids(value: &Value, user_ids: &mut BTreeSet<String>) {
    if value.get("type").and_then(Value::as_str) == Some("user") {
        if let Some(user_id) = value.get("user_id").and_then(Value::as_str) {
            user_ids.insert(user_id.to_string());
        }
    }
    if let Some(elements) = value.get("elements").and_then(Value::as_array) {
        for element in elements {
            collect_rich_text_user_ids(element, user_ids);
        }
    }
}

fn channel_is_visible_sidebar_entry(
    channel: &Value,
    active_conversation_id: &str,
    now: OffsetDateTime,
) -> bool {
    let target_id = string_at(channel, &["id"]).unwrap_or_default();
    target_id == active_conversation_id
        || has_slack_unread_messages(channel)
        || channel_updated_ms(channel) >= active_sidebar_cutoff_ms(now)
}

fn active_sidebar_cutoff_ms(now: OffsetDateTime) -> i64 {
    (now - Duration::days(30)).unix_timestamp() * 1_000
}

fn channel_updated_ms(channel: &Value) -> i64 {
    channel
        .get("updated")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        })
        .unwrap_or_default()
}
