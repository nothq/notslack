use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::model::SlackMessageTimestamp;
use crate::model::{
    SlackAllThread, SlackAllThreadsCursor, SlackAllThreadsSnapshot, SlackConversationKind,
    SlackUserPresence,
};
use chrono_tz::Tz;
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::internal_sidebar::threads::SlackAllThreadWire;
use crate::live::payload::{
    message::slack_message_from_value_with_context_in_timezone, sidebar_dom::SlackSidebarSnapshot,
    users::load_slack_search_users_with_cache, util::slack_user_display_name,
};

impl SlackLiveWorkspaceLoader {
    pub fn load_all_threads(
        &self,
        cursor: Option<&SlackAllThreadsCursor>,
    ) -> Result<SlackAllThreadsSnapshot, String> {
        let page = self.sidebar_api.load_all_threads(cursor)?;
        let mut user_ids = BTreeSet::new();
        for thread in &page.threads {
            collect_thread_user_ids(thread, &mut user_ids);
        }
        let users = load_slack_search_users_with_cache(
            &self.api,
            user_ids,
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let self_user_id = self.load_self_user_id()?;
        let sidebar = self.load_sidebar_snapshot()?;
        let context = AllThreadsDecodeContext {
            users: &users,
            sidebar: &sidebar,
            self_user_id: &self_user_id,
            timezone: self.timezone.value,
        };
        let threads = decode_all_threads(page.threads, &context)?;
        Ok(SlackAllThreadsSnapshot {
            team_id: self.team_id.clone(),
            timezone: self.timezone.value.into(),
            threads,
            next_cursor: page.next_cursor,
            total_unread_replies: page.total_unread_replies,
        })
    }
}

struct AllThreadsDecodeContext<'a> {
    users: &'a HashMap<String, Value>,
    sidebar: &'a SlackSidebarSnapshot,
    self_user_id: &'a str,
    timezone: Tz,
}

struct ValidatedThread {
    id: String,
    conversation_id: String,
    thread_timestamp: String,
    reply_count: u32,
    reply_users: Vec<String>,
    unread_reply_timestamps: Vec<SlackMessageTimestamp>,
    parent_payload: Value,
    reply_payloads: Vec<Value>,
}

struct AllThreadsConversation {
    name: String,
    kind: SlackConversationKind,
    direct_message_user_id: Option<String>,
    direct_message_presence: Option<SlackUserPresence>,
    participant_authority: AllThreadsParticipantAuthority,
}

enum AllThreadsParticipantAuthority {
    DirectMessage { peer_label: String },
    ThreadRepliers,
    UnavailableDirectMessage,
}

fn decode_all_threads(
    threads: Vec<SlackAllThreadWire>,
    context: &AllThreadsDecodeContext<'_>,
) -> Result<Vec<SlackAllThread>, String> {
    let mut thread_ids = HashSet::with_capacity(threads.len());
    threads
        .into_iter()
        .map(|thread| {
            validated_thread(thread, &mut thread_ids)
                .and_then(|thread| decode_thread(thread, context))
        })
        .collect()
}

fn validated_thread(
    thread: SlackAllThreadWire,
    thread_ids: &mut HashSet<String>,
) -> Result<ValidatedThread, String> {
    validate_thread_root(&thread.root_msg)?;
    let id = format!("{}:{}", thread.root_msg.channel, thread.root_msg.thread_ts);
    if !thread_ids.insert(id.clone()) {
        return Err(format!(
            "Slack subscriptions.thread.getView returned duplicate thread {id}"
        ));
    }
    let conversation_id = thread.root_msg.channel.clone();
    let thread_timestamp = thread.root_msg.thread_ts.clone();
    let reply_count = thread.root_msg.reply_count;
    let mut unread_reply_ids = HashSet::new();
    let mut unread_reply_timestamps = Vec::new();
    let mut replies_by_timestamp = BTreeMap::new();
    for (reply, is_unread) in thread
        .unread_replies
        .into_iter()
        .map(|reply| (reply, true))
        .chain(
            thread
                .latest_replies
                .into_iter()
                .map(|reply| (reply, false)),
        )
    {
        let timestamp = validate_thread_reply(&reply, &conversation_id, &thread_timestamp)?;
        if is_unread && unread_reply_ids.insert(timestamp.as_str().to_string()) {
            unread_reply_timestamps.push(timestamp.clone());
        }
        replies_by_timestamp
            .entry(timestamp.sort_key())
            .or_insert_with(|| reply.into_value());
    }
    if usize::try_from(reply_count).is_ok_and(|count| count < replies_by_timestamp.len()) {
        return Err(format!(
            "Slack subscriptions.thread.getView returned more visible replies than reply_count for {id}"
        ));
    }
    Ok(ValidatedThread {
        id,
        conversation_id,
        thread_timestamp,
        reply_count,
        reply_users: thread.root_msg.reply_users.clone(),
        unread_reply_timestamps,
        parent_payload: thread.root_msg.into_value(),
        reply_payloads: replies_by_timestamp.into_values().collect(),
    })
}

