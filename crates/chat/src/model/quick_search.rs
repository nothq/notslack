use serde::{Deserialize, Serialize};

use super::{SlackConversationKind, SlackMessageTimestamp};

const SLACK_QUICK_MESSAGE_MIN_QUERY_UTF16_LEN: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackQuickMessageQuery {
    text: String,
    scope: SlackQuickMessageQueryScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SlackQuickMessageQueryScope {
    Global,
    Channel(String),
    User(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackQuickMessageQueryScopeRef<'a> {
    Global,
    Channel(&'a str),
    User(&'a str),
}

impl SlackQuickMessageQuery {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.encode_utf16().count() < SLACK_QUICK_MESSAGE_MIN_QUERY_UTF16_LEN {
            return None;
        }
        let Some(filter) = slack_quick_message_in_filter(value) else {
            return (!contains_slack_search_modifier(value)).then(|| Self {
                text: value.to_string(),
                scope: SlackQuickMessageQueryScope::Global,
            });
        };
        let filter = filter?;
        let text = slack_quick_message_text_without_filter(value, filter.range.clone())?;
        if text.encode_utf16().count() < SLACK_QUICK_MESSAGE_MIN_QUERY_UTF16_LEN
            || contains_slack_search_modifier(&text)
        {
            return None;
        }
        Some(Self {
            text,
            scope: filter.scope,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn scope(&self) -> SlackQuickMessageQueryScopeRef<'_> {
        match &self.scope {
            SlackQuickMessageQueryScope::Global => SlackQuickMessageQueryScopeRef::Global,
            SlackQuickMessageQueryScope::Channel(conversation_id) => {
                SlackQuickMessageQueryScopeRef::Channel(conversation_id)
            }
            SlackQuickMessageQueryScope::User(user_id) => {
                SlackQuickMessageQueryScopeRef::User(user_id)
            }
        }
    }
}

struct SlackQuickMessageInFilter {
    range: std::ops::Range<usize>,
    scope: SlackQuickMessageQueryScope,
}

fn slack_quick_message_in_filter(query: &str) -> Option<Option<SlackQuickMessageInFilter>> {
    let mut starts = query.match_indices("in:<").filter_map(|(start, _)| {
        (start == 0
            || query[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace))
        .then_some(start)
    });
    let start = starts.next()?;
    if starts.next().is_some() {
        return Some(None);
    }
    let reference_start = start + "in:<".len();
    let close = query[reference_start..].find('>')? + reference_start;
    let end = close + '>'.len_utf8();
    if query[end..]
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return Some(None);
    }
    let (reference, label) = query[reference_start..close].split_once('|')?;
    if label.trim().is_empty() {
        return Some(None);
    }
    let prefix = reference.get(..1)?;
    let id = reference.get(1..)?;
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Some(None);
    }
    let scope = match prefix {
        "#" if matches!(id.as_bytes().first(), Some(b'C' | b'G')) => {
            SlackQuickMessageQueryScope::Channel(id.to_string())
        }
        "@" if matches!(id.as_bytes().first(), Some(b'U' | b'W')) => {
            SlackQuickMessageQueryScope::User(id.to_string())
        }
        _ => return Some(None),
    };
    Some(Some(SlackQuickMessageInFilter {
        range: start..end,
        scope,
    }))
}

fn slack_quick_message_text_without_filter(
    query: &str,
    filter: std::ops::Range<usize>,
) -> Option<String> {
    let before = query[..filter.start].trim();
    let after = query[filter.end..].trim();
    let text = match (before.is_empty(), after.is_empty()) {
        (true, true) => return None,
        (false, true) => before.to_string(),
        (true, false) => after.to_string(),
        (false, false) => format!("{before} {after}"),
    };
    Some(text)
}

fn contains_slack_search_modifier(query: &str) -> bool {
    query.split_ascii_whitespace().any(|token| {
        let Some((modifier, _)) = token.split_once(':') else {
            return false;
        };
        matches!(
            modifier.to_ascii_lowercase().as_str(),
            "after" | "before" | "during" | "from" | "has" | "in" | "is" | "on" | "to" | "with"
        )
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackQuickSearchSnapshot {
    pub query: String,
    pub channels: Vec<SlackQuickSearchConversation>,
    pub people: Vec<SlackQuickSearchPerson>,
    pub direct_messages: Vec<SlackQuickSearchConversation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_messages: Vec<SlackQuickSearchMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackQuickSearchConversation {
    pub id: String,
    pub team_id: String,
    pub kind: SlackConversationKind,
    pub label: String,
    pub is_member: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_user_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackQuickSearchPerson {
    pub id: String,
    pub team_id: String,
    pub username: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub real_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackQuickSearchMessage {
    pub id: String,
    pub team_id: String,
    pub conversation_id: String,
    pub conversation_label: String,
    pub conversation_kind: SlackConversationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub author_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    pub timestamp: SlackMessageTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub excerpt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub highlights: Vec<SlackQuickSearchHighlight>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackQuickSearchHighlight {
    pub start: usize,
    pub end: usize,
}
