use super::super::thread_reply::{
    execute_slack_thread_reply_send, prepare_slack_thread_reply_draft,
    PreparedSlackThreadReplyDraft, PreparedSlackThreadReplySendResult,
};
use super::{
    prepare_slack_all_threads_snapshot, Arc, Context, SlackAllThreadsReplyRequest, SlackMainRoute,
    SlackMessageTimestamp, SurfaceState, WorkspaceApi,
};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraftKey, SlackReplyComposerTarget,
};

struct SlackAllThreadsReplyWork {
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackAllThreadsReplyRequest,
}

struct SlackAllThreadsReplyDraftInput<'a> {
    thread_key: &'a str,
    draft_key: &'a SlackComposerDraftKey,
    conversation_id: &'a str,
    thread_timestamp: &'a SlackMessageTimestamp,
    broadcast_supported: bool,
}

impl SurfaceState {
    pub(crate) fn send_slack_all_threads_reply(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        let SlackReplyComposerTarget::AllThreads { draft_key, .. } = target else {
            return;
        };
        if !self.can_mutate_slack_reply_composer(target)
            || self.slack_thread_reply_mutation_is_blocked(draft_key)
        {
            return;
        }
        if !self.slack_thread_reply_draft_is_sendable(draft_key) {
            return;
        }
        let Some(work) = self.prepare_slack_all_threads_reply(target, cx) else {
            return;
        };
        self.spawn_background_task(
            (work.workspace_api, work.request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackAllThreadsReplyRequest)| {
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
                this.finish_slack_all_threads_reply(request, result, cx);
            },
        );
    }

    fn prepare_slack_all_threads_reply(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) -> Option<SlackAllThreadsReplyWork> {
        let SlackReplyComposerTarget::AllThreads {
            thread_key,
            draft_key,
            broadcast_supported,
        } = target
        else {
            return None;
        };
        if !self.slack_all_threads_reply_target_is_available(target, draft_key) {
            return None;
        }
        let SlackComposerDestination::Thread {
            conversation_id,
            thread_timestamp,
        } = &draft_key.destination
        else {
            return None;
        };
        let workspace_api = self.slack_all_threads_reply_runtime(thread_key, cx)?;
        let (draft_key, draft_token, draft) = self.prepare_slack_all_threads_reply_draft(
            SlackAllThreadsReplyDraftInput {
                thread_key,
                draft_key,
                conversation_id,
                thread_timestamp,
                broadcast_supported: *broadcast_supported,
            },
            cx,
        )?;
        let team_id = self.slack_workspace()?.team_id.clone();
        let generation = self.next_slack_thread_reply_send_generation();
        let request = SlackAllThreadsReplyRequest {
            generation,
            team_id,
            thread_key: thread_key.to_string(),
            draft_key,
            draft_token,
            draft_document_revision: draft.document_revision,
            conversation_id: conversation_id.clone(),
            thread_timestamp: thread_timestamp.clone(),
            payload: draft.payload,
            broadcast: draft.broadcast,
        };
        if !self.start_slack_thread_reply_send(request.draft_key.clone(), request.pending_send()) {
            return None;
        }
        self.slack_all_threads_reply_errors
            .remove(thread_key.as_ref());
        cx.notify();
        Some(SlackAllThreadsReplyWork {
            workspace_api,
            request,
        })
    }

    fn slack_all_threads_reply_target_is_available(
        &self,
        target: &SlackReplyComposerTarget,
        draft_key: &SlackComposerDraftKey,
    ) -> bool {
        self.slack_main_route == SlackMainRoute::AllThreads
            && self.slack_workspace_api_capabilities.send_thread_reply
            && self.can_mutate_slack_reply_composer(target)
            && !self.slack_thread_reply_mutation_is_blocked(draft_key)
    }

