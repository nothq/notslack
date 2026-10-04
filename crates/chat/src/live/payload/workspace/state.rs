use std::{thread, time::Instant};

use super::{
    format_duration, load_cached_slack_conversation_users, slack_conversation_from_payloads,
    slack_conversation_history_next_cursor, timed, CachedSlackSidebarSnapshot,
    ConversationUserLoadInput, SlackApiCallCountSnapshot, SlackConversationLoadProfile,
    SlackConversationMutationProfile, SlackConversationPayloads, SlackConversationSnapshot,
    SlackLiveWorkspaceLoader, SlackSidebarSnapshot, SlackWorkspaceLoadProfile,
    SLACK_LOAD_PROFILE_ENV, SLACK_SIDEBAR_REFRESH_INTERVAL, SLACK_SIDEBAR_REFRESH_RETRY_INTERVAL,
};

impl SlackLiveWorkspaceLoader {
    pub(super) fn workspace_load_api_counts(&self) -> Result<SlackApiCallCountSnapshot, String> {
        if std::env::var_os("NOTSLACK_SLACK_API_CALL_PROFILE").is_none() {
            return Ok(None);
        }
        self.api.call_counts_snapshot().map(Some)
    }

    pub(super) fn load_sidebar_snapshot(&self) -> Result<SlackSidebarSnapshot, String> {
        if let Some(snapshot) = self.cached_sidebar_snapshot()? {
            return Ok(snapshot);
        }
        let snapshot = self.sidebar_api.load_sidebar_snapshot(&self.user_cache)?;
        self.store_sidebar_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }

    pub(super) fn ensure_user_directory_cache(&self) -> Result<(), String> {
        let users = self.sidebar_api.load_user_directory()?;
        self.user_cache.extend(users)?;
        Ok(())
    }

    pub(super) fn cached_sidebar_snapshot(&self) -> Result<Option<SlackSidebarSnapshot>, String> {
        let mut should_refresh = false;
        let snapshot = {
            let mut cache = self
                .sidebar_snapshot_cache
                .lock()
                .map_err(|_| "slack sidebar snapshot cache mutex poisoned".to_string())?;
            let Some(cached) = cache.as_mut() else {
                return Ok(None);
            };
            if cached.loaded_at.elapsed() >= SLACK_SIDEBAR_REFRESH_INTERVAL
                && cached.refresh_started_at.is_none_or(|started_at| {
                    started_at.elapsed() >= SLACK_SIDEBAR_REFRESH_RETRY_INTERVAL
                })
            {
                cached.refresh_started_at = Some(Instant::now());
                should_refresh = true;
            }
            cached.snapshot.clone()
        };
        if should_refresh {
            self.refresh_sidebar_snapshot_in_background();
        }
        Ok(Some(snapshot))
    }

    fn store_sidebar_snapshot(&self, snapshot: SlackSidebarSnapshot) -> Result<(), String> {
        *self
            .sidebar_snapshot_cache
            .lock()
            .map_err(|_| "slack sidebar snapshot cache mutex poisoned".to_string())? =
            Some(CachedSlackSidebarSnapshot {
                snapshot,
                loaded_at: Instant::now(),
                refresh_started_at: None,
            });
        Ok(())
    }

    pub(super) fn invalidate_sidebar_snapshot(&self) -> Result<(), String> {
        *self
            .sidebar_snapshot_cache
            .lock()
            .map_err(|_| "slack sidebar snapshot cache mutex poisoned".to_string())? = None;
        Ok(())
    }

    pub(crate) fn invalidate_realtime_sidebar_caches(&self) -> Result<(), String> {
        self.invalidate_sidebar_snapshot()?;
        self.sidebar_api.invalidate_dm_counts()?;
        *self
            .conversations_cache
            .lock()
            .map_err(|_| "slack conversations cache mutex poisoned".to_string())? = None;
        Ok(())
    }

    fn refresh_sidebar_snapshot_in_background(&self) {
        let loader = self.clone();
        thread::spawn(
            move || match loader.sidebar_api.load_sidebar_snapshot(&loader.user_cache) {
                Ok(snapshot) => {
                    if let Err(error) = loader.store_sidebar_snapshot(snapshot) {
                        eprintln!("[notslack-slack-sidebar-refresh] cache_update_error={error}");
                    }
                }
                Err(error) => {
                    eprintln!("[notslack-slack-sidebar-refresh] error={error}");
                }
            },
        );
    }

