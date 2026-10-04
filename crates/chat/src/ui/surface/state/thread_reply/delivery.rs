mod file_share;
mod result;

use file_share::merge_sent_slack_thread_file_share;
use result::{parse_slack_thread_reply_timestamp, validate_slack_thread_reply_send_result};

use super::{
    execute_slack_thread_reply_send, merge_slack_thread_rows, prepare_slack_thread_reply_draft,
    refresh_slack_thread_list_rows, slack_thread_reply_list_index, Arc, Context,
    PreparedSlackThreadReplySendResult, PreparedSlackThreadSnapshot, SlackComposerDraftKey,
    SlackMessageRow, SlackMessageTimestamp, SlackThreadReplySendRequest, SurfaceState,
    WorkspaceApi,
};

struct SlackThreadReplySendContext {
    draft_key: SlackComposerDraftKey,
    conversation_id: String,
    parent_message_id: String,
    origin_is_current: bool,
    broadcast_supported: bool,
}

struct SlackThreadReplyReceiptTarget {
    panel_is_current: bool,
    all_threads_thread_key: Option<String>,
}

impl SurfaceState {
    pub(crate) fn can_send_slack_thread_reply(&self) -> bool {
        self.slack_workspace_api_capabilities.send_thread_reply
            && self.slack_thread_panel.as_ref().is_some_and(|panel| {
                !self.slack_thread_reply_mutation_is_blocked(&panel.reply_draft_key)
                    && self.slack_thread_panel_origin_is_current(panel)
            })
    }

