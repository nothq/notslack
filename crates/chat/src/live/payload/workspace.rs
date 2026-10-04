use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, Weak},
    thread,
    time::{Duration, Instant},
};

use crate::model::{
    SlackActivityCursor, SlackActivitySnapshot, SlackConnectedOrganization,
    SlackConversationHistoryCursor, SlackConversationHistoryPage, SlackConversationMembersCursor,
    SlackConversationMembersSnapshot, SlackConversationSnapshot, SlackDmInboxSnapshot, SlackFileId,
    SlackFilesRequest, SlackFilesSnapshot, SlackMessage, SlackMessageDraft,
    SlackMessageForwardReceipt, SlackMessageSendReceipt, SlackProfile, SlackSearchSnapshot,
    SlackShellSnapshot, SlackSidebarSnapshot as SlackSidebarStageSnapshot, SlackThreadReplyReceipt,
    SlackThreadSnapshot, SlackUploadFile, SlackWorkspace,
};
use crate::model::{
    SlackMessageTimestamp, SlackReactionMutation, SlackReactionName, SlackReactionTarget,
    SlackSavedMessageMutation, SlackStarMutation,
};
use serde_json::Value;

use crate::live::{
    api::SlackApiClient,
    cache::SlackUserPayloadCache,
    internal_sidebar::{SlackInternalSidebarClient, SlackTeamDomain, SlackWebSessionCredentials},
    payload::sidebar_dom::SlackSidebarSnapshot,
};

mod actions;
mod all_threads;
mod bookmark_folder;
mod build;
mod channel_details;
mod conversation;
mod conversation_files;
mod core;
mod drafts_sent;
mod file_metadata;
mod file_staging;
mod later;
mod members;
mod new_message;
mod notifications;
mod ops;
mod pins;
mod preferences;
mod reaction_catalog;
mod requests;
mod state;
mod tabs;
#[cfg(test)]
mod tests;

pub(crate) use actions::SlackFileShareMutationOutcome;

use super::message::{
    slack_conversation_history_next_cursor, slack_conversation_history_page,
    slack_message_send_receipt_from_payload, SlackConversationHistoryPagePayloads,
    SlackMessageSendReceiptPayloads,
};
use super::search::{
    enrich_slack_quick_search_messages, enrich_slack_search_snapshot,
    finish_slack_quick_search_snapshot, load_slack_quick_search_bots,
    resolve_slack_quick_message_request, slack_quick_search_bot_ids,
    slack_quick_search_conversation_user_ids, slack_quick_search_user_ids, slack_search_user_ids,
    SlackQuickSearchDirectory,
};
use super::thread::{
    slack_thread_from_payloads, slack_thread_reply_receipt_from_payload, SlackThreadPayloads,
    SlackThreadReplyPayloads, SLACK_THREAD_PAGE_SIZE,
};
use super::users::{
    load_cached_slack_conversation_users, load_slack_activity_users_with_cache,
    load_slack_conversation_history_users_with_cache,
    load_slack_conversation_history_window_users_with_cache,
    load_slack_conversation_users_with_cache, load_slack_search_users_with_cache,
    load_slack_sidebar_users_with_cache, load_slack_users_with_cache, ActivityUserLoadInput,
    ConversationUserLoadInput, SidebarUserLoadInput, WorkspaceUserLoadInput,
};
use super::util::SlackTimezone;
use build::{
    slack_conversation_from_payloads, slack_last_read_timestamp, slack_shell_from_payloads,
    slack_sidebar_from_payloads, slack_workspace_from_payloads, SlackConversationPayloads,
    SlackWorkspacePayloads,
};
const SLACK_LOAD_PROFILE_ENV: &str = "NOTSLACK_SLACK_LOAD_PROFILE";
const SLACK_SIDEBAR_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const SLACK_SIDEBAR_REFRESH_RETRY_INTERVAL: Duration = Duration::from_secs(15);
const SLACK_CONVERSATION_DETAIL_CACHE_INTERVAL: Duration = Duration::from_secs(60);
const SLACK_DND_STATUS_CACHE_INTERVAL: Duration = Duration::from_secs(30);
const SLACK_QUICK_MESSAGE_CACHE_INTERVAL: Duration = Duration::from_secs(30);
const SLACK_QUICK_MESSAGE_CACHE_LIMIT: usize = 20;

