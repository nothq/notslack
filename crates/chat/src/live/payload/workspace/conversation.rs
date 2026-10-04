mod threads;

use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{
    load_slack_conversation_history_users_with_cache,
    load_slack_conversation_history_window_users_with_cache,
    load_slack_conversation_users_with_cache, load_slack_users_with_cache, ops, requests,
    slack_conversation_from_payloads, slack_conversation_history_next_cursor,
    slack_conversation_history_page, slack_last_read_timestamp, slack_thread_from_payloads,
    slack_thread_reply_receipt_from_payload, slack_workspace_from_payloads, timed,
    ConversationUserLoadInput, SlackChannelPayloads, SlackConversationHistoryCursor,
    SlackConversationHistoryPage, SlackConversationHistoryPagePayloads,
    SlackConversationLoadProfile, SlackConversationPayloads, SlackConversationSnapshot,
    SlackInitialHistoryPolicy, SlackLiveWorkspaceLoader, SlackMessageDraft, SlackMessageTimestamp,
    SlackSidebarSnapshot, SlackSidebarStageSnapshot, SlackStarMutation, SlackThreadPayloads,
    SlackThreadReplyPayloads, SlackThreadReplyReceipt, SlackThreadSnapshot, SlackWorkspace,
    SlackWorkspaceLoadProfile, SlackWorkspacePayloads, WorkspacePreloadTasks,
    WorkspaceUserLoadInput, SLACK_THREAD_PAGE_SIZE,
};

struct WorkspaceStage {
    conversations: Value,
    conversations_elapsed: Duration,
    sidebar_snapshot: SlackSidebarSnapshot,
    sidebar_elapsed: Duration,
    channel_payloads: SlackChannelPayloads,
    channel_payloads_elapsed: Duration,
}

type TimedWorkspaceUsers = (HashMap<String, Value>, Duration);
type WorkspacePreload = (Value, String, Value);

impl SlackLiveWorkspaceLoader {
    pub fn load_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<SlackConversationSnapshot, String> {
        self.load_conversation_with_anchor(conversation_id, None)
    }

    pub fn refresh_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<SlackConversationSnapshot, String> {
        self.load_conversation_snapshot(
            conversation_id,
            None,
            SlackInitialHistoryPolicy::NewestOnly,
        )
    }