fn decode_thread(
    thread: ValidatedThread,
    context: &AllThreadsDecodeContext<'_>,
) -> Result<SlackAllThread, String> {
    let visible_replies = thread
        .reply_payloads
        .into_iter()
        .map(|reply| {
            slack_message_from_value_with_context_in_timezone(
                reply,
                context.users,
                Some(context.sidebar),
                Some(context.self_user_id),
                context.timezone,
            )
        })
        .collect();
    let parent = slack_message_from_value_with_context_in_timezone(
        thread.parent_payload,
        context.users,
        Some(context.sidebar),
        Some(context.self_user_id),
        context.timezone,
    );
    if parent.id != thread.thread_timestamp {
        return Err(format!(
            "Slack subscriptions.thread.getView decoded root {} for thread {}",
            parent.id, thread.thread_timestamp
        ));
    }
    let AllThreadsConversation {
        name: conversation_name,
        kind: conversation_kind,
        direct_message_user_id,
        direct_message_presence,
        participant_authority,
    } = thread_conversation(context.sidebar, &thread.conversation_id);
    let participant_names =
        thread_participant_names(participant_authority, thread.reply_users, context);
    Ok(SlackAllThread {
        id: thread.id,
        conversation_id: thread.conversation_id,
        conversation_name,
        conversation_kind,
        direct_message_user_id,
        direct_message_presence,
        thread_timestamp: thread.thread_timestamp,
        parent,
        visible_replies,
        reply_count: thread.reply_count,
        unread_reply_timestamps: thread.unread_reply_timestamps,
        participant_names,
    })
}

fn thread_conversation(
    sidebar: &SlackSidebarSnapshot,
    conversation_id: &str,
) -> AllThreadsConversation {
    sidebar
        .item(conversation_id)
        .map(|item| {
            let kind = conversation_kind(conversation_id, item.kind.as_deref());
            let participant_authority = match kind {
                SlackConversationKind::DirectMessage if !item.label.trim().is_empty() => {
                    AllThreadsParticipantAuthority::DirectMessage {
                        peer_label: item.label.clone(),
                    }
                }
                SlackConversationKind::DirectMessage => {
                    AllThreadsParticipantAuthority::UnavailableDirectMessage
                }
                _ => AllThreadsParticipantAuthority::ThreadRepliers,
            };
            AllThreadsConversation {
                name: item.label.clone(),
                kind,
                direct_message_user_id: if kind == SlackConversationKind::DirectMessage {
                    item.user_id.clone()
                } else {
                    None
                },
                direct_message_presence: if kind == SlackConversationKind::DirectMessage {
                    item.presence
                } else {
                    None
                },
                participant_authority,
            }
        })
        .unwrap_or_else(|| {
            let kind = conversation_kind(conversation_id, None);
            AllThreadsConversation {
                name: conversation_id.to_string(),
                kind,
                direct_message_user_id: None,
                direct_message_presence: None,
                participant_authority: if kind == SlackConversationKind::DirectMessage {
                    AllThreadsParticipantAuthority::UnavailableDirectMessage
                } else {
                    AllThreadsParticipantAuthority::ThreadRepliers
                },
            }
        })
}

