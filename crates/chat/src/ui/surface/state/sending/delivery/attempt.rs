use std::sync::Arc;

use crate::ui::surface::{
    prepare_slack_message_send_receipt, SlackMessageDeliveryState, SlackOutboundDelivery,
    SlackSendPayload, SlackSendRequest, SurfaceState,
};
use crate::ui::{Context, SlackMessageClientId, WorkspaceApi};

use super::super::{PreparedSlackSendResult, SlackSendWork};

impl SurfaceState {
    pub(super) fn spawn_slack_send_attempt(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        work: SlackSendWork,
        cx: &mut Context<Self>,
    ) {
        let completion_request = work.request.clone();
        self.spawn_background_task(
            work,
            cx,
            move |work| execute_slack_send(workspace_api, work),
            move |this, result, cx| {
                this.finish_slack_remote_send(&completion_request, result, cx);
            },
        );
    }

    pub(crate) fn retry_slack_outbound_delivery(
        &mut self,
        client_message_id: &SlackMessageClientId,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.send_message {
            return;
        }
        let Some(delivery) = self.retryable_slack_outbound_delivery(client_message_id) else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.fail_slack_outbound_delivery(
                &delivery.request,
                "Slack workspace API is unavailable.".to_string(),
            );
            cx.notify();
            return;
        };
        self.slack_send_generation = self
            .slack_send_generation
            .checked_add(1)
            .expect("Slack send request generation overflowed");
        let request = SlackSendRequest {
            generation: self.slack_send_generation,
            ..delivery.request
        };
        let work = SlackSendWork {
            request: request.clone(),
            payload: delivery
                .retry_payload
                .expect("retryable Slack delivery must retain its immutable payload"),
            previous_message: self.slack_previous_message_for_send(&request),
        };
        let ledger_entry = self
            .slack_outbound_delivery_ledger
            .iter_mut()
            .find(|entry| entry.request.client_message_id == *client_message_id)
            .expect("retryable Slack delivery must remain in the ledger");
        ledger_entry.request = request.clone();
        ledger_entry.row.delivery = Some(SlackMessageDeliveryState::Pending {
            label: "Sending…".into(),
        });
        self.slack_pending_sends.insert(request.generation, request);
        self.sync_slack_local_delivery_rows(false, false);
        cx.notify();
        self.spawn_slack_send_attempt(workspace_api, work, cx);
    }

    fn retryable_slack_outbound_delivery(
        &self,
        client_message_id: &SlackMessageClientId,
    ) -> Option<SlackOutboundDelivery> {
        let delivery = self
            .slack_outbound_delivery_ledger
            .iter()
            .find(|delivery| {
                &delivery.request.client_message_id == client_message_id
                    && matches!(
                        delivery.row.delivery.as_ref(),
                        Some(SlackMessageDeliveryState::Failed { .. })
                    )
                    && delivery.retry_payload.is_some()
            })
            .cloned()?;
        if !self.slack_send_request_identity_is_current(&delivery.request)
            || self
                .slack_pending_sends
                .values()
                .any(|request| request.client_message_id == *client_message_id)
        {
            return None;
        }
        Some(delivery)
    }
}

fn execute_slack_send(
    workspace_api: Arc<dyn WorkspaceApi>,
    work: SlackSendWork,
) -> Result<PreparedSlackSendResult, String> {
    let SlackSendWork {
        request,
        payload,
        previous_message,
    } = work;
    match payload.as_ref() {
        SlackSendPayload::Message(draft) => workspace_api
            .send_slack_message(&request.conversation_id, &request.client_message_id, draft)
            .map(|receipt| {
                PreparedSlackSendResult::Message(Box::new(prepare_slack_message_send_receipt(
                    receipt,
                    previous_message.map(|message| *message),
                )))
            }),
        SlackSendPayload::Share(share) => match workspace_api.share_slack_files(share)? {
            crate::model::SlackFileShareReceipt::Conversation(snapshot) => {
                Ok(PreparedSlackSendResult::Share(Box::new(snapshot)))
            }
            crate::model::SlackFileShareReceipt::Thread(_) => {
                Err("Slack conversation file share returned a thread refresh receipt".to_string())
            }
        },
    }
}
