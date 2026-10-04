use std::sync::Arc;

use crate::ui::surface::{
    SlackFileStagingDraftOwner, SlackMessageDeliveryAction, SlackMessageDeliveryActionKind,
    SlackMessageDeliveryState, SlackOutboundDelivery, SlackSendDraftSource, SlackSendRequest,
    SurfaceState,
};
use crate::ui::Context;

impl SurfaceState {
    pub(crate) fn slack_message_display_row_count(&self) -> usize {
        self.slack_message_rows
            .len()
            .saturating_add(self.slack_local_delivery_rows.len())
    }

    pub(in crate::ui::surface::state) fn rebuild_slack_local_delivery_rows(&mut self) {
        let Some(workspace) = self.slack_workspace() else {
            self.slack_local_delivery_rows = Arc::default();
            return;
        };
        let team_id = workspace.team_id.clone();
        let self_user_id = workspace.self_user_id.clone();
        let conversation_id = workspace.conversation_id.clone();
        self.slack_local_delivery_rows = self
            .slack_outbound_delivery_ledger
            .iter()
            .filter(|delivery| {
                delivery.request.team_id == team_id
                    && Some(delivery.request.self_user_id.as_str()) == self_user_id.as_deref()
                    && delivery.request.conversation_id == conversation_id
            })
            .map(|delivery| delivery.row.clone())
            .collect::<Vec<_>>()
            .into();
    }

    pub(in crate::ui::surface::state) fn rebuild_slack_activity_local_delivery_rows(&mut self) {
        let Some(composer) = self.slack_activity_detail.composer() else {
            self.slack_activity_local_delivery_rows = Arc::default();
            return;
        };
        self.slack_activity_local_delivery_rows = self
            .slack_outbound_delivery_ledger
            .iter()
            .filter(|delivery| {
                delivery.request.team_id == composer.target.team_id
                    && delivery.request.self_user_id == composer.target.self_user_id
                    && delivery.request.conversation_id == composer.target.conversation_id
            })
            .map(|delivery| delivery.row.clone())
            .collect::<Vec<_>>()
            .into();
    }

    fn sync_slack_activity_local_delivery_rows(&mut self, scroll_to_end: bool) {
        let previous_rows = self.slack_activity_local_delivery_rows.clone();
        let authoritative_count = match &self.slack_activity_detail {
            crate::ui::surface::SlackActivityDetailState::Loaded { rows, .. } => rows.len(),
            _ => 0,
        };
        let previous_display_count = authoritative_count.saturating_add(previous_rows.len());
        let was_following_end = self.slack_activity_detail_list_state.is_following_tail();
        self.rebuild_slack_activity_local_delivery_rows();
        if self.slack_activity_local_delivery_rows == previous_rows {
            return;
        }
        let next_local_count = self.slack_activity_local_delivery_rows.len();
        if self.slack_activity_detail_list_state.item_count() == previous_display_count {
            self.slack_activity_detail_list_state.splice(
                authoritative_count..authoritative_count.saturating_add(previous_rows.len()),
                next_local_count,
            );
        } else {
            self.slack_activity_detail_list_state
                .reset(authoritative_count.saturating_add(next_local_count));
        }
        if scroll_to_end || was_following_end {
            self.slack_activity_detail_list_state.scroll_to_end();
        }
    }

    pub(super) fn sync_slack_local_delivery_rows(
        &mut self,
        scroll_primary_to_end: bool,
        scroll_activity_to_end: bool,
    ) {
        let previous_rows = self.slack_local_delivery_rows.clone();
        let previous_display_count = self
            .slack_message_rows
            .len()
            .saturating_add(previous_rows.len());
        let was_following_end = self.slack_message_list_state.is_following_tail();
        self.rebuild_slack_local_delivery_rows();
        self.sync_slack_activity_local_delivery_rows(scroll_activity_to_end);
        if self.slack_local_delivery_rows == previous_rows {
            return;
        }
        let authoritative_count = self.slack_message_rows.len();
        let next_local_count = self.slack_local_delivery_rows.len();
        if self.slack_message_list_state.item_count() == previous_display_count {
            self.slack_message_list_state.splice(
                authoritative_count..authoritative_count.saturating_add(previous_rows.len()),
                next_local_count,
            );
        } else {
            self.slack_message_list_state
                .reset(authoritative_count.saturating_add(next_local_count));
        }
        if scroll_primary_to_end || was_following_end {
            self.slack_message_list_state.scroll_to_end();
        }
    }