type SlackUserCache = Arc<SlackUserPayloadCache>;
type SlackBotPayloadCache = Arc<Mutex<HashMap<String, Value>>>;
type SlackQuickMessageCache = Arc<Mutex<VecDeque<CachedSlackQuickMessages>>>;
type SlackQuickMessageFetchLocks = Arc<Mutex<HashMap<String, Weak<Mutex<()>>>>>;
type SlackConversationDetailCache = Arc<Mutex<HashMap<String, CachedSlackConversationDetail>>>;
type SlackConversationDetailFetchLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;
type SlackDndStatusCache = Arc<Mutex<HashMap<String, CachedSlackDndStatus>>>;
type SlackDndStatusFetchLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;
/// URLs whose preview failed to load, so they are not fetched again. Loaded
/// previews are not kept here: the UI caches the decoded, downscaled image.
type SlackAttachmentPreviewCache = Arc<Mutex<HashSet<String>>>;
type SlackFileMetadataCache =
    Arc<Mutex<HashMap<SlackFileId, file_metadata::CachedSlackFileMetadata>>>;
type SlackFileMetadataFetchLocks = Arc<Mutex<HashMap<SlackFileId, Arc<Mutex<()>>>>>;
type SlackCanvasTabMetadataCache = Arc<Mutex<HashMap<String, tabs::SlackCanvasTabMetadata>>>;
type SlackCanvasTabHydrationStarted = Arc<Mutex<HashSet<String>>>;
type SlackConnectedOrganizationCache = Arc<Mutex<HashMap<String, SlackConnectedOrganization>>>;
type SlackSidebarSnapshotCache = Arc<Mutex<Option<CachedSlackSidebarSnapshot>>>;
type SlackUserPreferencesCache = Arc<Mutex<Option<preferences::SlackUserPreferences>>>;
type SlackReactionCatalogCache =
    Arc<Mutex<Option<Arc<crate::model::SlackReactionCatalogSnapshot>>>>;
type SlackApiCallCounts = HashMap<String, usize>;
type SlackApiCallCountSnapshot = Option<SlackApiCallCounts>;
type SlackSelfUser = (String, Value);
type SlackSelfUserTask = thread::JoinHandle<Result<SlackSelfUser, String>>;

#[derive(Clone)]
pub struct SlackLiveWorkspaceLoader {
    pub(super) api: SlackApiClient,
    pub(super) sidebar_api: SlackInternalSidebarClient,
    pub(super) team_id: String,
    pub(super) timezone: SlackTimezone,
    pub(super) auth_test_cache: Arc<Mutex<Option<Value>>>,
    pub(super) auth_test_fetch_lock: Arc<Mutex<()>>,
    pub(super) team_info_cache: Arc<Mutex<Option<Value>>>,
    pub(super) conversations_cache: Arc<Mutex<Option<CachedSlackConversations>>>,
    pub(super) sidebar_snapshot_cache: SlackSidebarSnapshotCache,
    pub(super) user_cache: SlackUserCache,
    pub(super) user_fetch_lock: Arc<Mutex<()>>,
    pub(super) bot_cache: SlackBotPayloadCache,
    pub(super) bot_fetch_lock: Arc<Mutex<()>>,
    pub(super) quick_message_cache: SlackQuickMessageCache,
    pub(super) quick_message_fetch_locks: SlackQuickMessageFetchLocks,
    pub(super) conversation_detail_cache: SlackConversationDetailCache,
    pub(super) conversation_detail_fetch_locks: SlackConversationDetailFetchLocks,
    pub(super) connected_organization_cache: SlackConnectedOrganizationCache,
    user_preferences_cache: SlackUserPreferencesCache,
    user_preferences_fetch_lock: Arc<Mutex<()>>,
    user_preferences_mutation_lock: Arc<Mutex<()>>,
    reaction_catalog_cache: SlackReactionCatalogCache,
    reaction_catalog_fetch_lock: Arc<Mutex<()>>,
    dnd_status_cache: SlackDndStatusCache,
    dnd_status_fetch_locks: SlackDndStatusFetchLocks,
    pub(super) attachment_preview_cache: SlackAttachmentPreviewCache,
    file_metadata_cache: SlackFileMetadataCache,
    file_metadata_fetch_locks: SlackFileMetadataFetchLocks,
    file_staging_ledger: file_staging::SlackFileStagingLedger,
    canvas_tab_metadata_cache: SlackCanvasTabMetadataCache,
    canvas_tab_hydration_started: SlackCanvasTabHydrationStarted,
}

pub(super) struct WorkspacePreloadTasks {
    pub(super) team_info: thread::JoinHandle<Result<Value, String>>,
    pub(super) self_user: SlackSelfUserTask,
}

