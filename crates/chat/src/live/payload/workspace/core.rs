use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use serde_json::{json, Value};

use super::{
    format_duration, load_slack_activity_users_with_cache, load_slack_sidebar_users_with_cache,
    members, slack_shell_from_payloads, slack_sidebar_from_payloads, ActivityUserLoadInput,
    SidebarUserLoadInput, SlackActivityCursor, SlackActivitySnapshot, SlackApiClient,
    SlackConversationMembersCursor, SlackConversationMembersSnapshot, SlackDmInboxSnapshot,
    SlackFilesRequest, SlackFilesSnapshot, SlackInternalSidebarClient, SlackLiveWorkspaceLoader,
    SlackShellSnapshot, SlackSidebarStageSnapshot, SlackTeamDomain, SlackTimezone,
    SlackUserPayloadCache, SlackWebSessionCredentials, SlackWorkspace, SLACK_LOAD_PROFILE_ENV,
};
use crate::live::{
    api::messages::SlackMessagesListPurpose, internal_sidebar::SlackActivitySnapshotHydrationInput,
};

impl SlackLiveWorkspaceLoader {
    pub fn new(
        team_id: &str,
        team_domain: impl Into<String>,
        web_session: SlackWebSessionCredentials,
    ) -> Result<Self, String> {
        let team_domain = SlackTeamDomain::parse(team_domain.into())?;
        Self::new_with_team_domain(team_id, team_domain, web_session)
    }

