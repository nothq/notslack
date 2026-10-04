use std::collections::{BTreeMap, HashMap};

use crate::model::SlackMessageTimestamp;
use crate::model::{SlackConversationHistoryCursor, SlackConversationHistoryPage, SlackMessage};
use chrono_tz::Tz;
use serde_json::{json, Map, Value};

use super::slack_message_from_value_with_context_in_timezone;
use crate::live::payload::{
    sidebar_dom::SlackSidebarSnapshot, SLACK_CONVERSATION_HISTORY_PAGE_SIZE,
};

type SlackResponseMetadata = Map<String, Value>;

pub(in crate::live::payload) fn slack_messages(
    history: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
    self_user_id: Option<&str>,
    timezone: Tz,
) -> Vec<SlackMessage> {
    history
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|message| message.get("hidden").and_then(Value::as_bool) != Some(true))
        .rev()
        .map(|message| {
            slack_message_from_value_with_context_in_timezone(
                message,
                users,
                sidebar_snapshot,
                self_user_id,
                timezone,
            )
        })
        .filter(|message| !message.body.is_empty() || !message.attachments.is_empty())
        .collect()
}

pub(in crate::live::payload) fn slack_conversation_history_next_cursor(
    history: &Value,
    request_cursor: Option<&SlackConversationHistoryCursor>,
) -> Result<Option<SlackConversationHistoryCursor>, String> {
    Ok(slack_conversation_history_page_state(history, request_cursor)?.next_cursor)
}

pub(in crate::live::payload) struct SlackConversationHistoryPageState {
    pub(in crate::live::payload) timestamps: Vec<SlackMessageTimestamp>,
    pub(in crate::live::payload) next_cursor: Option<SlackConversationHistoryCursor>,
}

pub(in crate::live::payload) fn slack_conversation_history_page_state(
    history: &Value,
    request_cursor: Option<&SlackConversationHistoryCursor>,
) -> Result<SlackConversationHistoryPageState, String> {
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
    let timestamps = conversation_history_timestamps(messages)?;
    let next_cursor = conversation_history_next_cursor(history, request_cursor)?;
    Ok(SlackConversationHistoryPageState {
        timestamps,
        next_cursor,
    })
}

fn conversation_history_timestamps(
    messages: &[Value],
) -> Result<Vec<SlackMessageTimestamp>, String> {
    let timestamps = messages
        .iter()
        .map(|message| {
            let timestamp = message.get("ts").and_then(Value::as_str).ok_or_else(|| {
                "Slack conversations.history message missing timestamp".to_string()
            })?;
            SlackMessageTimestamp::parse(timestamp).map_err(|error| {
                format!("Slack conversations.history returned invalid timestamp: {error}")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if timestamps
        .windows(2)
        .any(|pair| pair[0].sort_key() <= pair[1].sort_key())
    {
        return Err(
            "Slack conversations.history messages must use strict newest-first timestamp order"
                .to_string(),
        );
    }
    Ok(timestamps)
}

fn conversation_history_next_cursor(
    history: &Value,
    request_cursor: Option<&SlackConversationHistoryCursor>,
) -> Result<Option<SlackConversationHistoryCursor>, String> {
    let Some(response_metadata) = conversation_history_response_metadata(history)? else {
        return Ok(None);
    };
    let cursor = match response_metadata.get("next_cursor") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(cursor)) => cursor,
        Some(_) => {
            return Err(
                "Slack conversations.history response_metadata.next_cursor must be a string"
                    .to_string(),
            );
        }
    };
    let next_cursor = (!cursor.is_empty())
        .then(|| SlackConversationHistoryCursor::new(cursor.clone()))
        .transpose()?;
    if next_cursor.is_some() && next_cursor.as_ref() == request_cursor {
        return Err("Slack conversations.history repeated its pagination cursor".to_string());
    }
    Ok(next_cursor)
}

fn conversation_history_response_metadata(
    history: &Value,
) -> Result<Option<&SlackResponseMetadata>, String> {
    match history.get("response_metadata") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(response_metadata)) => Ok(Some(response_metadata)),
        Some(_) => {
            Err("Slack conversations.history response_metadata must be an object".to_string())
        }
    }
}

pub(in crate::live::payload) fn merge_slack_conversation_history_pages(
    pages_oldest_to_newest: &[Value],
) -> Result<Value, String> {
    let mut messages_by_timestamp = BTreeMap::new();
    for history in pages_oldest_to_newest {
        let messages = history
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "Slack conversations.history response missing messages array".to_string()
            })?;
        let state = slack_conversation_history_page_state(history, None)?;
        for (timestamp, message) in state.timestamps.into_iter().zip(messages) {
            messages_by_timestamp.insert(timestamp.sort_key(), message.clone());
        }
    }
    let messages = messages_by_timestamp
        .into_values()
        .rev()
        .collect::<Vec<_>>();
    Ok(json!({ "messages": messages }))
}

pub(in crate::live::payload) struct SlackConversationHistoryPagePayloads<'a> {
    pub(in crate::live::payload) team_id: &'a str,
    pub(in crate::live::payload) conversation_id: &'a str,
    pub(in crate::live::payload) history: &'a Value,
    pub(in crate::live::payload) request_cursor: &'a SlackConversationHistoryCursor,
    pub(in crate::live::payload) users: &'a HashMap<String, Value>,
    pub(in crate::live::payload) sidebar_snapshot: Option<&'a SlackSidebarSnapshot>,
    pub(in crate::live::payload) self_user_id: Option<&'a str>,
    pub(in crate::live::payload) timezone: Tz,
}

pub(in crate::live::payload) fn slack_conversation_history_page(
    payloads: SlackConversationHistoryPagePayloads<'_>,
) -> Result<SlackConversationHistoryPage, String> {
    let next_cursor =
        slack_conversation_history_next_cursor(payloads.history, Some(payloads.request_cursor))?;
    Ok(SlackConversationHistoryPage {
        team_id: payloads.team_id.to_string(),
        conversation_id: payloads.conversation_id.to_string(),
        messages: slack_messages(
            payloads.history,
            payloads.users,
            payloads.sidebar_snapshot,
            payloads.self_user_id,
            payloads.timezone,
        ),
        next_cursor,
    })
}

pub(in crate::live::payload) fn slack_thread_messages(
    payload: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
    self_user_id: Option<&str>,
    timezone: Tz,
) -> Result<Vec<SlackMessage>, String> {
    let messages = payload
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack conversations.replies response missing messages array".to_string())?;
    Ok(messages
        .iter()
        .filter(|message| message.get("hidden").and_then(Value::as_bool) != Some(true))
        .cloned()
        .map(|message| {
            slack_message_from_value_with_context_in_timezone(
                message,
                users,
                sidebar_snapshot,
                self_user_id,
                timezone,
            )
        })
        .filter(|message| !message.body.is_empty() || !message.attachments.is_empty())
        .collect())
}
