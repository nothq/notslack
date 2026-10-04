mod excerpt;

use std::{
    collections::{BTreeSet, HashMap},
    sync::Mutex,
};

use serde_json::Value;

use crate::{
    live::api::{
        SlackApiClient, SlackQuickMessageRequest, SlackQuickSearchApiSnapshot,
        SlackQuickSearchMessageReference,
    },
    model::{
        SlackConversationKind, SlackQuickMessageQuery, SlackQuickMessageQueryScopeRef,
        SlackQuickSearchMessage, SlackQuickSearchSnapshot,
    },
};

use super::{nonempty, SlackSidebarSnapshot};
use crate::live::payload::util::{
    slack_conversation_kind, slack_conversation_label, slack_user_avatar_image_url,
    slack_user_display_name, string_at, SlackAvatarPurpose,
};
use excerpt::normalize_slack_quick_search_excerpt;

pub(in crate::live::payload) fn slack_quick_search_user_ids(
    messages: &[SlackQuickSearchMessageReference],
) -> BTreeSet<String> {
    messages
        .iter()
        .filter(|message| message.bot_id.is_none())
        .filter_map(|message| message.user_id.clone())
        .collect()
}

pub(in crate::live::payload) fn slack_quick_search_bot_ids(
    messages: &[SlackQuickSearchMessageReference],
) -> BTreeSet<String> {
    messages
        .iter()
        .filter_map(|message| message.bot_id.clone())
        .collect()
}

pub(in crate::live::payload) fn slack_quick_search_conversation_user_ids(
    messages: &[SlackQuickSearchMessageReference],
    conversations: &Value,
) -> BTreeSet<String> {
    let conversation_ids = messages
        .iter()
        .map(|message| message.conversation_id.as_str())
        .collect::<BTreeSet<_>>();
    conversations
        .get("channels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|channel| {
            string_at(channel, &["id"])
                .as_deref()
                .is_some_and(|id| conversation_ids.contains(id))
        })
        .filter_map(|channel| string_at(channel, &["user"]))
        .collect()
}

pub(in crate::live::payload) fn resolve_slack_quick_message_request(
    query: &SlackQuickMessageQuery,
    recent_channels: &[String],
    conversations: Option<&Value>,
) -> Result<Option<SlackQuickMessageRequest>, String> {
    let user_conversation_id = match query.scope() {
        SlackQuickMessageQueryScopeRef::Global | SlackQuickMessageQueryScopeRef::Channel(_) => None,
        SlackQuickMessageQueryScopeRef::User(user_id) => {
            resolve_slack_quick_message_user_conversation(user_id, conversations)?
        }
    };
    Ok(SlackQuickMessageRequest::resolve(
        query,
        recent_channels,
        user_conversation_id.as_deref(),
    ))
}

