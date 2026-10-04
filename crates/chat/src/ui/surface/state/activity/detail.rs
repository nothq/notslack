mod installation;

use super::super::main_composer::{
    slack_activity_conversation_composer_context,
    slack_activity_current_conversation_composer_context,
};
use super::{
    collect_slack_activity_detail_row_urls, find_activity_anchor_index, load_slack_activity_detail,
    next_slack_activity_generation, px, slack_activity_thread_detail_rows, Arc, Context,
    ListOffset, PreparedSlackActivityDetail, PreparedSlackConversationSnapshot,
    PreparedSlackThreadSnapshot, SharedString, SlackActivityDetailInstallation,
    SlackActivityDetailRequest, SlackActivityDetailState, SlackActivityDetailTarget,
    SlackActivityRow, SlackMessageRow, SlackMessageTimestamp, SlackRailView, SurfaceState,
    SLACK_ACTIVITY_DETAIL_IMAGE_VISIBLE_OVERDRAW, SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_slack_activity_conversation_refresh(
        &mut self,
        prepared: PreparedSlackConversationSnapshot,
        force_scroll_to_end: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let SlackActivityDetailState::Loaded { target, .. } = &self.slack_activity_detail else {
            return Ok(());
        };
        let Some(current) = target.composer() else {
            return Ok(());
        };
        if current.target.team_id != prepared.snapshot.team_id
            || current.target.conversation_id != prepared.snapshot.conversation_id
        {
            return Ok(());
        }
        let crate::ui::surface::SlackSendDraftSource::Activity { item_key, .. } = &current.source
        else {
            return Err("Slack Activity conversation lost its exact composer source.".to_string());
        };
        let composer = slack_activity_conversation_composer_context(
            &prepared.snapshot,
            current.target.self_user_id.clone(),
            item_key.clone(),
        );
        let was_following_end = self.slack_activity_detail_list_state.is_following_tail();
        let row_count = prepared.message_rows.len();
        self.slack_remote_images.extend(prepared.remote_images);
        let (key, channel_label, message_timestamp) =
            loaded_slack_activity_detail_identity(&self.slack_activity_detail);
        self.slack_activity_detail = SlackActivityDetailState::Loaded {
            key,
            channel_label,
            message_timestamp,
            target: SlackActivityDetailTarget::Conversation {
                composer: Box::new(composer.clone()),
            },
            rows: prepared.message_rows,
        };
        self.activate_slack_activity_main_composer(composer, cx);
        self.rebuild_slack_activity_local_delivery_rows();
        self.slack_activity_detail_list_state
            .reset(row_count.saturating_add(self.slack_activity_local_delivery_rows.len()));
        if force_scroll_to_end || was_following_end {
            self.slack_activity_detail_list_state.scroll_to_end();
        }
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
        Ok(())
    }

    pub(in crate::ui::surface::state) fn load_slack_activity_detail(
        &mut self,
        row: SlackActivityRow,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.slack_activity_detail,
            SlackActivityDetailState::Loading { .. }
        ) {
            return;
        }
        let generation = self.begin_slack_activity_detail_loading(&row, cx);
        if self.finish_cached_slack_activity_detail(&row, cx) {
            return;
        }
        if !self.slack_workspace_api_capabilities.load_conversation {
            self.fail_slack_activity_detail(
                row,
                "Conversation details are unavailable for this workspace.".into(),
                cx,
            );
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.fail_slack_activity_detail(row, "missing Slack workspace api".into(), cx);
            return;
        };
        let Some((team_id, self_user_id)) = self.slack_workspace().and_then(|workspace| {
            Some((
                self.slack_activity_team_id.clone()?,
                workspace.self_user_id.clone()?,
            ))
        }) else {
            self.fail_slack_activity_detail(
                row,
                "Slack Activity authenticated workspace identity is unavailable.".into(),
                cx,
            );
            return;
        };
        let request = SlackActivityDetailRequest {
            generation,
            team_id,
            self_user_id,
            key: row.key,
            channel_id: row.channel_id,
            channel_label: row.channel_label,
            message_timestamp: row.message_timestamp,
            thread_timestamp: row.thread_timestamp,
        };
        self.spawn_background_task(
            request,
            cx,
            move |request| load_slack_activity_detail(workspace_api, request),
            move |this, (request, result), cx| {
                this.finish_slack_activity_detail(request, result, cx);
            },
        );
    }

    fn finish_cached_slack_activity_detail(
        &mut self,
        row: &SlackActivityRow,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_conversation_id() != Some(row.channel_id.as_ref()) {
            return false;
        }
        if let Some(thread_timestamp) = row.thread_timestamp.as_ref() {
            let Some(rows) = slack_activity_thread_detail_rows(
                &self.slack_message_rows,
                thread_timestamp,
                &row.message_timestamp,
            ) else {
                return false;
            };
            self.finish_current_slack_activity_thread_detail(row.clone(), rows, cx);
            return true;
        }
        if find_activity_anchor_index(&self.slack_message_rows, &row.message_timestamp).is_none() {
            return false;
        }
        self.finish_current_slack_activity_detail(row.clone(), cx);
        true
    }

    fn begin_slack_activity_detail_loading(
        &mut self,
        row: &SlackActivityRow,
        cx: &mut Context<Self>,
    ) -> u64 {
        self.slack_activity_detail_generation = next_slack_activity_generation(
            self.slack_activity_detail_generation,
            "Slack Activity detail generation overflowed",
        );
        self.park_slack_main_composer(cx);
        self.slack_activity_detail = SlackActivityDetailState::Loading {
            key: row.key.clone(),
            channel_label: row.channel_label.clone(),
        };
        self.rebuild_slack_activity_local_delivery_rows();
        self.slack_activity_detail_list_state.reset(0);
        cx.notify();
        self.slack_activity_detail_generation
    }

    pub(in crate::ui::surface::state) fn finish_current_slack_activity_detail(
        &mut self,
        row: SlackActivityRow,
        cx: &mut Context<Self>,
    ) {
        let Some(anchor_index) =
            find_activity_anchor_index(&self.slack_message_rows, &row.message_timestamp)
        else {
            self.fail_slack_activity_detail(
                row,
                "This activity message is outside the loaded conversation page.".into(),
                cx,
            );
            return;
        };
        let channel_label = row
            .channel_label
            .clone()
            .or_else(|| {
                self.slack_workspace()
                    .map(|workspace| workspace.channel_name.clone().into())
            })
            .expect("loaded Slack conversation must provide a channel label");
        let rows = self.slack_message_rows.clone();
        let workspace = self
            .slack_workspace()
            .cloned()
            .expect("loaded Slack conversation must retain its workspace");
        let self_user_id = workspace
            .self_user_id
            .clone()
            .expect("loaded Slack Activity conversation requires an authenticated user");
        let target = SlackActivityDetailTarget::Conversation {
            composer: Box::new(slack_activity_current_conversation_composer_context(
                &workspace,
                self_user_id,
                row.key.clone(),
            )),
        };
        self.install_slack_activity_detail(
            SlackActivityDetailInstallation {
                key: row.key,
                channel_label,
                message_timestamp: row.message_timestamp,
                target,
                rows,
                anchor_index,
            },
            cx,
        );
    }

    fn finish_current_slack_activity_thread_detail(
        &mut self,
        row: SlackActivityRow,
        rows: Arc<[SlackMessageRow]>,
        cx: &mut Context<Self>,
    ) {
        let channel_label = row
            .channel_label
            .clone()
            .or_else(|| {
                self.slack_workspace()
                    .map(|workspace| workspace.channel_name.clone().into())
            })
            .expect("loaded Slack conversation must provide a channel label");
        let (team_id, self_user_id) = self
            .slack_workspace()
            .map(|workspace| (workspace.team_id.clone(), workspace.self_user_id.clone()))
            .expect("loaded Slack Activity thread requires its workspace");
        let self_user_id = self_user_id
            .clone()
            .expect("loaded Slack Activity thread requires an authenticated user");
        let thread_timestamp = row
            .thread_timestamp
            .clone()
            .expect("loaded Slack Activity thread requires its parent timestamp");
        let item_key = row.key.clone();
        let conversation_id = row.channel_id.to_string();
        self.install_slack_activity_detail(
            SlackActivityDetailInstallation {
                key: row.key,
                channel_label,
                message_timestamp: row.message_timestamp,
                target: SlackActivityDetailTarget::Thread {
                    team_id,
                    self_user_id,
                    conversation_id,
                    item_key,
                    thread_timestamp,
                },
                rows,
                anchor_index: 1,
            },
            cx,
        );
    }

    fn finish_slack_activity_detail(
        &mut self,
        request: SlackActivityDetailRequest,
        result: Result<PreparedSlackActivityDetail, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_detail_generation != request.generation
            || self.slack_active_rail_view != SlackRailView::Activity
            || self.slack_activity_team_id.as_deref() != Some(request.team_id.as_str())
            || self.slack_activity_detail.key() != Some(request.key.as_ref())
        {
            return;
        }
        if self.slack_workspace().is_none_or(|workspace| {
            workspace.team_id != request.team_id
                || workspace.self_user_id.as_deref() != Some(request.self_user_id.as_str())
        }) {
            self.reset_slack_activity_context(cx);
            self.activate_slack_activity(cx);
            return;
        }
        if self.slack_activity_selected_key.as_deref() != Some(request.key.as_ref()) {
            self.slack_activity_detail = SlackActivityDetailState::Empty;
            self.activate_selected_slack_activity(cx);
            return;
        }
        match result {
            Ok(PreparedSlackActivityDetail::Conversation(prepared)) => {
                self.apply_loaded_slack_activity_conversation_detail(request, *prepared, cx)
            }
            Ok(PreparedSlackActivityDetail::Thread(prepared)) => {
                self.apply_loaded_slack_activity_thread_detail(request, *prepared, cx)
            }
            Err(message) => {
                self.park_slack_main_composer(cx);
                self.slack_activity_detail = SlackActivityDetailState::Error {
                    key: request.key,
                    channel_label: request.channel_label,
                    message: message.into(),
                };
                self.rebuild_slack_activity_local_delivery_rows();
                cx.notify();
            }
        }
    }
}

fn loaded_slack_activity_detail_identity(
    detail: &SlackActivityDetailState,
) -> (SharedString, SharedString, SlackMessageTimestamp) {
    match detail {
        SlackActivityDetailState::Loaded {
            key,
            channel_label,
            message_timestamp,
            ..
        } => (
            key.clone(),
            channel_label.clone(),
            message_timestamp.clone(),
        ),
        SlackActivityDetailState::Empty
        | SlackActivityDetailState::Loading { .. }
        | SlackActivityDetailState::Error { .. } => {
            unreachable!("validated Slack Activity detail must remain loaded")
        }
    }
}
