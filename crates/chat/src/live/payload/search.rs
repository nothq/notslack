use std::collections::{BTreeSet, HashMap};

use crate::model::{SlackConversationKind, SlackSearchSnapshot};
use serde_json::Value;

use super::{
    sidebar_dom::SlackSidebarSnapshot,
    util::{slack_user_avatar_image_url, slack_user_display_name, SlackAvatarPurpose},
};

mod quick;

pub(super) use quick::{
    enrich_slack_quick_search_messages, finish_slack_quick_search_snapshot,
    load_slack_quick_search_bots, resolve_slack_quick_message_request, slack_quick_search_bot_ids,
    slack_quick_search_conversation_user_ids, slack_quick_search_user_ids,
    SlackQuickSearchDirectory,
};

pub(super) fn slack_search_user_ids(snapshot: &SlackSearchSnapshot) -> BTreeSet<String> {
    snapshot
        .messages
        .iter()
        .flat_map(|message| {
            std::iter::once(message.user_id.as_str())
                .chain(message.reply_user_ids.iter().map(String::as_str))
                .chain(
                    (message.conversation_kind == SlackConversationKind::DirectMessage)
                        .then_some(message.conversation_name.as_str()),
                )
        })
        .filter(|user_id| !user_id.trim().is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn enrich_slack_search_snapshot(
    mut snapshot: SlackSearchSnapshot,
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> SlackSearchSnapshot {
    for message in &mut snapshot.messages {
        let user = users.get(&message.user_id);
        message.username = user
            .and_then(slack_user_display_name)
            .or_else(|| nonempty(message.username.clone()))
            .unwrap_or_else(|| "Slack".to_string());
        message.avatar_image_url =
            user.and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Message));
        message.conversation_name = search_conversation_label(message, users, sidebar)
            .unwrap_or_else(|| {
                if message.conversation_kind == SlackConversationKind::DirectMessage {
                    "Direct message".to_string()
                } else {
                    "Conversation".to_string()
                }
            });
        message.body = normalize_slack_search_mrkdwn(&message.body, users, sidebar);
        message.reply_user_avatar_image_urls = message
            .reply_user_ids
            .iter()
            .filter_map(|user_id| users.get(user_id))
            .filter_map(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Message))
            .take(4)
            .collect();
    }
    snapshot
}

fn search_conversation_label(
    message: &crate::model::SlackSearchMessage,
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> Option<String> {
    sidebar
        .item(&message.conversation_id)
        .and_then(|item| nonempty(item.label.clone()))
        .or_else(|| {
            (message.conversation_kind == SlackConversationKind::DirectMessage)
                .then(|| users.get(&message.conversation_name))
                .flatten()
                .and_then(slack_user_display_name)
        })
        .or_else(|| nonempty(message.conversation_name.clone()))
}

fn normalize_slack_search_mrkdwn(
    body: &str,
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> String {
    let mut normalized = String::with_capacity(body.len());
    let mut index = 0;
    while let Some(open_offset) = body[index..].find('<') {
        let open = index + open_offset;
        normalized.push_str(&body[index..open]);
        let Some(close_offset) = body[open..].find('>') else {
            normalized.push_str(&body[open..]);
            return normalized;
        };
        let close = open + close_offset;
        let token = &body[open + 1..close];
        if let Some(replacement) = normalized_slack_token(token, users, sidebar) {
            normalized.push_str(&replacement);
        } else {
            normalized.push_str(&body[open..=close]);
        }
        index = close + 1;
    }
    normalized.push_str(&body[index..]);
    normalized
}

fn normalized_slack_token(
    token: &str,
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> Option<String> {
    if let Some(mention) = token.strip_prefix('@') {
        let (user_id, explicit_label) = split_slack_reference(mention);
        let label = nonempty(explicit_label.map(str::to_string).unwrap_or_default())
            .or_else(|| users.get(user_id).and_then(slack_user_display_name))?;
        return Some(format!("@{}", label.trim_start_matches('@')));
    }
    if let Some(channel) = token.strip_prefix('#') {
        let (channel_id, explicit_label) = split_slack_reference(channel);
        let label =
            nonempty(explicit_label.map(str::to_string).unwrap_or_default()).or_else(|| {
                sidebar
                    .item(channel_id)
                    .and_then(|item| nonempty(item.label.clone()))
            })?;
        return Some(format!("#{}", label.trim_start_matches('#')));
    }
    if let Some(special) = token.strip_prefix('!') {
        let (kind, explicit_label) = split_slack_reference(special);
        if let Some(label) = nonempty(explicit_label.map(str::to_string).unwrap_or_default()) {
            return Some(label);
        }
        if matches!(kind, "here" | "channel" | "everyone") {
            return Some(format!("@{kind}"));
        }
        if kind.starts_with("date^") {
            return kind
                .rsplit_once('^')
                .map(|(_, fallback)| fallback.to_string());
        }
    }
    if let Some(mailto) = token.strip_prefix("mailto:") {
        let (address, label) = split_slack_reference(mailto);
        return nonempty(label.map(str::to_string).unwrap_or_default())
            .or_else(|| nonempty(address.to_string()));
    }
    None
}

fn split_slack_reference(reference: &str) -> (&str, Option<&str>) {
    reference
        .split_once('|')
        .map_or((reference, None), |(id, label)| (id, Some(label)))
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
