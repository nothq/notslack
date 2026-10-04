use crate::ui::surface::{
    prepare_slack_conversation_refresh_reusing_rows, prepare_slack_conversation_snapshot,
    PreparedSlackMessageSendReceipt, SlackSendRequest, SurfaceState,
};
use crate::ui::{Context, SlackConversationSnapshot};

use super::super::PreparedSlackSendResult;

impl SurfaceState {
    pub(super) fn finish_slack_remote_send(
        &mut self,
        request: &SlackSendRequest,
        result: Result<PreparedSlackSendResult, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_sends.get(&request.generation) != Some(request) {
            return;
        }
        self.slack_pending_sends.remove(&request.generation);
        let target_is_current = self.slack_send_request_target_is_current(request);
        match result {
            Ok(PreparedSlackSendResult::Message(prepared)) => {
                self.finish_slack_message_send(request, *prepared, target_is_current, cx)
            }
            Ok(PreparedSlackSendResult::Share(prepared)) => {
                self.finish_slack_file_share_send(request, *prepared, cx)
            }
            Err(message) => self.finish_failed_slack_send(request, message, cx),
        }
    }

    fn finish_slack_message_send(
        &mut self,
        request: &SlackSendRequest,
        prepared: PreparedSlackMessageSendReceipt,
        target_is_current: bool,
        cx: &mut Context<Self>,
    ) {
        let workspace_is_current = self.slack_send_request_workspace_is_current(request);
        if let Err(message) = self.validate_slack_send_receipt(request, &prepared.receipt) {
            self.fail_slack_outbound_delivery(request, message);
            self.queue_slack_send_reconciliation_if_live(request, cx);
            cx.notify();
            return;
        }
        let activity_matches = self
            .slack_activity_detail
            .composer()
            .is_some_and(|composer| {
                composer.target.team_id == request.team_id
                    && composer.target.self_user_id == request.self_user_id
                    && composer.target.conversation_id == request.conversation_id
            });
        if workspace_is_current {
            if let Err(message) =
                self.apply_prepared_slack_message_send_receipt(request, prepared.clone(), cx)
            {
                self.fail_slack_outbound_delivery(request, message);
                self.queue_slack_send_reconciliation_if_live(request, cx);
                cx.notify();
                return;
            }
        }
        if activity_matches {
            if let Err(message) =
                self.apply_prepared_slack_activity_message_send_receipt(request, prepared, cx)
            {
                self.fail_slack_outbound_delivery(request, message);
                self.queue_slack_send_reconciliation_if_live(request, cx);
                cx.notify();
                return;
            }
        }
        if target_is_current || workspace_is_current || activity_matches {
            self.queue_slack_send_reconciliation_if_live(request, cx);
        }
        if let Some(delivery) = self.take_slack_outbound_delivery(request) {
            self.discard_slack_outbound_delivery_draft(delivery, cx);
        }
        cx.notify();
    }

    fn finish_slack_file_share_send(
        &mut self,
        request: &SlackSendRequest,
        refreshed: SlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) {
        if refreshed.team_id != request.team_id
            || refreshed.conversation_id != request.conversation_id
        {
            let message = "Slack send refresh returned a mismatched conversation state.";
            self.fail_slack_outbound_delivery(request, message.to_string());
            cx.notify();
            return;
        }
        if let Err(message) = self.apply_slack_file_share_activity_refresh(request, &refreshed, cx)
        {
            self.fail_slack_outbound_delivery(request, message);
            self.queue_slack_send_reconciliation_if_live(request, cx);
            cx.notify();
            return;
        }
        if let Err(message) =
            self.apply_slack_file_share_conversation_refresh(request, refreshed, cx)
        {
            self.fail_slack_outbound_delivery(request, message);
            self.queue_slack_send_reconciliation_if_live(request, cx);
            cx.notify();
            return;
        }
        if let Some(delivery) = self.take_slack_outbound_delivery(request) {
            self.discard_slack_outbound_delivery_draft(delivery, cx);
        }
        self.queue_slack_send_reconciliation_if_live(request, cx);
        cx.notify();
    }

    fn apply_slack_file_share_activity_refresh(
        &mut self,
        request: &SlackSendRequest,
        refreshed: &SlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let activity_composer = self
            .slack_activity_detail
            .composer()
            .filter(|composer| {
                composer.target.team_id == request.team_id
                    && composer.target.self_user_id == request.self_user_id
                    && composer.target.conversation_id == request.conversation_id
            })
            .cloned();
        let Some(activity_composer) = activity_composer else {
            return Ok(());
        };
        let prepared = prepare_slack_conversation_snapshot(refreshed.clone());
        let force_scroll_to_end = activity_composer.source == request.draft_source;
        self.apply_slack_activity_conversation_refresh(prepared, force_scroll_to_end, cx)
    }

    fn apply_slack_file_share_conversation_refresh(
        &mut self,
        request: &SlackSendRequest,
        refreshed: SlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.slack_send_request_workspace_is_current(request) {
            return Ok(());
        }
        let existing = self
            .slack_conversation_snapshot
            .as_ref()
            .filter(|snapshot| {
                snapshot.team_id == request.team_id
                    && snapshot.conversation_id == request.conversation_id
            })
            .cloned()
            .ok_or_else(|| {
                "Slack file-share refresh requires the active conversation state.".to_string()
            })?;
        let prepared = prepare_slack_conversation_refresh_reusing_rows(
            existing,
            self.slack_message_rows.clone(),
            self.slack_message_rows_local_today,
            refreshed,
        )
        .map_err(|error| format!("Slack file-share refresh could not be reconciled: {error}"))?;
        if let Some(prepared) = prepared {
            self.apply_prepared_slack_conversation_snapshot(prepared, cx);
        }
        Ok(())
    }

    fn finish_failed_slack_send(
        &mut self,
        request: &SlackSendRequest,
        message: String,
        cx: &mut Context<Self>,
    ) {
        self.fail_slack_outbound_delivery(request, message);
        self.queue_slack_send_reconciliation_if_live(request, cx);
        cx.notify();
    }

    fn queue_slack_send_reconciliation_if_live(
        &mut self,
        request: &SlackSendRequest,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_conversation_live_target
            .as_ref()
            .is_some_and(|target| {
                target.team_id == request.team_id
                    && target.conversation_id == request.conversation_id
            })
        {
            self.queue_slack_conversation_reconciliation(cx);
        }
    }
}