    pub(super) fn log_workspace_load_api_counts(
        &self,
        conversation_id: &str,
        before: SlackApiCallCountSnapshot,
    ) -> Result<(), String> {
        let Some(before) = before else {
            return Ok(());
        };
        let after = self.api.call_counts_snapshot()?;
        let mut call_counts = after
            .into_iter()
            .filter_map(|(method, count)| {
                let previous_count = before.get(&method).copied().unwrap_or_default();
                (count > previous_count).then_some((method, count - previous_count))
            })
            .collect::<Vec<_>>();
        call_counts.sort_by(|left, right| left.0.cmp(&right.0));
        let summary = call_counts
            .into_iter()
            .map(|(method, count)| format!("{method}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("[notslack-slack-api-calls] conversation={conversation_id} {summary}");
        Ok(())
    }

    pub(super) fn log_workspace_load_profile(
        &self,
        conversation_id: &str,
        profile: SlackWorkspaceLoadProfile,
    ) {
        if std::env::var_os(SLACK_LOAD_PROFILE_ENV).is_none() {
            return;
        }
        eprintln!(
            "[notslack-slack-workspace-load-profile] conversation={} total={} conversations={} sidebar={} channel_payloads={} users={} team_self={}",
            conversation_id,
            format_duration(profile.total),
            format_duration(profile.conversations),
            format_duration(profile.sidebar),
            format_duration(profile.channel_payloads),
            format_duration(profile.users),
            format_duration(profile.team_self),
        );
    }

    pub(super) fn log_conversation_load_profile(
        &self,
        conversation_id: &str,
        profile: SlackConversationLoadProfile,
    ) {
        if std::env::var_os(SLACK_LOAD_PROFILE_ENV).is_none() {
            return;
        }
        eprintln!(
            "[notslack-slack-conversation-load-profile] conversation={} total={} core_payloads={} users={}",
            conversation_id,
            format_duration(profile.total),
            format_duration(profile.core_payloads),
            format_duration(profile.users),
        );
    }

    pub(super) fn mutate_and_refresh_conversation(
        &self,
        conversation_id: &str,
        operation: &'static str,
        mutation: impl FnOnce() -> Result<(), String>,
    ) -> Result<SlackConversationSnapshot, String> {
        let total_started_at = Instant::now();
        let api_call_counts_before = self.workspace_load_api_counts()?;
        let ((), mutation_elapsed) = timed(mutation)?;
        let (core_payloads, core_payloads_elapsed) =
            timed(|| self.load_conversation_core_payloads(conversation_id, None))?;
        let history_next_cursor =
            slack_conversation_history_next_cursor(&core_payloads.history, None)?;
        let (users, cached_users_elapsed) = timed(|| {
            load_cached_slack_conversation_users(
                ConversationUserLoadInput {
                    channel_info: &core_payloads.channel_info,
                    history: &core_payloads.history,
                },
                &self.user_cache,
            )
        })?;
        let (self_user_id, self_user) = self.load_self_user()?;
        let (mut conversation, shape_elapsed) = timed(|| {
            slack_conversation_from_payloads(SlackConversationPayloads {
                team_id: &self.team_id,
                conversation_id,
                self_user_id: Some(&self_user_id),
                self_user: Some(&self_user),
                self_timezone_id: self.timezone.id.as_deref(),
                timezone: self.timezone.value,
                channel_info: &core_payloads.channel_info,
                history: &core_payloads.history,
                history_next_cursor,
                users: &users,
                sidebar_snapshot: None,
                peer_notifications_paused: false,
            })
        })?;
        self.prepare_conversation_tabs(&mut conversation.tabs);
        let profile = SlackConversationMutationProfile {
            mutation: mutation_elapsed,
            core_payloads: core_payloads_elapsed,
            cached_users: cached_users_elapsed,
            shape: shape_elapsed,
            total: total_started_at.elapsed(),
        };
        self.log_workspace_load_api_counts(conversation_id, api_call_counts_before)?;
        self.log_conversation_mutation_profile(conversation_id, operation, profile);
        Ok(conversation)
    }

    fn log_conversation_mutation_profile(
        &self,
        conversation_id: &str,
        operation: &str,
        profile: SlackConversationMutationProfile,
    ) {
        if std::env::var_os(SLACK_LOAD_PROFILE_ENV).is_none() {
            return;
        }
        eprintln!(
            "[notslack-slack-conversation-mutation-profile] operation={} conversation={} total={} mutation={} core_payloads={} cached_users={} shape={}",
            operation,
            conversation_id,
            format_duration(profile.total),
            format_duration(profile.mutation),
            format_duration(profile.core_payloads),
            format_duration(profile.cached_users),
            format_duration(profile.shape),
        );
    }
}