fn resolve_slack_quick_message_user_conversation(
    user_id: &str,
    conversations: Option<&Value>,
) -> Result<Option<String>, String> {
    let conversations = conversations
        .expect("user-scoped Slack quick-message query requires conversation directory");
    let conversation_ids = conversations
        .get("channels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|conversation| string_at(conversation, &["user"]).as_deref() == Some(user_id))
        .filter_map(|conversation| string_at(conversation, &["id"]))
        .filter(|conversation_id| conversation_id.starts_with('D'))
        .collect::<BTreeSet<_>>();
    match conversation_ids.len() {
        0 => Ok(None),
        1 => Ok(conversation_ids.into_iter().next()),
        _ => Err(format!(
            "Slack quick-message user scope {user_id} resolves to multiple direct-message conversations"
        )),
    }
}

pub(in crate::live::payload) fn load_slack_quick_search_bots(
    api: &SlackApiClient,
    bot_ids: BTreeSet<String>,
    bot_cache: &Mutex<HashMap<String, Value>>,
    bot_fetch_lock: &Mutex<()>,
) -> Result<HashMap<String, Value>, String> {
    let mut bots = cached_slack_bots(&bot_ids, bot_cache)?;
    if bots.len() == bot_ids.len() {
        return Ok(bots);
    }
    let _fetch_guard = bot_fetch_lock
        .lock()
        .map_err(|_| "Slack bot fetch mutex poisoned".to_string())?;
    bots = cached_slack_bots(&bot_ids, bot_cache)?;
    let missing_bot_ids = bot_ids
        .iter()
        .filter(|bot_id| !bots.contains_key(*bot_id))
        .cloned()
        .collect::<Vec<_>>();
    for bot_id in missing_bot_ids {
        let bot = load_slack_quick_search_bot(api, &bot_id)?;
        bots.insert(bot_id.clone(), bot.clone());
        bot_cache
            .lock()
            .map_err(|_| "Slack bot cache mutex poisoned".to_string())?
            .insert(bot_id, bot);
    }
    Ok(bots)
}

fn load_slack_quick_search_bot(api: &SlackApiClient, bot_id: &str) -> Result<Value, String> {
    let payload = api.post("bots.info", &[("bot", bot_id.to_string())])?;
    let bot = payload
        .get("bot")
        .cloned()
        .ok_or_else(|| format!("Slack bots.info response missing bot payload for {bot_id}"))?;
    let returned_id = string_at(&bot, &["id"])
        .ok_or_else(|| "Slack bots.info bot payload missing id".to_string())?;
    if returned_id != bot_id {
        return Err(format!(
            "Slack bots.info returned bot {returned_id} for {bot_id}"
        ));
    }
    Ok(bot)
}

fn cached_slack_bots(
    bot_ids: &BTreeSet<String>,
    bot_cache: &Mutex<HashMap<String, Value>>,
) -> Result<HashMap<String, Value>, String> {
    let cache = bot_cache
        .lock()
        .map_err(|_| "Slack bot cache mutex poisoned".to_string())?;
    Ok(bot_ids
        .iter()
        .filter_map(|bot_id| cache.get(bot_id).cloned().map(|bot| (bot_id.clone(), bot)))
        .collect())
}

pub(in crate::live::payload) fn finish_slack_quick_search_snapshot(
    snapshot: SlackQuickSearchApiSnapshot,
    recent_messages: Vec<SlackQuickSearchMessage>,
) -> SlackQuickSearchSnapshot {
    let SlackQuickSearchApiSnapshot {
        query,
        channels,
        people,
        direct_messages,
    } = snapshot;
    SlackQuickSearchSnapshot {
        query,
        channels,
        people,
        direct_messages,
        recent_messages,
    }
}

pub(in crate::live::payload) struct SlackQuickSearchDirectory<'a> {
    pub(in crate::live::payload) users: &'a HashMap<String, Value>,
    pub(in crate::live::payload) bots: &'a HashMap<String, Value>,
    pub(in crate::live::payload) sidebar: &'a SlackSidebarSnapshot,
}

/// Author display label and avatar image URL.
type SlackQuickSearchAuthor = (String, Option<String>);