    pub(super) fn insert_slack_outbound_delivery(&mut self, delivery: SlackOutboundDelivery) {
        assert!(
            !self.slack_outbound_delivery_ledger.iter().any(|existing| {
                existing.request.client_message_id == delivery.request.client_message_id
            }),
            "Slack outbound delivery client message ids must be unique"
        );
        let activity_source = matches!(
            delivery.request.draft_source,
            SlackSendDraftSource::Activity { .. }
        );
        let scroll_activity_to_end = self
            .slack_activity_detail
            .composer()
            .is_some_and(|composer| composer.source == delivery.request.draft_source);
        self.slack_outbound_delivery_ledger.push(delivery);
        self.sync_slack_local_delivery_rows(!activity_source, scroll_activity_to_end);
    }

    pub(super) fn take_slack_outbound_delivery(
        &mut self,
        request: &SlackSendRequest,
    ) -> Option<SlackOutboundDelivery> {
        let index = self
            .slack_outbound_delivery_ledger
            .iter()
            .position(|delivery| {
                delivery.request.client_message_id == request.client_message_id
                    && delivery.request.team_id == request.team_id
                    && delivery.request.self_user_id == request.self_user_id
                    && delivery.request.conversation_id == request.conversation_id
            })?;
        let delivery = self.slack_outbound_delivery_ledger.remove(index);
        self.sync_slack_local_delivery_rows(false, false);
        Some(delivery)
    }

    pub(super) fn discard_slack_outbound_delivery_draft(
        &mut self,
        delivery: SlackOutboundDelivery,
        cx: &mut Context<Self>,
    ) {
        assert_eq!(
            delivery.accepted_draft_handle.draft_id, delivery.accepted_draft.id,
            "Slack outbound delivery draft handle must match its accepted draft"
        );
        self.discard_slack_composer_draft(
            SlackFileStagingDraftOwner::Main(delivery.accepted_draft_handle.owner),
            delivery.accepted_draft,
            cx,
        );
    }

    pub(super) fn fail_slack_outbound_delivery(
        &mut self,
        request: &SlackSendRequest,
        message: String,
    ) -> bool {
        let Some(delivery) = self
            .slack_outbound_delivery_ledger
            .iter_mut()
            .find(|delivery| delivery.request == *request)
        else {
            return false;
        };
        let retryable = delivery.retry_payload.is_some();
        let detail = if retryable {
            message
        } else {
            format!(
                "{message} Slack file shares cannot be retried safely because their delivery \
                 status may be ambiguous. Check the conversation, then restore the original \
                 draft or dismiss this failed delivery."
            )
        };
        let client_message_id = request.client_message_id.clone();
        let mut actions = Vec::with_capacity(if retryable { 3 } else { 2 });
        if retryable {
            actions.push(SlackMessageDeliveryAction::new(
                SlackMessageDeliveryActionKind::Retry,
                client_message_id.clone(),
            ));
        }
        actions.push(SlackMessageDeliveryAction::new(
            SlackMessageDeliveryActionKind::Restore,
            client_message_id.clone(),
        ));
        actions.push(SlackMessageDeliveryAction::new(
            SlackMessageDeliveryActionKind::Dismiss,
            client_message_id,
        ));
        delivery.row.delivery = Some(SlackMessageDeliveryState::Failed {
            label: "Message failed to send".into(),
            detail: detail.into(),
            actions: actions.into(),
        });
        self.sync_slack_local_delivery_rows(false, false);
        true
    }
}
