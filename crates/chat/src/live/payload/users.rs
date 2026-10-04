mod collect;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::thread;
use std::time::Duration as StdDuration;

use serde_json::Value;
use time::OffsetDateTime;

use crate::live::{api::SlackApiClient, cache::SlackUserPayloadCache};

use self::collect::{
    collect_channel_user_id, collect_sidebar_snapshot_dm_user_ids, collect_workspace_user_ids,
    conversation_history_user_ids,
};
use super::{util::string_at, SLACK_CONVERSATION_HISTORY_PAGE_SIZE};

type CachedSlackUsers = (HashMap<String, Value>, Vec<String>);
type SlackUserRecord = (String, Value);

const SLACK_TARGETED_USER_INFO_LIMIT: usize = 4;
const SLACK_SIDEBAR_TARGETED_USER_INFO_LIMIT: usize = 4;
const SLACK_USER_INFO_WORKER_LIMIT: usize = 4;
const SLACK_USER_INFO_REQUEST_SPACING: StdDuration = StdDuration::from_millis(75);

pub(super) struct WorkspaceUserLoadInput<'a> {
    pub(super) active_conversation_id: &'a str,
    pub(super) channel_info: &'a Value,
    pub(super) conversations: &'a Value,
    pub(super) history: &'a Value,
    pub(super) sidebar_snapshot: Option<&'a super::sidebar_dom::SlackSidebarSnapshot>,
    pub(super) now: OffsetDateTime,
}

pub(super) struct SidebarUserLoadInput<'a> {
    pub(super) active_conversation_id: &'a str,
    pub(super) conversations: &'a Value,
    pub(super) sidebar_snapshot: &'a super::sidebar_dom::SlackSidebarSnapshot,
}

pub(super) struct ConversationUserLoadInput<'a> {
    pub(super) channel_info: &'a Value,
    pub(super) history: &'a Value,
}

pub(super) struct ActivityUserLoadInput<'a> {
    pub(super) history: &'a Value,
    pub(super) user_ids: BTreeSet<String>,
}

pub(super) fn load_slack_users_with_cache(
    api: &SlackApiClient,
    input: WorkspaceUserLoadInput<'_>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    let user_ids = collect_workspace_user_ids(&input);
    load_slack_user_ids_with_cache(
        api,
        user_ids,
        user_cache,
        user_fetch_lock,
        ensure_user_directory,
    )
}

pub(super) fn load_slack_conversation_users_with_cache(
    api: &SlackApiClient,
    input: ConversationUserLoadInput<'_>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
) -> Result<HashMap<String, Value>, String> {
    load_slack_conversation_history_window_users_with_cache(
        api,
        input.channel_info,
        &[input.history],
        user_cache,
        user_fetch_lock,
    )
}

pub(super) fn load_slack_conversation_history_window_users_with_cache(
    api: &SlackApiClient,
    channel_info: &Value,
    histories: &[&Value],
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
) -> Result<HashMap<String, Value>, String> {
    let (first_history, remaining_histories) = histories
        .split_first()
        .ok_or_else(|| "Slack conversation history window must contain a page".to_string())?;
    let history_message_limit = histories
        .len()
        .checked_mul(SLACK_CONVERSATION_HISTORY_PAGE_SIZE)
        .expect("bounded Slack history page count must fit usize");
    let mut user_ids = conversation_history_user_ids(first_history)?;
    for history in remaining_histories {
        user_ids.extend(conversation_history_user_ids(history)?);
    }
    collect_channel_user_id(channel_info, &mut user_ids);
    load_slack_targeted_user_ids_with_cache(
        api,
        user_ids,
        user_cache,
        user_fetch_lock,
        history_message_limit
            .checked_add(1)
            .expect("bounded Slack history user limit must fit usize"),
    )
}