pub(in crate::live::payload) fn enrich_slack_quick_search_messages(
    team_id: &str,
    messages: Vec<SlackQuickSearchMessageReference>,
    directory: &SlackQuickSearchDirectory<'_>,
    conversations: &Value,
) -> Vec<SlackQuickSearchMessage> {
    let conversations = conversations
        .get("channels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|channel| string_at(channel, &["id"]).map(|id| (id, channel)))
        .collect::<HashMap<_, _>>();
    messages
        .into_iter()
        .filter_map(|message| {
            enrich_slack_quick_search_message(team_id, message, directory, &conversations)
                .map_err(|error| eprintln!("Slack quick-message result skipped: {error}"))
                .ok()
        })
        .collect()
}

fn enrich_slack_quick_search_message(
    team_id: &str,
    message: SlackQuickSearchMessageReference,
    directory: &SlackQuickSearchDirectory<'_>,
    conversations: &HashMap<String, &Value>,
) -> Result<SlackQuickSearchMessage, String> {
    let SlackQuickSearchDirectory {
        users,
        bots,
        sidebar,
    } = *directory;
    let (author_label, avatar_image_url) = quick_search_author(&message, users, bots)?;
    let sidebar_item = sidebar.item(&message.conversation_id);
    let conversation = conversations.get(&message.conversation_id).copied();
    let conversation_kind = conversation
        .map(slack_conversation_kind)
        .filter(|kind| *kind != SlackConversationKind::Unknown)
        .map(Ok)
        .unwrap_or_else(|| {
            quick_search_conversation_kind(
                &message.conversation_id,
                sidebar_item.and_then(|item| item.kind.as_deref()),
            )
        })?;
    let item_label = sidebar_item
        .and_then(|item| nonempty(item.label.clone()))
        .or_else(|| conversation.and_then(|value| slack_conversation_label(value, users)))
        .ok_or_else(|| {
            "Slack quick-search message conversation has no display label".to_string()
        })?;
    let conversation_label = match conversation_kind {
        SlackConversationKind::DirectMessage => format!("Direct Message with {item_label}"),
        _ => item_label,
    };
    let (excerpt, highlights) =
        normalize_slack_quick_search_excerpt(&message.excerpt, &message.highlights, users, sidebar);
    Ok(SlackQuickSearchMessage {
        id: message.id,
        team_id: team_id.to_string(),
        conversation_id: message.conversation_id,
        conversation_label,
        conversation_kind,
        user_id: message.user_id,
        bot_id: message.bot_id,
        author_label,
        avatar_image_url,
        timestamp: message.timestamp,
        thread_timestamp: message.thread_timestamp,
        excerpt,
        highlights,
    })
}

fn quick_search_author(
    message: &SlackQuickSearchMessageReference,
    users: &HashMap<String, Value>,
    bots: &HashMap<String, Value>,
) -> Result<SlackQuickSearchAuthor, String> {
    if let Some(bot_id) = message.bot_id.as_deref() {
        return quick_search_bot_author(bot_id, bots);
    }
    let user_id = message
        .user_id
        .as_deref()
        .expect("validated Slack quick-search author must have a user or bot id");
    let user = users
        .get(user_id)
        .ok_or_else(|| format!("Slack quick-search message references missing user {user_id}"))?;
    let label = slack_user_display_name(user)
        .ok_or_else(|| "Slack quick-search message author has no display name".to_string())?;
    Ok((
        label,
        slack_user_avatar_image_url(user, SlackAvatarPurpose::Message),
    ))
}

fn quick_search_bot_author(
    bot_id: &str,
    bots: &HashMap<String, Value>,
) -> Result<SlackQuickSearchAuthor, String> {
    let bot = bots
        .get(bot_id)
        .ok_or_else(|| format!("Slack quick-search message references missing bot {bot_id}"))?;
    let label = string_at(bot, &["name"])
        .ok_or_else(|| "Slack quick-search bot has no display name".to_string())?;
    let avatar = [
        ["icons", "image_72"].as_slice(),
        ["icons", "image_48"].as_slice(),
        ["icons", "image_36"].as_slice(),
    ]
    .into_iter()
    .find_map(|path| string_at(bot, path));
    Ok((label, avatar))
}

fn quick_search_conversation_kind(
    conversation_id: &str,
    kind: Option<&str>,
) -> Result<SlackConversationKind, String> {
    match kind {
        Some("direct_message") => Ok(SlackConversationKind::DirectMessage),
        Some("group_message") => Ok(SlackConversationKind::GroupMessage),
        _ if conversation_id.starts_with('C') => Ok(SlackConversationKind::Channel),
        _ if conversation_id.starts_with('D') => Ok(SlackConversationKind::DirectMessage),
        _ if conversation_id.starts_with('G') => Ok(SlackConversationKind::PrivateChannel),
        _ => Err("Slack quick-search message has an unknown conversation kind".to_string()),
    }
}
