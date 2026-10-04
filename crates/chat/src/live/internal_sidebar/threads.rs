use crate::model::{SlackAllThreadsCursor, SlackThreadReadTarget};
use serde::Deserialize;
use serde_json::{Map, Number, Value};

use super::SlackInternalSidebarClient;

const THREADS_VIEW: &str = "subscriptions.thread.getView";
const THREAD_MARK: &str = "subscriptions.thread.mark";
const THREADS_PAGE_SIZE: usize = 8;

impl SlackInternalSidebarClient {
    pub(crate) fn load_all_threads(
        &self,
        cursor: Option<&SlackAllThreadsCursor>,
    ) -> Result<SlackAllThreadsWirePage, String> {
        let mut params = vec![
            ("limit", THREADS_PAGE_SIZE.to_string()),
            (
                "fetch_threads_state",
                if cursor.is_none() { "true" } else { "false" }.to_string(),
            ),
            ("priority_mode", "all".to_string()),
            (
                "_x_reason",
                if cursor.is_none() {
                    "refreshThreads"
                } else {
                    "loadMoreThreads"
                }
                .to_string(),
            ),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        if let Some(cursor) = cursor {
            params.push(("current_ts", cursor.as_str().to_string()));
        }
        let body = self.post_internal_method(THREADS_VIEW, params)?;
        decode_all_threads(&body, cursor)
    }

    pub(crate) fn mark_thread_read(&self, target: &SlackThreadReadTarget) -> Result<(), String> {
        let body = self.post_internal_method(
            THREAD_MARK,
            vec![
                ("channel", target.conversation_id().to_string()),
                ("thread_ts", target.thread_timestamp().to_string()),
                ("ts", target.message_timestamp().to_string()),
                ("read", "1".to_string()),
                ("_x_reason", "marking-thread".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        decode_thread_mark(&body)
    }
}

fn decode_thread_mark(body: &str) -> Result<(), String> {
    let response = serde_json::from_str::<SlackThreadMarkResponse>(body)
        .map_err(|error| format!("failed to decode Slack {THREAD_MARK} response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "Slack {THREAD_MARK} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    Ok(())
}

fn decode_all_threads(
    body: &str,
    request_cursor: Option<&SlackAllThreadsCursor>,
) -> Result<SlackAllThreadsWirePage, String> {
    let response = serde_json::from_str::<SlackAllThreadsResponse>(body)
        .map_err(|error| format!("failed to decode Slack {THREADS_VIEW} response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "Slack {THREADS_VIEW} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    let threads = response
        .threads
        .ok_or_else(|| format!("Slack {THREADS_VIEW} response omitted threads"))?;
    if threads.len() > THREADS_PAGE_SIZE {
        return Err(format!(
            "Slack {THREADS_VIEW} returned {} threads for a {THREADS_PAGE_SIZE}-thread page",
            threads.len()
        ));
    }
    let has_more = response
        .has_more
        .ok_or_else(|| format!("Slack {THREADS_VIEW} response omitted has_more"))?;
    let total_unread_replies = match (request_cursor, response.total_unread_replies) {
        (None, None) => {
            return Err(format!(
                "Slack {THREADS_VIEW} initial response omitted total_unread_replies"
            ));
        }
        (_, total_unread_replies) => total_unread_replies,
    };
    if has_more && threads.is_empty() {
        return Err(format!(
            "Slack {THREADS_VIEW} returned has_more without a pagination row"
        ));
    }
    let next_cursor = has_more
        .then(|| {
            threads
                .last()
                .expect("non-empty checked above")
                .sort_timestamp()
                .and_then(|timestamp| SlackAllThreadsCursor::new(timestamp.to_string()))
        })
        .transpose()?;
    if next_cursor.is_some() && next_cursor.as_ref() == request_cursor {
        return Err(format!(
            "Slack {THREADS_VIEW} repeated its pagination cursor"
        ));
    }
    Ok(SlackAllThreadsWirePage {
        threads,
        next_cursor,
        total_unread_replies,
    })
}

#[derive(Debug, Deserialize)]
struct SlackAllThreadsResponse {
    ok: bool,
    error: Option<String>,
    threads: Option<Vec<SlackAllThreadWire>>,
    has_more: Option<bool>,
    total_unread_replies: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SlackThreadMarkResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SlackAllThreadWire {
    pub(crate) root_msg: SlackAllThreadRootWire,
    #[serde(default = "no_unread_thread_replies")]
    pub(crate) unread_replies: Vec<SlackAllThreadReplyWire>,
    pub(crate) latest_replies: Vec<SlackAllThreadReplyWire>,
}

fn no_unread_thread_replies() -> Vec<SlackAllThreadReplyWire> {
    Vec::new()
}

impl SlackAllThreadWire {
    fn sort_timestamp(&self) -> Result<&str, String> {
        self.unread_replies
            .iter()
            .chain(self.latest_replies.iter())
            .map(|reply| reply.ts.as_str())
            .max_by(|left, right| timestamp_key(left).cmp(&timestamp_key(right)))
            .or(Some(self.root_msg.ts.as_str()))
            .ok_or_else(|| format!("Slack {THREADS_VIEW} thread omitted a sort timestamp"))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SlackAllThreadRootWire {
    pub(crate) channel: String,
    pub(crate) ts: String,
    pub(crate) thread_ts: String,
    pub(crate) subscribed: bool,
    pub(crate) reply_count: u32,
    pub(crate) reply_users: Vec<String>,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

impl SlackAllThreadRootWire {
    pub(crate) fn into_value(mut self) -> Value {
        self.fields
            .insert("channel".to_string(), Value::String(self.channel.clone()));
        self.fields
            .insert("ts".to_string(), Value::String(self.ts.clone()));
        self.fields.insert(
            "thread_ts".to_string(),
            Value::String(self.thread_ts.clone()),
        );
        self.fields
            .insert("subscribed".to_string(), Value::Bool(self.subscribed));
        self.fields.insert(
            "reply_count".to_string(),
            Value::Number(Number::from(self.reply_count)),
        );
        self.fields.insert(
            "reply_users".to_string(),
            Value::Array(
                self.reply_users
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        Value::Object(self.fields)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SlackAllThreadReplyWire {
    pub(crate) ts: String,
    pub(crate) thread_ts: Option<String>,
    pub(crate) channel: Option<String>,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

impl SlackAllThreadReplyWire {
    pub(crate) fn into_value(mut self) -> Value {
        self.fields
            .insert("ts".to_string(), Value::String(self.ts.clone()));
        if let Some(thread_ts) = self.thread_ts {
            self.fields
                .insert("thread_ts".to_string(), Value::String(thread_ts));
        }
        if let Some(channel) = self.channel {
            self.fields
                .insert("channel".to_string(), Value::String(channel));
        }
        Value::Object(self.fields)
    }
}

pub(crate) struct SlackAllThreadsWirePage {
    pub(crate) threads: Vec<SlackAllThreadWire>,
    pub(crate) next_cursor: Option<SlackAllThreadsCursor>,
    pub(crate) total_unread_replies: Option<u32>,
}

fn timestamp_key(timestamp: &str) -> (u64, u32) {
    let Some((seconds, fractional)) = timestamp.split_once('.') else {
        return (0, 0);
    };
    (
        seconds.parse().unwrap_or_default(),
        fractional.parse().unwrap_or_default(),
    )
}