    pub(crate) fn new_with_team_domain(
        team_id: &str,
        team_domain: SlackTeamDomain,
        web_session: SlackWebSessionCredentials,
    ) -> Result<Self, String> {
        let started_at = Instant::now();
        let timezone = SlackTimezone::from_native_runtime()?;
        let timezone_loaded_at = Instant::now();
        let api = SlackApiClient::desktop(&web_session)?;
        let api_built_at = Instant::now();
        let sidebar_api = SlackInternalSidebarClient::new(team_id, team_domain, web_session)?;
        let sidebar_api_built_at = Instant::now();
        let user_cache = SlackUserPayloadCache::for_team(team_id);
        let user_cache_handle_built_at = Instant::now();
        if std::env::var_os(SLACK_LOAD_PROFILE_ENV).is_some() {
            eprintln!(
                "[notslack-slack-loader-construction-profile] total={} timezone={} api_client={} sidebar_client={} user_cache_handle={}",
                format_duration(user_cache_handle_built_at.duration_since(started_at)),
                format_duration(timezone_loaded_at.duration_since(started_at)),
                format_duration(api_built_at.duration_since(timezone_loaded_at)),
                format_duration(sidebar_api_built_at.duration_since(api_built_at)),
                format_duration(user_cache_handle_built_at.duration_since(sidebar_api_built_at)),
            );
        }
        Ok(Self {
            api,
            sidebar_api,
            team_id: team_id.to_string(),
            timezone,
            auth_test_cache: Arc::new(Mutex::new(None)),
            auth_test_fetch_lock: Arc::new(Mutex::new(())),
            team_info_cache: Arc::new(Mutex::new(None)),
            conversations_cache: Arc::new(Mutex::new(None)),
            sidebar_snapshot_cache: Arc::new(Mutex::new(None)),
            user_cache: Arc::new(user_cache),
            user_fetch_lock: Arc::new(Mutex::new(())),
            bot_cache: Arc::new(Mutex::new(HashMap::new())),
            bot_fetch_lock: Arc::new(Mutex::new(())),
            quick_message_cache: Arc::new(Mutex::new(VecDeque::new())),
            quick_message_fetch_locks: Arc::new(Mutex::new(HashMap::new())),
            conversation_detail_cache: Arc::new(Mutex::new(HashMap::new())),
            conversation_detail_fetch_locks: Arc::new(Mutex::new(HashMap::new())),
            connected_organization_cache: Arc::new(Mutex::new(HashMap::new())),
            user_preferences_cache: Arc::new(Mutex::new(None)),
            user_preferences_fetch_lock: Arc::new(Mutex::new(())),
            user_preferences_mutation_lock: Arc::new(Mutex::new(())),
            reaction_catalog_cache: Arc::new(Mutex::new(None)),
            reaction_catalog_fetch_lock: Arc::new(Mutex::new(())),
            dnd_status_cache: Arc::new(Mutex::new(HashMap::new())),
            dnd_status_fetch_locks: Arc::new(Mutex::new(HashMap::new())),
            attachment_preview_cache: Arc::new(Mutex::new(HashMap::new())),
            file_metadata_cache: Arc::new(Mutex::new(HashMap::new())),
            file_metadata_fetch_locks: Arc::new(Mutex::new(HashMap::new())),
            file_staging_ledger: super::file_staging::SlackFileStagingLedger::default(),
            canvas_tab_metadata_cache: Arc::new(Mutex::new(HashMap::new())),
            canvas_tab_hydration_started: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn team_id(&self) -> &str {
        &self.team_id
    }

    pub fn api_client(&self) -> SlackApiClient {
        self.api.clone()
    }

    pub fn load_workspace(&self, conversation_id: &str) -> Result<SlackWorkspace, String> {
        self.load_workspace_with_anchor(conversation_id, None)
    }

    pub fn load_conversation_members(
        &self,
        conversation_id: &str,
        cursor: Option<&SlackConversationMembersCursor>,
    ) -> Result<SlackConversationMembersSnapshot, String> {
        members::load_conversation_members(self, conversation_id, cursor)
    }

    pub fn load_shell(&self) -> Result<SlackShellSnapshot, String> {
        let tasks = self.spawn_workspace_preload_tasks();
        let team_info = tasks
            .team_info
            .join()
            .map_err(|_| "Slack team info request thread panicked".to_string())??;
        let (self_user_id, self_user) = tasks
            .self_user
            .join()
            .map_err(|_| "Slack self user request thread panicked".to_string())??;
        Ok(slack_shell_from_payloads(
            &self.team_id,
            &team_info,
            Some(self_user_id.as_str()),
            Some(&self_user),
            &self.timezone,
        ))
    }

    pub fn load_sidebar(&self, conversation_id: &str) -> Result<SlackSidebarStageSnapshot, String> {
        let (conversations_result, sidebar_result) = thread::scope(|scope| {
            let conversations = scope.spawn(|| self.load_workspace_conversations(conversation_id));
            let sidebar = scope.spawn(|| self.load_sidebar_snapshot());
            (
                conversations
                    .join()
                    .map_err(|_| "Slack conversations request thread panicked".to_string()),
                sidebar
                    .join()
                    .map_err(|_| "Slack sidebar request thread panicked".to_string()),
            )
        });
        let conversations = conversations_result??;
        let sidebar_snapshot = sidebar_result??;
        let users = load_slack_sidebar_users_with_cache(
            &self.api,
            SidebarUserLoadInput {
                active_conversation_id: conversation_id,
                conversations: &conversations,
                sidebar_snapshot: &sidebar_snapshot,
            },
            &self.user_cache,
            &self.user_fetch_lock,
        )?;
        Ok(slack_sidebar_from_payloads(
            &self.team_id,
            conversation_id,
            &conversations,
            &users,
            Some(&sidebar_snapshot),
        ))
    }

    pub fn load_dm_inbox(&self, cursor: Option<&str>) -> Result<SlackDmInboxSnapshot, String> {
        let auth_test = self.load_auth_test()?;
        let self_user_id = auth_test
            .get("user_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|user_id| !user_id.is_empty())
            .ok_or_else(|| "Slack auth.test did not return user_id".to_string())?;
        let load = self.sidebar_api.load_dm_inbox(
            &self.team_id,
            self_user_id,
            cursor,
            &self.user_cache,
        )?;
        self.user_cache.extend(load.users)?;
        Ok(load.snapshot)
    }

    pub fn load_activity(
        &self,
        cursor: Option<&SlackActivityCursor>,
    ) -> Result<SlackActivitySnapshot, String> {
        let page = self.sidebar_api.load_activity_feed(cursor)?;
        let actor_user_ids = page.reaction_actor_user_ids();
        let hydration = self.api.load_messages_list(
            page.message_references(),
            SlackMessagesListPurpose::Activity,
        )?;
        let history = json!({
            "messages": hydration.values().cloned().collect::<Vec<_>>(),
        });
        let users = load_slack_activity_users_with_cache(
            &self.api,
            ActivityUserLoadInput {
                history: &history,
                user_ids: actor_user_ids,
            },
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let self_user_id = self.load_self_user_id()?;
        let sidebar_snapshot = self.load_sidebar_snapshot()?;
        page.into_snapshot(SlackActivitySnapshotHydrationInput::new(
            hydration,
            &users,
            Some(&sidebar_snapshot),
            &self_user_id,
            self.timezone.value,
        ))
    }

    pub fn mark_activity_item_read(
        &self,
        target: &crate::model::SlackActivityReadTarget,
    ) -> Result<(), String> {
        self.sidebar_api.mark_activity_item_read(target)?;
        self.invalidate_sidebar_snapshot()
    }

    pub fn mark_activity_item_unread(
        &self,
        target: &crate::model::SlackActivityReadTarget,
    ) -> Result<(), String> {
        self.sidebar_api.mark_activity_item_unread(target)?;
        self.invalidate_sidebar_snapshot()
    }

    pub fn archive_activity_item(
        &self,
        target: &crate::model::SlackActivityArchiveTarget,
        reason: &str,
    ) -> Result<(), String> {
        self.sidebar_api.archive_activity_item(target, reason)?;
        self.invalidate_sidebar_snapshot()
    }

    pub fn unarchive_activity_item(
        &self,
        target: &crate::model::SlackActivityArchiveTarget,
        reason: &str,
    ) -> Result<(), String> {
        self.sidebar_api.unarchive_activity_item(target, reason)?;
        self.invalidate_sidebar_snapshot()
    }

    pub fn load_files(&self, request: SlackFilesRequest) -> Result<SlackFilesSnapshot, String> {
        if request.team_id != self.team_id {
            return Err(format!(
                "Slack Files request targeted team {} from runtime team {}",
                request.team_id, self.team_id
            ));
        }
        self.sidebar_api.load_files(&request)
    }
}