pub(super) struct SlackChannelPayloads {
    channel_info: Value,
    history: Value,
    peer_notifications_paused: bool,
}

pub(super) struct SlackConversationCorePayloads {
    channel_info: Value,
    history: Value,
}

pub(super) struct CachedSlackConversations {
    pub(super) payload: Value,
}

#[derive(Clone)]
pub(super) struct CachedSlackQuickMessages {
    pub(super) key: String,
    pub(super) messages: Vec<crate::model::SlackQuickSearchMessage>,
    pub(super) loaded_at: Instant,
}

pub(super) struct CachedSlackSidebarSnapshot {
    pub(super) snapshot: SlackSidebarSnapshot,
    pub(super) loaded_at: Instant,
    pub(super) refresh_started_at: Option<Instant>,
}

#[derive(Clone)]
pub(super) struct CachedSlackConversationDetail {
    pub(super) channel: Value,
    pub(super) loaded_at: Instant,
}

#[derive(Clone, Copy)]
pub(super) struct CachedSlackDndStatus {
    pub(super) notifications_paused: bool,
    pub(super) loaded_at: Instant,
}

struct SlackWorkspaceLoadProfile {
    conversations: Duration,
    sidebar: Duration,
    channel_payloads: Duration,
    users: Duration,
    team_self: Duration,
    total: Duration,
}

struct SlackConversationLoadProfile {
    core_payloads: Duration,
    users: Duration,
    total: Duration,
}

struct SlackConversationMutationProfile {
    mutation: Duration,
    core_payloads: Duration,
    cached_users: Duration,
    shape: Duration,
    total: Duration,
}

#[derive(Clone, Copy)]
enum SlackInitialHistoryPolicy {
    LoadLastReadWindow,
    NewestOnly,
}

#[derive(Clone)]
pub struct SlackAttachmentPreview {
    pub bytes: Vec<u8>,
    pub mimetype: String,
}

fn ensure_reaction_mutation_reflected(
    conversation: &SlackConversationSnapshot,
    target: &SlackReactionTarget,
    reaction_name: &SlackReactionName,
    mutation: SlackReactionMutation,
) -> Result<(), String> {
    let message = conversation
        .messages
        .iter()
        .flat_map(|message| std::iter::once(message).chain(message.replies.iter()))
        .find(|message| message.id == target.message_timestamp().as_str());
    ensure_message_reaction_mutation_reflected(message, target, reaction_name, mutation)
}

fn ensure_thread_reaction_mutation_reflected(
    thread: &SlackThreadSnapshot,
    target: &SlackReactionTarget,
    reaction_name: &SlackReactionName,
    mutation: SlackReactionMutation,
) -> Result<(), String> {
    let message = thread
        .parent
        .iter()
        .chain(thread.replies.iter())
        .find(|message| message.id == target.message_timestamp().as_str());
    ensure_message_reaction_mutation_reflected(message, target, reaction_name, mutation)
}

fn ensure_message_reaction_mutation_reflected(
    message: Option<&SlackMessage>,
    target: &SlackReactionTarget,
    reaction_name: &SlackReactionName,
    mutation: SlackReactionMutation,
) -> Result<(), String> {
    let message = message.ok_or_else(|| {
        format!(
            "Slack refresh did not include reacted message {}",
            target.message_timestamp().as_str()
        )
    })?;
    let active = message
        .reactions
        .iter()
        .find(|reaction| reaction.emoji == reaction_name.as_str())
        .is_some_and(|reaction| reaction.active);
    if active != mutation.active_after() {
        return Err(format!(
            "Slack conversation refresh did not confirm reaction {} was {}",
            reaction_name.as_str(),
            if mutation.active_after() {
                "added"
            } else {
                "removed"
            }
        ));
    }
    Ok(())
}

fn timed<T>(load: impl FnOnce() -> Result<T, String>) -> Result<(T, Duration), String> {
    let started_at = Instant::now();
    let value = load()?;
    Ok((value, started_at.elapsed()))
}

fn format_duration(duration: Duration) -> String {
    format!("{:.2}ms", duration.as_secs_f64() * 1000.0)
}

pub fn load_slack_live_workspace(
    team_id: &str,
    conversation_id: &str,
    team_domain: &str,
    web_session: SlackWebSessionCredentials,
) -> Result<SlackWorkspace, String> {
    SlackLiveWorkspaceLoader::new(team_id, team_domain, web_session)?
        .load_workspace(conversation_id)
}
