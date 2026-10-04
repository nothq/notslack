use crate::ui::surface::{
    SlackMessageDeliveryAction, SlackMessageDeliveryActionKind, SlackMessageDeliveryState,
    SurfaceState,
};
use crate::ui::Context;
use crate::ui::SlackMessageClientId;

impl SurfaceState {
    pub(crate) fn activate_slack_outbound_delivery_action(
        &mut self,
        action: &SlackMessageDeliveryAction,
        cx: &mut Context<Self>,
    ) {
        let supported = self
            .slack_outbound_delivery_ledger
            .iter()
            .find(|delivery| delivery.request.client_message_id == *action.client_message_id())
            .is_some_and(|delivery| {
                matches!(
                    delivery.row.delivery.as_ref(),
                    Some(SlackMessageDeliveryState::Failed { actions, .. })
                        if actions.iter().any(|candidate| candidate == action)
                )
            });
        if !supported {
            return;
        }
        match action.kind() {
            SlackMessageDeliveryActionKind::Retry => {
                self.retry_slack_outbound_delivery(action.client_message_id(), cx);
            }
            SlackMessageDeliveryActionKind::Restore => {
                self.restore_slack_outbound_delivery(action.client_message_id(), cx);
            }
            SlackMessageDeliveryActionKind::Dismiss => {
                self.dismiss_slack_outbound_delivery(action.client_message_id(), cx);
            }
        }
    }

    fn restore_slack_outbound_delivery(
        &mut self,
        client_message_id: &SlackMessageClientId,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self
            .slack_outbound_delivery_ledger
            .iter()
            .find(|delivery| {
                delivery.request.client_message_id == *client_message_id
                    && matches!(
                        delivery.row.delivery.as_ref(),
                        Some(SlackMessageDeliveryState::Failed { .. })
                    )
            })
            .map(|delivery| delivery.request.clone())
        else {
            return;
        };
        if !self.slack_send_request_target_is_current(&request) {
            return;
        }
        if !self.snapshot_slack_send_draft().is_empty() {
            self.slack_error = Some(
                "Finish or clear the current draft before restoring this failed message."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        let current_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("restoring a Slack outbound draft requires an active main composer");
        let Some(delivery) = self.take_slack_outbound_delivery(&request) else {
            return;
        };
        assert_eq!(
            current_handle.owner, delivery.accepted_draft_handle.owner,
            "Slack outbound delivery must restore into its exact composer owner"
        );
        assert_eq!(
            delivery.accepted_draft_handle.draft_id, delivery.accepted_draft.id,
            "Slack outbound delivery draft handle must match its accepted draft"
        );
        self.restore_slack_send_draft(Some(delivery.accepted_draft));
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    fn dismiss_slack_outbound_delivery(
        &mut self,
        client_message_id: &SlackMessageClientId,
        cx: &mut Context<Self>,
    ) {
        let request = self
            .slack_outbound_delivery_ledger
            .iter()
            .find(|delivery| {
                delivery.request.client_message_id == *client_message_id
                    && matches!(
                        delivery.row.delivery.as_ref(),
                        Some(SlackMessageDeliveryState::Failed { .. })
                    )
            })
            .map(|delivery| delivery.request.clone());
        if let Some(delivery) = request
            .as_ref()
            .and_then(|request| self.take_slack_outbound_delivery(request))
        {
            self.discard_slack_outbound_delivery_draft(delivery, cx);
            cx.notify();
        }
    }
}
