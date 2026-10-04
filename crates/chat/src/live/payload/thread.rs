use std::collections::HashMap;

use crate::model::{
    SlackMessage, SlackMessageClientId, SlackMessageTimestamp, SlackThreadLoad,
    SlackThreadReadMetadata, SlackThreadReplyReceipt, SlackThreadSnapshot,
};
use chrono_tz::Tz;
use serde_json::{json, Value};

use super::{
    message::slack_thread_messages,
    sidebar_dom::SlackSidebarSnapshot,
    util::{slack_conversation_label, string_at},
};

pub(super) const SLACK_THREAD_PAGE_SIZE: usize = 20;

pub(super) struct SlackThreadPayloads<'a> {
    pub(super) team_id: &'a str,
    pub(super) conversation_id: &'a str,
    pub(super) thread_timestamp: &'a str,
    pub(super) cursor: Option<&'a str>,
    pub(super) channel_info: &'a Value,
    pub(super) replies: &'a Value,
    pub(super) users: &'a HashMap<String, Value>,
    pub(super) sidebar_snapshot: Option<&'a SlackSidebarSnapshot>,
    pub(super) self_user_id: Option<&'a str>,
    pub(super) self_timezone_id: Option<&'a str>,
    pub(super) timezone: Tz,
}

pub(super) struct SlackThreadReplyPayloads<'a> {
    pub(super) team_id: &'a str,
    pub(super) conversation_id: &'a str,
    pub(super) thread_timestamp: &'a SlackMessageTimestamp,
    pub(super) client_message_id: &'a SlackMessageClientId,
    pub(super) broadcast: bool,
    pub(super) response: &'a Value,
    pub(super) users: &'a HashMap<String, Value>,
    pub(super) self_user_id: &'a str,
    pub(super) self_timezone_id: Option<&'a str>,
    pub(super) timezone: Tz,
}

pub(super) fn slack_thread_from_payloads(
    payloads: SlackThreadPayloads<'_>,
) -> Result<SlackThreadLoad, String> {
    let raw_messages = payloads
        .replies
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack conversations.replies response missing messages array".to_string())?;
    validate_slack_thread_page(raw_messages)?;
    let messages = slack_thread_messages(
        payloads.replies,
        payloads.users,
        payloads.sidebar_snapshot,
        payloads.self_user_id,
        payloads.timezone,
    )?;
    let (parent, replies) = partition_slack_thread_messages(messages, payloads.thread_timestamp);
    let read_metadata = first_page_read_metadata(
        raw_messages,
        payloads.thread_timestamp,
        payloads.cursor,
        parent.as_ref(),
    )?;
    let next_cursor = string_at(payloads.replies, &["response_metadata", "next_cursor"])
        .filter(|cursor| !cursor.is_empty());
    if next_cursor.is_some() && next_cursor.as_deref() == payloads.cursor {
        return Err("Slack conversations.replies repeated its pagination cursor".to_string());
    }
    let channel = payloads
        .channel_info
        .get("channel")
        .ok_or_else(|| "Slack conversations.info response missing channel".to_string())?;
    let conversation_name = payloads
        .sidebar_snapshot
        .and_then(|snapshot| snapshot.item(payloads.conversation_id))
        .map(|item| item.label.clone())
        .filter(|label| !label.trim().is_empty())
        .or_else(|| slack_conversation_label(channel, payloads.users))
        .unwrap_or_else(|| payloads.conversation_id.to_string());
    Ok(SlackThreadLoad {
        snapshot: SlackThreadSnapshot {
            team_id: payloads.team_id.to_string(),
            conversation_id: payloads.conversation_id.to_string(),
            self_timezone_id: payloads.self_timezone_id.map(str::to_string),
            conversation_name,
            thread_timestamp: payloads.thread_timestamp.to_string(),
            parent,
            replies,
            next_cursor,
        },
        read_metadata,
    })
}

fn validate_slack_thread_page(messages: &[Value]) -> Result<(), String> {
    let message_count = messages.len();
    if message_count > SLACK_THREAD_PAGE_SIZE + 1 {
        return Err(format!(
            "Slack conversations.replies returned {message_count} messages for a page limited to {SLACK_THREAD_PAGE_SIZE}"
        ));
    }
    Ok(())
}

fn first_page_read_metadata(
    messages: &[Value],
    thread_timestamp: &str,
    cursor: Option<&str>,
    parent: Option<&SlackMessage>,
) -> Result<Option<SlackThreadReadMetadata>, String> {
    if cursor.is_some() {
        return Ok(None);
    }
    if parent.is_none() {
        return Err(format!(
            "Slack conversations.replies omitted parent message {thread_timestamp} from the first page"
        ));
    }
    slack_thread_read_metadata(messages, thread_timestamp).map(Some)
}