    pub fn mark_conversation_read(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
    ) -> Result<(), String> {
        self.api.post(
            "conversations.mark",
            &[
                ("channel", conversation_id.to_string()),
                ("ts", message_timestamp.as_str().to_string()),
                ("_x_reason", "viewed".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        self.invalidate_realtime_sidebar_caches()?;
        Ok(())
    }

    pub fn mutate_channel_star(
        &self,
        conversation_id: &str,
        mutation: SlackStarMutation,
    ) -> Result<SlackSidebarStageSnapshot, String> {
        ops::mutate_channel_star(&self.api, conversation_id, mutation)?;
        self.invalidate_sidebar_snapshot()?;
        self.load_sidebar(conversation_id)
    }

    pub fn channel_permalink(&self, conversation_id: &str) -> Result<String, String> {
        self.sidebar_api.channel_permalink(conversation_id)
    }

    pub fn load_conversation_history_page(
        &self,
        conversation_id: &str,
        cursor: &SlackConversationHistoryCursor,
    ) -> Result<SlackConversationHistoryPage, String> {
        let history = self.load_conversation_history_page_payload(conversation_id, cursor)?;
        slack_conversation_history_next_cursor(&history, Some(cursor))?;
        let users = load_slack_conversation_history_users_with_cache(
            &self.api,
            &history,
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let (self_user_id, _) = self.load_self_user()?;
        let sidebar_snapshot = self.cached_sidebar_snapshot()?;
        slack_conversation_history_page(SlackConversationHistoryPagePayloads {
            team_id: &self.team_id,
            conversation_id,
            history: &history,
            request_cursor: cursor,
            users: &users,
            sidebar_snapshot: sidebar_snapshot.as_ref(),
            self_user_id: Some(&self_user_id),
            timezone: self.timezone.value,
        })
    }

    pub fn load_conversation_with_anchor(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<SlackConversationSnapshot, String> {
        self.load_conversation_snapshot(
            conversation_id,
            anchor_timestamp,
            if anchor_timestamp.is_none() {
                SlackInitialHistoryPolicy::LoadLastReadWindow
            } else {
                SlackInitialHistoryPolicy::NewestOnly
            },
        )
    }

    pub(super) fn load_conversation_snapshot(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
        initial_history_policy: SlackInitialHistoryPolicy,
    ) -> Result<SlackConversationSnapshot, String> {
        let total_started_at = Instant::now();
        let api_call_counts_before = self.workspace_load_api_counts()?;
        let ((channel_info, history_window), core_payloads_elapsed) = timed(|| {
            self.load_conversation_history_window(
                conversation_id,
                anchor_timestamp,
                initial_history_policy,
            )
        })?;
        let history_page_refs = history_window.source_pages.iter().collect::<Vec<_>>();
        let (users, users_elapsed) = timed(|| {
            load_slack_conversation_history_window_users_with_cache(
                &self.api,
                &channel_info,
                &history_page_refs,
                &self.user_cache,
                &self.user_fetch_lock,
            )
        })?;
        let (self_user_id, self_user) = self.load_self_user()?;
        let sidebar_snapshot = self.cached_sidebar_snapshot()?;
        let mut conversation = slack_conversation_from_payloads(SlackConversationPayloads {
            team_id: &self.team_id,
            conversation_id,
            self_user_id: Some(&self_user_id),
            self_user: Some(&self_user),
            self_timezone_id: self.timezone.id.as_deref(),
            timezone: self.timezone.value,
            channel_info: &channel_info,
            history: &history_window.history,
            history_next_cursor: history_window.next_cursor,
            users: &users,
            sidebar_snapshot: sidebar_snapshot.as_ref(),
            peer_notifications_paused: false,
        })?;
        conversation.last_read_boundary_loaded = history_window.last_read_boundary_loaded;
        self.prepare_conversation_tabs(&mut conversation.tabs);
        let profile = SlackConversationLoadProfile {
            core_payloads: core_payloads_elapsed,
            users: users_elapsed,
            total: total_started_at.elapsed(),
        };
        self.log_workspace_load_api_counts(conversation_id, api_call_counts_before)?;
        self.log_conversation_load_profile(conversation_id, profile);
        Ok(conversation)
    }

    fn load_conversation_history_window(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
        initial_history_policy: SlackInitialHistoryPolicy,
    ) -> Result<(Value, requests::SlackInitialHistoryWindow), String> {
        let core_payloads =
            self.load_conversation_core_payloads(conversation_id, anchor_timestamp)?;
        let last_read = if matches!(
            initial_history_policy,
            SlackInitialHistoryPolicy::LoadLastReadWindow
        ) {
            let channel = core_payloads.channel_info.get("channel").ok_or_else(|| {
                format!("Slack conversations.info response missing channel for {conversation_id}")
            })?;
            slack_last_read_timestamp(channel, conversation_id)?
        } else {
            None
        };
        let history_window = match last_read {
            Some(last_read) => self.load_initial_history_window(
                conversation_id,
                core_payloads.history,
                &last_read,
            )?,
            None => requests::SlackInitialHistoryWindow::newest(core_payloads.history)?,
        };
        Ok((core_payloads.channel_info, history_window))
    }

    pub fn load_workspace_with_anchor(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<SlackWorkspace, String> {
        let total_started_at = Instant::now();
        let api_call_counts_before = self.workspace_load_api_counts()?;
        let preload_tasks = self.spawn_workspace_preload_tasks();
        let stage = self.load_workspace_stage(conversation_id, anchor_timestamp)?;
        let history_next_cursor =
            slack_conversation_history_next_cursor(&stage.channel_payloads.history, None)?;
        let (users, users_elapsed) = self.load_workspace_users(conversation_id, &stage)?;
        let team_self_started_at = Instant::now();
        let (team_info, self_user_id, self_user) = join_workspace_preload(preload_tasks)?;
        let load_profile = SlackWorkspaceLoadProfile {
            conversations: stage.conversations_elapsed,
            sidebar: stage.sidebar_elapsed,
            channel_payloads: stage.channel_payloads_elapsed,
            users: users_elapsed,
            team_self: team_self_started_at.elapsed(),
            total: total_started_at.elapsed(),
        };
        self.log_workspace_load_api_counts(conversation_id, api_call_counts_before)?;
        self.log_workspace_load_profile(conversation_id, load_profile);
        let mut workspace = slack_workspace_from_payloads(SlackWorkspacePayloads {
            team_id: &self.team_id,
            conversation_id,
            team_info: &team_info,
            self_user_id: Some(self_user_id.as_str()),
            self_user: Some(&self_user),
            timezone: &self.timezone,
            channel_info: &stage.channel_payloads.channel_info,
            conversations: &stage.conversations,
            history: &stage.channel_payloads.history,
            history_next_cursor,
            users: &users,
            sidebar_snapshot: Some(&stage.sidebar_snapshot),
            peer_notifications_paused: stage.channel_payloads.peer_notifications_paused,
        })?;
        self.prepare_conversation_tabs(&mut workspace.tabs);
        Ok(workspace)
    }

    fn load_workspace_stage(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<WorkspaceStage, String> {
        let (conversations_result, sidebar_result, channel_payloads_result, users_result) =
            thread::scope(|scope| {
                let conversations =
                    scope.spawn(|| timed(|| self.load_workspace_conversations(conversation_id)));
                let sidebar = scope.spawn(|| timed(|| self.load_sidebar_snapshot()));
                let channel_payloads = scope.spawn(|| {
                    timed(|| self.load_channel_payloads(conversation_id, anchor_timestamp))
                });
                let users = scope.spawn(|| self.ensure_user_directory_cache());
                (
                    conversations
                        .join()
                        .map_err(|_| "Slack conversations request thread panicked".to_string()),
                    sidebar
                        .join()
                        .map_err(|_| "Slack sidebar request thread panicked".to_string()),
                    channel_payloads
                        .join()
                        .map_err(|_| "Slack channel payload request thread panicked".to_string()),
                    users
                        .join()
                        .map_err(|_| "Slack users.list request thread panicked".to_string()),
                )
            });
        users_result??;
        let (conversations, conversations_elapsed) = conversations_result??;
        let (sidebar_snapshot, sidebar_elapsed) = sidebar_result??;
        let (channel_payloads, channel_payloads_elapsed) = channel_payloads_result??;
        Ok(WorkspaceStage {
            conversations,
            conversations_elapsed,
            sidebar_snapshot,
            sidebar_elapsed,
            channel_payloads,
            channel_payloads_elapsed,
        })
    }

    fn load_workspace_users(
        &self,
        conversation_id: &str,
        stage: &WorkspaceStage,
    ) -> Result<TimedWorkspaceUsers, String> {
        timed(|| {
            load_slack_users_with_cache(
                &self.api,
                WorkspaceUserLoadInput {
                    active_conversation_id: conversation_id,
                    channel_info: &stage.channel_payloads.channel_info,
                    conversations: &stage.conversations,
                    history: &stage.channel_payloads.history,
                    sidebar_snapshot: Some(&stage.sidebar_snapshot),
                    now: time::OffsetDateTime::now_utc(),
                },
                &self.user_cache,
                &self.user_fetch_lock,
                || self.ensure_user_directory_cache(),
            )
        })
    }
}

fn join_workspace_preload(tasks: WorkspacePreloadTasks) -> Result<WorkspacePreload, String> {
    let team_info = tasks
        .team_info
        .join()
        .map_err(|_| "Slack team info request thread panicked".to_string())??;
    let (self_user_id, self_user) = tasks
        .self_user
        .join()
        .map_err(|_| "Slack self user request thread panicked".to_string())??;
    Ok((team_info, self_user_id, self_user))
}