    fn slack_all_threads_reply_runtime(
        &mut self,
        thread_key: &str,
        cx: &mut Context<Self>,
    ) -> Option<Arc<dyn WorkspaceApi>> {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.set_slack_all_threads_reply_error(
                thread_key,
                "Slack thread replies require a connected workspace.".to_string(),
                cx,
            );
            return None;
        };
        Some(workspace_api)
    }

    fn prepare_slack_all_threads_reply_draft(
        &mut self,
        input: SlackAllThreadsReplyDraftInput<'_>,
        cx: &mut Context<Self>,
    ) -> Option<(SlackComposerDraftKey, u64, PreparedSlackThreadReplyDraft)> {
        let SlackAllThreadsReplyDraftInput {
            thread_key,
            draft_key,
            conversation_id,
            thread_timestamp,
            broadcast_supported,
        } = input;
        let share_files = self.slack_workspace_api_capabilities.share_files;
        let Some(draft_state) = self.slack_composer_drafts.get_mut(draft_key) else {
            self.set_slack_all_threads_reply_error(
                thread_key,
                "Add a reply or attachment before sending.".to_string(),
                cx,
            );
            return None;
        };
        let draft_token = draft_state.token;
        match prepare_slack_thread_reply_draft(
            draft_state,
            conversation_id,
            thread_timestamp,
            broadcast_supported,
            share_files,
        ) {
            Ok(prepared) => Some((draft_key.clone(), draft_token, prepared)),
            Err(error) => {
                self.set_slack_all_threads_reply_error(thread_key, error, cx);
                None
            }
        }
    }

    fn set_slack_all_threads_reply_error(
        &mut self,
        thread_key: &str,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.slack_all_threads_reply_errors
            .insert(thread_key.to_string(), error);
        cx.notify();
    }

    fn finish_slack_all_threads_reply(
        &mut self,
        request: SlackAllThreadsReplyRequest,
        result: Result<PreparedSlackThreadReplySendResult, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_thread_reply_send_is_pending(&request.draft_key, request.pending_send()) {
            return;
        }
        let prepared = match validate_slack_all_threads_reply(&request, result) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_slack_all_threads_reply(&request, error, cx);
                return;
            }
        };
        self.clear_submitted_slack_thread_reply_draft(
            &request.draft_key,
            request.pending_send(),
            cx,
        );
        self.finish_slack_thread_reply_pending_send(&request.draft_key, request.pending_send());
        let _ = self.restore_matching_slack_composer_schedule_recovery(cx);
        let all_threads_applied =
            self.apply_successful_slack_all_threads_reply(&request, prepared, cx);
        self.reconcile_successful_slack_thread_reply(&request.draft_key, all_threads_applied, cx);
        cx.notify();
    }

    fn apply_successful_slack_all_threads_reply(
        &mut self,
        request: &SlackAllThreadsReplyRequest,
        prepared: PreparedSlackThreadReplySendResult,
        cx: &mut Context<Self>,
    ) -> bool {
        let all_threads_thread_key = self
            .slack_all_threads_surface_thread_key(&request.draft_key)
            .filter(|thread_key| thread_key == &request.thread_key);
        let panel_is_current = self.slack_thread_reply_panel_is_current(&request.draft_key);
        match prepared {
            PreparedSlackThreadReplySendResult::Message(prepared) => {
                if let Some(thread_key) = all_threads_thread_key.as_deref() {
                    self.slack_all_threads_reply_errors
                        .remove(&request.thread_key);
                    self.apply_slack_all_threads_reply_receipt(thread_key, prepared.receipt);
                    self.slack_all_threads_list_state.remeasure();
                    true
                } else if panel_is_current {
                    let panel = self
                        .slack_thread_panel
                        .as_mut()
                        .expect("current Slack thread disappeared while applying its sent reply");
                    panel.reply_error = None;
                    panel.reply_composer_focused = true;
                    self.apply_sent_slack_thread_reply(prepared.reply_row, cx);
                    false
                } else {
                    false
                }
            }
            PreparedSlackThreadReplySendResult::Share(prepared) => {
                self.slack_all_threads_reply_errors
                    .remove(&request.thread_key);
                if panel_is_current {
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

    fn slack_all_threads_reply_surface_is_current(
        &self,
        request: &SlackAllThreadsReplyRequest,
    ) -> bool {
        self.slack_all_threads_surface_thread_key(&request.draft_key)
            .as_deref()
            == Some(request.thread_key.as_str())
    }

    fn fail_slack_all_threads_reply(
        &mut self,
        request: &SlackAllThreadsReplyRequest,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if !self.finish_slack_thread_reply_pending_send(&request.draft_key, request.pending_send())
        {
            return;
        }
        if self.slack_all_threads_reply_surface_is_current(request) {
            self.slack_all_threads_reply_errors
                .insert(request.thread_key.clone(), error);
        } else if self.slack_thread_reply_panel_is_current(&request.draft_key) {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("current Slack thread disappeared while applying its reply error");
            panel.reply_error = Some(error);
            panel.reply_composer_focused = true;
            panel.list_state.remeasure();
        }
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn apply_slack_all_threads_reply_receipt(
        &mut self,
        thread_key: &str,
        receipt: crate::ui::SlackThreadReplyReceipt,
    ) {
        let self_user_id = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_user_id.clone());
        let snapshot = self
            .slack_all_threads_snapshot
            .as_mut()
            .expect("current Slack All Threads route requires a snapshot");
        let thread = snapshot
            .threads
            .iter_mut()
            .find(|thread| thread.id == thread_key)
            .expect("current Slack All Threads reply target should remain in the snapshot");
        if !thread
            .visible_replies
            .iter()
            .any(|reply| reply.id == receipt.reply.id)
        {
            let participant_name = if receipt.reply.user_id.as_deref() == self_user_id.as_deref() {
                "you".to_string()
            } else {
                receipt.reply.author.clone()
            };
            if !participant_name.is_empty()
                && !thread
                    .participant_names
                    .iter()
                    .any(|name| name == &participant_name)
            {
                thread.participant_names.push(participant_name);
            }
            thread.visible_replies.push(receipt.reply);
            thread.reply_count = thread
                .reply_count
                .checked_add(1)
                .expect("Slack All Threads reply count overflowed");
        }
        let mut prepared = prepare_slack_all_threads_snapshot(snapshot.clone());
        self.slack_presence_authority
            .overlay_prepared_all_threads(&mut prepared);
        self.slack_all_threads_snapshot = Some(prepared.snapshot);
        self.slack_all_threads_rows = prepared.rows;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_all_threads(data);
    }
}

fn validate_slack_all_threads_reply(
    request: &SlackAllThreadsReplyRequest,
    result: Result<PreparedSlackThreadReplySendResult, String>,
) -> Result<PreparedSlackThreadReplySendResult, String> {
    let prepared = result?;
    match &prepared {
        PreparedSlackThreadReplySendResult::Message(prepared) => {
            let receipt = &prepared.receipt;
            if receipt.team_id != request.team_id
                || receipt.conversation_id != request.conversation_id
                || receipt.thread_timestamp != request.thread_timestamp.as_str()
                || receipt.broadcast != request.broadcast
                || receipt.reply.id == request.thread_timestamp.as_str()
                || SlackMessageTimestamp::parse(&receipt.reply.id).is_err()
            {
                return Err("Slack reply response did not match the requested thread.".to_string());
            }
        }
        PreparedSlackThreadReplySendResult::Share(prepared) => {
            if prepared.snapshot.team_id != request.team_id
                || prepared.snapshot.conversation_id != request.conversation_id
                || prepared.snapshot.thread_timestamp != request.thread_timestamp.as_str()
            {
                return Err(
                    "Slack file-share response did not match the requested thread.".to_string(),
                );
            }
        }
    }
    Ok(prepared)
}