fn slack_thread_read_metadata(
    messages: &[Value],
    thread_timestamp: &str,
) -> Result<SlackThreadReadMetadata, String> {
    let root = messages
        .iter()
        .find(|message| message.get("ts").and_then(Value::as_str) == Some(thread_timestamp))
        .ok_or_else(|| {
            format!(
                "Slack conversations.replies omitted parent message {thread_timestamp} from the read-state page"
            )
        })?;
    let subscribed = match root.get("subscribed") {
        None => false,
        Some(Value::Bool(subscribed)) => *subscribed,
        Some(_) => {
            return Err(
                "Slack conversations.replies parent subscribed field must be a boolean".to_string(),
            )
        }
    };
    if !subscribed {
        return Ok(SlackThreadReadMetadata {
            subscribed,
            last_read: None,
            unread_count: 0,
        });
    }
    let last_read = match root.get("last_read") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(SlackMessageTimestamp::parse(value)?),
        Some(_) => {
            return Err(
                "Slack conversations.replies parent last_read must be a timestamp or null"
                    .to_string(),
            )
        }
    };
    let unread_count = match root.get("unread_count") {
        None => 0,
        Some(value) => value.as_u64().ok_or_else(|| {
            "Slack conversations.replies parent unread_count must be an unsigned integer"
                .to_string()
        })?,
    };
    let unread_count = u32::try_from(unread_count)
        .map_err(|_| "Slack conversations.replies parent unread_count exceeds u32".to_string())?;
    Ok(SlackThreadReadMetadata {
        subscribed,
        last_read,
        unread_count,
    })
}

pub(super) fn slack_thread_reply_receipt_from_payload(
    payloads: SlackThreadReplyPayloads<'_>,
) -> Result<SlackThreadReplyReceipt, String> {
    let SlackThreadReplyPayloads {
        team_id,
        conversation_id,
        thread_timestamp,
        client_message_id,
        broadcast,
        response,
        users,
        self_user_id,
        self_timezone_id,
        timezone,
    } = payloads;
    let (raw_reply, reply_timestamp) =
        validated_thread_reply(response, conversation_id, thread_timestamp)?;
    let mut reply =
        decoded_thread_reply(raw_reply, &reply_timestamp, users, self_user_id, timezone)?;
    match reply.client_message_id.as_ref() {
        Some(response_client_message_id) if response_client_message_id != client_message_id => {
            return Err(
                "Slack chat.postMessage thread reply response returned a different client message id"
                    .to_string(),
            );
        }
        Some(_) => {}
        None => reply.client_message_id = Some(client_message_id.clone()),
    }
    Ok(SlackThreadReplyReceipt {
        team_id: team_id.to_string(),
        conversation_id: conversation_id.to_string(),
        self_timezone_id: self_timezone_id.map(str::to_string),
        thread_timestamp: thread_timestamp.as_str().to_string(),
        broadcast,
        reply,
    })
}

fn validated_thread_reply(
    response: &Value,
    conversation_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
) -> Result<(Value, String), String> {
    let response_conversation_id = string_at(response, &["channel"])
        .ok_or_else(|| "Slack chat.postMessage response missing channel".to_string())?;
    if response_conversation_id != conversation_id {
        return Err(format!(
            "Slack chat.postMessage returned channel {response_conversation_id} for requested channel {conversation_id}"
        ));
    }
    let raw_reply = response
        .get("message")
        .cloned()
        .ok_or_else(|| "Slack chat.postMessage response missing message".to_string())?;
    let response_thread_timestamp = string_at(&raw_reply, &["thread_ts"]).ok_or_else(|| {
        "Slack chat.postMessage thread reply response missing message.thread_ts".to_string()
    })?;
    if response_thread_timestamp != thread_timestamp.as_str() {
        return Err(format!(
            "Slack chat.postMessage returned thread {} for requested thread {}",
            response_thread_timestamp,
            thread_timestamp.as_str()
        ));
    }
    let response_reply_timestamp = string_at(&raw_reply, &["ts"]).ok_or_else(|| {
        "Slack chat.postMessage thread reply response missing message.ts".to_string()
    })?;
    SlackMessageTimestamp::parse(&response_reply_timestamp).map_err(|error| {
        format!("Slack chat.postMessage returned an invalid reply timestamp: {error}")
    })?;
    if response_reply_timestamp == thread_timestamp.as_str() {
        return Err("Slack chat.postMessage returned the parent as its thread reply".to_string());
    }
    if string_at(response, &["ts"]).is_some_and(|timestamp| timestamp != response_reply_timestamp) {
        return Err(
            "Slack chat.postMessage returned mismatched top-level and message timestamps"
                .to_string(),
        );
    }
    Ok((raw_reply, response_reply_timestamp))
}

fn decoded_thread_reply(
    raw_reply: Value,
    response_reply_timestamp: &str,
    users: &HashMap<String, Value>,
    self_user_id: &str,
    timezone: Tz,
) -> Result<SlackMessage, String> {
    let mut replies = slack_thread_messages(
        &json!({ "messages": [raw_reply] }),
        users,
        None,
        Some(self_user_id),
        timezone,
    )?;
    if replies.len() != 1 {
        return Err(
            "Slack chat.postMessage response did not contain one visible thread reply".to_string(),
        );
    }
    let reply = replies
        .pop()
        .expect("one Slack thread reply was validated above");
    if reply.id != response_reply_timestamp {
        return Err(
            "Slack chat.postMessage reply timestamp changed while decoding its message".to_string(),
        );
    }
    Ok(reply)
}

fn partition_slack_thread_messages(
    messages: Vec<SlackMessage>,
    thread_timestamp: &str,
) -> (Option<SlackMessage>, Vec<SlackMessage>) {
    let mut parent = None;
    let mut replies = Vec::with_capacity(messages.len());
    for message in messages {
        if message.id == thread_timestamp {
            parent = Some(message);
        } else {
            replies.push(message);
        }
    }
    (parent, replies)
}