pub(super) fn load_cached_slack_conversation_users(
    input: ConversationUserLoadInput<'_>,
    user_cache: &SlackUserPayloadCache,
) -> Result<HashMap<String, Value>, String> {
    let mut user_ids = conversation_history_user_ids(input.history)?;
    collect_channel_user_id(input.channel_info, &mut user_ids);
    let (users, _) = load_cached_users(user_cache, &user_ids)?;
    Ok(users)
}

pub(super) fn load_slack_conversation_history_users_with_cache(
    api: &SlackApiClient,
    history: &Value,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    load_slack_user_ids_with_cache(
        api,
        conversation_history_user_ids(history)?,
        user_cache,
        user_fetch_lock,
        ensure_user_directory,
    )
}

pub(super) fn load_slack_activity_users_with_cache(
    api: &SlackApiClient,
    input: ActivityUserLoadInput<'_>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    let mut user_ids = conversation_history_user_ids(input.history)?;
    user_ids.extend(input.user_ids);
    load_slack_user_ids_with_cache(
        api,
        user_ids,
        user_cache,
        user_fetch_lock,
        ensure_user_directory,
    )
}

pub(super) fn load_slack_sidebar_users_with_cache(
    api: &SlackApiClient,
    input: SidebarUserLoadInput<'_>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
) -> Result<HashMap<String, Value>, String> {
    let channels = input
        .conversations
        .get("channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut user_ids = BTreeSet::new();
    collect_sidebar_snapshot_dm_user_ids(input.sidebar_snapshot, &channels, &mut user_ids);
    if let Some(channel) = channels.iter().find(|channel| {
        string_at(channel, &["id"]).as_deref() == Some(input.active_conversation_id)
    }) {
        collect_channel_user_id(
            &serde_json::json!({
                "channel": channel,
            }),
            &mut user_ids,
        );
    }
    load_slack_bounded_targeted_user_ids_with_cache(
        api,
        user_ids,
        user_cache,
        user_fetch_lock,
        SLACK_SIDEBAR_TARGETED_USER_INFO_LIMIT,
    )
}

pub(super) fn load_slack_search_users_with_cache(
    api: &SlackApiClient,
    user_ids: BTreeSet<String>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    load_slack_user_ids_with_cache(
        api,
        user_ids,
        user_cache,
        user_fetch_lock,
        ensure_user_directory,
    )
}

pub(super) fn load_slack_member_users_with_cache(
    api: &SlackApiClient,
    user_ids: BTreeSet<String>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    let (users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    ensure_user_directory()?;
    let (users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    let _fetch_guard = user_fetch_lock
        .lock()
        .map_err(|_| "slack user fetch mutex poisoned".to_string())?;
    let (mut users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    let fetched_users = load_missing_slack_users(api, missing_user_ids)?;
    merge_slack_users(&mut users, user_cache, fetched_users)?;
    Ok(users)
}

fn load_slack_user_ids_with_cache(
    api: &SlackApiClient,
    user_ids: BTreeSet<String>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    ensure_user_directory: impl FnOnce() -> Result<(), String>,
) -> Result<HashMap<String, Value>, String> {
    let (users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    if missing_user_ids.len() > SLACK_TARGETED_USER_INFO_LIMIT {
        ensure_user_directory()?;
        let (users, remaining_user_ids) = load_cached_users(user_cache, &user_ids)?;
        if remaining_user_ids.is_empty()
            || remaining_user_ids.len() > SLACK_TARGETED_USER_INFO_LIMIT
        {
            return Ok(users);
        }
    }
    let _fetch_guard = user_fetch_lock
        .lock()
        .map_err(|_| "slack user fetch mutex poisoned".to_string())?;
    let (mut users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    let fetched_users = load_missing_slack_users(api, missing_user_ids)?;
    merge_slack_users(&mut users, user_cache, fetched_users)?;
    Ok(users)
}

fn load_slack_targeted_user_ids_with_cache(
    api: &SlackApiClient,
    user_ids: BTreeSet<String>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    targeted_user_info_limit: usize,
) -> Result<HashMap<String, Value>, String> {
    let (users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.len() > targeted_user_info_limit {
        return Err(format!(
            "Slack conversation references {} uncached users for a history window limited to {} messages",
            missing_user_ids.len(),
            targeted_user_info_limit.saturating_sub(1),
        ));
    }
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    let _fetch_guard = user_fetch_lock
        .lock()
        .map_err(|_| "slack user fetch mutex poisoned".to_string())?;
    let (mut users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    let fetched_users = load_missing_slack_users(api, missing_user_ids)?;
    merge_slack_users(&mut users, user_cache, fetched_users)?;
    Ok(users)
}

fn load_slack_bounded_targeted_user_ids_with_cache(
    api: &SlackApiClient,
    user_ids: BTreeSet<String>,
    user_cache: &SlackUserPayloadCache,
    user_fetch_lock: &Mutex<()>,
    targeted_user_info_limit: usize,
) -> Result<HashMap<String, Value>, String> {
    let (users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    if missing_user_ids.is_empty() {
        return Ok(users);
    }
    let _fetch_guard = user_fetch_lock
        .lock()
        .map_err(|_| "slack user fetch mutex poisoned".to_string())?;
    let (mut users, missing_user_ids) = load_cached_users(user_cache, &user_ids)?;
    let fetched_users = load_missing_slack_users(
        api,
        missing_user_ids
            .into_iter()
            .take(targeted_user_info_limit)
            .collect(),
    )?;
    merge_slack_users(&mut users, user_cache, fetched_users)?;
    Ok(users)
}

fn load_cached_users(
    user_cache: &SlackUserPayloadCache,
    user_ids: &BTreeSet<String>,
) -> Result<CachedSlackUsers, String> {
    let mut users = HashMap::new();
    let mut missing_user_ids = Vec::new();
    let cached_users = user_cache
        .lock()
        .map_err(|_| "slack user cache mutex poisoned".to_string())?;
    for user_id in user_ids {
        if let Some(user) = cached_users.get(user_id) {
            users.insert(user_id.clone(), user.clone());
        } else {
            missing_user_ids.push(user_id.clone());
        }
    }
    Ok((users, missing_user_ids))
}

fn merge_slack_users(
    users: &mut HashMap<String, Value>,
    user_cache: &SlackUserPayloadCache,
    fetched_users: Vec<SlackUserRecord>,
) -> Result<(), String> {
    for (user_id, user) in &fetched_users {
        users.insert(user_id.clone(), user.clone());
    }
    user_cache.extend(fetched_users)?;
    Ok(())
}

fn load_missing_slack_users(
    api: &SlackApiClient,
    missing_user_ids: Vec<String>,
) -> Result<Vec<SlackUserRecord>, String> {
    if missing_user_ids.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = missing_user_ids.len().min(SLACK_USER_INFO_WORKER_LIMIT);
    let chunk_size = missing_user_ids.len().div_ceil(worker_count);
    thread::scope(|scope| {
        missing_user_ids
            .chunks(chunk_size)
            .map(|user_ids| scope.spawn(move || load_missing_slack_user_chunk(api, user_ids)))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| "Slack users.info worker panicked".to_string())?
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|users| users.into_iter().flatten().collect())
    })
}

fn load_missing_slack_user_chunk(
    api: &SlackApiClient,
    missing_user_ids: &[String],
) -> Result<Vec<SlackUserRecord>, String> {
    let mut users = Vec::with_capacity(missing_user_ids.len());
    for user_id in missing_user_ids {
        if !users.is_empty() {
            thread::sleep(SLACK_USER_INFO_REQUEST_SPACING);
        }
        let payload = api.post("users.info", &[("user", user_id.clone())])?;
        let user = payload.get("user").cloned().ok_or_else(|| {
            format!("Slack users.info response missing user payload for {user_id}")
        })?;
        users.push((user_id.clone(), user));
    }
    Ok(users)
}