fn thread_participant_names(
    authority: AllThreadsParticipantAuthority,
    reply_users: Vec<String>,
    context: &AllThreadsDecodeContext<'_>,
) -> Vec<String> {
    match authority {
        AllThreadsParticipantAuthority::DirectMessage { peer_label } => {
            vec![peer_label, "you".to_string()]
        }
        AllThreadsParticipantAuthority::ThreadRepliers => reply_users
            .into_iter()
            .filter_map(|user_id| {
                (user_id == context.self_user_id)
                    .then(|| "you".to_string())
                    .or_else(|| {
                        context
                            .users
                            .get(&user_id)
                            .and_then(slack_user_display_name)
                    })
            })
            .collect(),
        AllThreadsParticipantAuthority::UnavailableDirectMessage => Vec::new(),
    }
}

fn validate_thread_root(
    root: &crate::live::internal_sidebar::threads::SlackAllThreadRootWire,
) -> Result<(), String> {
    if root.channel.trim().is_empty() {
        return Err(
            "Slack subscriptions.thread.getView returned a root without channel".to_string(),
        );
    }
    SlackMessageTimestamp::parse(&root.ts)?;
    SlackMessageTimestamp::parse(&root.thread_ts)?;
    if root.ts != root.thread_ts {
        return Err(format!(
            "Slack subscriptions.thread.getView returned mismatched root ts {} and thread_ts {}",
            root.ts, root.thread_ts
        ));
    }
    if !root.subscribed {
        return Err(format!(
            "Slack subscriptions.thread.getView returned unsubscribed thread {}",
            root.thread_ts
        ));
    }
    Ok(())
}

fn validate_thread_reply(
    reply: &crate::live::internal_sidebar::threads::SlackAllThreadReplyWire,
    conversation_id: &str,
    thread_timestamp: &str,
) -> Result<SlackMessageTimestamp, String> {
    let timestamp = SlackMessageTimestamp::parse(&reply.ts)?;
    if reply
        .thread_ts
        .as_deref()
        .is_some_and(|value| value != thread_timestamp)
    {
        return Err(format!(
            "Slack subscriptions.thread.getView returned reply {} for the wrong thread",
            reply.ts
        ));
    }
    if reply
        .channel
        .as_deref()
        .is_some_and(|value| value != conversation_id)
    {
        return Err(format!(
            "Slack subscriptions.thread.getView returned reply {} for the wrong channel",
            reply.ts
        ));
    }
    Ok(timestamp)
}

fn collect_thread_user_ids(
    thread: &crate::live::internal_sidebar::threads::SlackAllThreadWire,
    user_ids: &mut BTreeSet<String>,
) {
    user_ids.extend(thread.root_msg.reply_users.iter().cloned());
    collect_message_user_ids(&thread.root_msg.clone().into_value(), user_ids);
    for reply in thread
        .unread_replies
        .iter()
        .chain(thread.latest_replies.iter())
    {
        collect_message_user_ids(&reply.clone().into_value(), user_ids);
    }
}

fn collect_message_user_ids(value: &Value, user_ids: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(key.as_str(), "user" | "user_id" | "author_id") {
                    if let Some(user_id) = value.as_str().filter(|value| !value.is_empty()) {
                        user_ids.insert(user_id.to_string());
                    }
                } else if matches!(key.as_str(), "users" | "reply_users") {
                    if let Some(users) = value.as_array() {
                        user_ids.extend(
                            users
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string),
                        );
                    }
                }
                collect_message_user_ids(value, user_ids);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_message_user_ids(value, user_ids);
            }
        }
        _ => {}
    }
}

fn conversation_kind(conversation_id: &str, kind: Option<&str>) -> SlackConversationKind {
    match kind {
        Some("direct_message") => SlackConversationKind::DirectMessage,
        Some("group_message") => SlackConversationKind::GroupMessage,
        _ if conversation_id.starts_with('C') => SlackConversationKind::Channel,
        _ if conversation_id.starts_with('D') => SlackConversationKind::DirectMessage,
        _ if conversation_id.starts_with('G') => SlackConversationKind::PrivateChannel,
        _ => SlackConversationKind::Unknown,
    }
}