    pub(crate) fn send_slack_thread_reply(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.send_thread_reply {
            return;
        }
        let Some(draft_key) = self.current_slack_thread_reply_draft_key() else {
            return;
        };
        if self.slack_thread_reply_mutation_is_blocked(&draft_key) {
            return;
        }
        if !self.slack_thread_reply_draft_is_sendable(&draft_key) {
            return;
        }
        let Some(request) = self.prepare_slack_thread_reply_send(cx) else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.show_slack_thread_reply_error(
                "Slack thread replies require a connected workspace".to_string(),
                cx,
            );
            return;
        };
        if !self.start_slack_thread_reply_send(request.draft_key.clone(), request.pending_send()) {
            return;
        }
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackThreadReplySendRequest)| {
                let result = execute_slack_thread_reply_send(
                    workspace_api.as_ref(),
                    &request.conversation_id,
                    &request.thread_timestamp,
                    &request.payload,
                    request.broadcast,
                );
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_thread_reply_send(request, result, cx);
            },
        );
    }

    fn prepare_slack_thread_reply_send(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<SlackThreadReplySendRequest> {
        let target = self.slack_thread_reply_send_context()?;
        let team_id = self.slack_thread_reply_team_id(cx)?;
        if !target.origin_is_current {
            self.show_slack_thread_reply_error(
                "The selected Slack thread is no longer active.".to_string(),
                cx,
            );
            return None;
        }
        let thread_timestamp = self.slack_thread_reply_timestamp(&target.parent_message_id, cx)?;
        let (draft_token, prepared_draft) = {
            let panel = self
                .slack_thread_panel
                .as_ref()
                .filter(|panel| panel.reply_draft_key == target.draft_key)?;
            let mut draft_state = panel.reply_draft.borrow_mut();
            let draft_token = draft_state.token;
            let prepared = prepare_slack_thread_reply_draft(
                &mut draft_state,
                &target.conversation_id,
                &thread_timestamp,
                target.broadcast_supported,
                self.slack_workspace_api_capabilities.share_files,
            );
            (draft_token, prepared)
        };
        let prepared_draft = match prepared_draft {
            Ok(prepared) => prepared,
            Err(error) => {
                self.show_slack_thread_reply_error(error, cx);
                return None;
            }
        };
        let send_generation = self.next_slack_thread_reply_send_generation();
        Some(SlackThreadReplySendRequest {
            send_generation,
            team_id,
            draft_key: target.draft_key,
            conversation_id: target.conversation_id,
            parent_message_id: target.parent_message_id,
            thread_timestamp,
            draft_token,
            draft_document_revision: prepared_draft.document_revision,
            payload: prepared_draft.payload,
            broadcast: prepared_draft.broadcast,
        })
    }

    fn slack_thread_reply_send_context(&self) -> Option<SlackThreadReplySendContext> {
        let panel = self.slack_thread_panel.as_ref()?;
        if self.slack_thread_reply_mutation_is_blocked(&panel.reply_draft_key) {
            return None;
        }
        Some(SlackThreadReplySendContext {
            draft_key: panel.reply_draft_key.clone(),
            conversation_id: panel.conversation_id.clone(),
            parent_message_id: panel.parent_message_id.clone(),
            origin_is_current: self.slack_thread_panel_origin_is_current(panel),
            broadcast_supported: panel.broadcast_label.is_some(),
        })
    }

    fn slack_thread_reply_team_id(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let team_id = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone());
        if team_id.is_none() {
            self.show_slack_thread_reply_error(
                "Slack thread replies require a connected workspace.".to_string(),
                cx,
            );
        }
        team_id
    }

    fn slack_thread_reply_timestamp(
        &mut self,
        parent_message_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<SlackMessageTimestamp> {
        match parse_slack_thread_reply_timestamp(parent_message_id) {
            Ok(timestamp) => Some(timestamp),
            Err(error) => {
                self.show_slack_thread_reply_error(error, cx);
                None
            }
        }
    }

    fn finish_slack_thread_reply_send(
        &mut self,
        request: SlackThreadReplySendRequest,
        result: Result<PreparedSlackThreadReplySendResult, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_thread_reply_send_is_pending(&request.draft_key, request.pending_send()) {
            return;
        }
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_slack_thread_reply_send(&request, error, cx);
                return;
            }
        };
        if let Err(error) = validate_slack_thread_reply_send_result(&request, &prepared) {
            self.fail_slack_thread_reply_send(&request, error, cx);
            return;
        }
        let receipt_target = SlackThreadReplyReceiptTarget {
            panel_is_current: self.slack_thread_reply_panel_is_current(&request.draft_key),
            all_threads_thread_key: self.slack_all_threads_surface_thread_key(&request.draft_key),
        };
        self.clear_submitted_slack_thread_reply_draft(
            &request.draft_key,
            request.pending_send(),
            cx,
        );
        self.finish_slack_thread_reply_pending_send(&request.draft_key, request.pending_send());
        let _ = self.restore_matching_slack_composer_schedule_recovery(cx);
        let all_threads_applied =
            self.apply_successful_slack_thread_reply_send(prepared, receipt_target, cx);
        self.reconcile_successful_slack_thread_reply(&request.draft_key, all_threads_applied, cx);
        cx.notify();
    }

    fn apply_successful_slack_thread_reply_send(
        &mut self,
        prepared: PreparedSlackThreadReplySendResult,
        target: SlackThreadReplyReceiptTarget,
        cx: &mut Context<Self>,
    ) -> bool {
        match prepared {
            PreparedSlackThreadReplySendResult::Message(prepared) => {
                if target.panel_is_current {
                    let panel = self
                        .slack_thread_panel
                        .as_mut()
                        .expect("current Slack thread disappeared while applying its sent reply");
                    panel.reply_error = None;
                    panel.reply_composer_focused = true;
                    self.apply_sent_slack_thread_reply(prepared.reply_row, cx);
                    false
                } else if let Some(thread_key) = target.all_threads_thread_key.as_deref() {
                    self.slack_all_threads_reply_errors.remove(thread_key);
                    self.apply_slack_all_threads_reply_receipt(thread_key, prepared.receipt);
                    self.slack_all_threads_list_state.remeasure();
                    true
                } else {
                    false
                }
            }
            PreparedSlackThreadReplySendResult::Share(prepared) => {
                if target.panel_is_current {
                    let panel = self.slack_thread_panel.as_mut().expect(
                        "current Slack thread disappeared while applying its sent file share",
                    );
                    panel.reply_error = None;
                    panel.reply_composer_focused = true;
                    self.apply_sent_slack_thread_file_share(prepared, cx);
                }
                false
            }
        }
    }

    fn fail_slack_thread_reply_send(
        &mut self,
        request: &SlackThreadReplySendRequest,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if !self.finish_slack_thread_reply_pending_send(&request.draft_key, request.pending_send())
        {
            return;
        }
        if self.slack_thread_reply_panel_is_current(&request.draft_key) {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("current Slack thread disappeared while applying its reply error");
            panel.reply_error = Some(error);
            panel.reply_composer_focused = true;
            panel.list_state.remeasure();
        } else if let Some(thread_key) =
            self.slack_all_threads_surface_thread_key(&request.draft_key)
        {
            self.slack_all_threads_reply_errors
                .insert(thread_key, error);
        }
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn show_slack_thread_reply_error(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("Slack thread panel disappeared while applying its reply error");
        panel.reply_error = Some(error);
        panel.reply_composer_focused = true;
        panel.list_state.remeasure();
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn apply_sent_slack_thread_reply(
        &mut self,
        reply_row: SlackMessageRow,
        cx: &mut Context<Self>,
    ) {
        let reply_id = reply_row.id.clone();
        let show_composer = self.slack_workspace_api_capabilities.send_thread_reply;
        let (reply_index, expected_reply_count) = {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("Slack thread disappeared before applying its sent reply");
            let old_rows = panel.reply_rows.clone();
            let reply_is_new = old_rows.iter().all(|row| row.id != reply_id);
            let merged_rows = merge_slack_thread_rows(
                &old_rows,
                std::slice::from_ref(&reply_row),
                panel.timezone,
            );
            panel.reply_rows = merged_rows.into();
            if reply_is_new {
                panel.expected_reply_count = panel.expected_reply_count.saturating_add(1);
            }
            panel.expected_reply_count = panel
                .expected_reply_count
                .max(u32::try_from(panel.reply_rows.len()).unwrap_or(u32::MAX));
            refresh_slack_thread_list_rows(panel, show_composer);
            panel.list_state.scroll_to_end();
            let reply_index = panel
                .reply_rows
                .iter()
                .position(|row| row.id == reply_id)
                .expect("sent Slack thread reply disappeared during row merge");
            let list_index = slack_thread_reply_list_index(panel, reply_index)
                .expect("sent Slack thread reply is missing from its list view model");
            (list_index, panel.expected_reply_count)
        };
        self.update_main_slack_thread_summary_after_reply(&reply_row, expected_reply_count);
        self.prefetch_slack_thread_images_for_range(reply_index..reply_index + 1, cx);
    }

    pub(in crate::ui::surface::state) fn apply_sent_slack_thread_file_share(
        &mut self,
        prepared: PreparedSlackThreadSnapshot,
        cx: &mut Context<Self>,
    ) {
        let show_composer = self.slack_workspace_api_capabilities.send_thread_reply;
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("current Slack thread disappeared before applying its shared files");
        let applied = merge_sent_slack_thread_file_share(panel, prepared, show_composer);
        if let Some(reply_row) = applied.reply_row.as_ref() {
            self.update_main_slack_thread_summary_after_reply(
                reply_row,
                applied.expected_reply_count,
            );
        }
        if let Some(reply_index) = applied.reply_index {
            self.prefetch_slack_thread_images_for_range(reply_index..reply_index + 1, cx);
        }
    }
}
