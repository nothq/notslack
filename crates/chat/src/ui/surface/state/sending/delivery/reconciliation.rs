use crate::ui::surface::{SlackAuthoredClientMessageIds, SlackSendRequest, SurfaceState};
use crate::ui::Context;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn reconcile_slack_current_conversation_outbound_deliveries(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self.slack_conversation_snapshot.as_ref() else {
            return;
        };
        let team_id = snapshot.team_id.clone();
        let conversation_id = snapshot.conversation_id.clone();
        let authored_client_message_ids = snapshot
            .messages
            .iter()
            .filter_map(|message| {
                Some((message.client_message_id.clone()?, message.user_id.clone()?))
            })
            .collect::<SlackAuthoredClientMessageIds>();
        self.reconcile_slack_observed_client_messages(
            &team_id,
            &conversation_id,
            &authored_client_message_ids,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn reconcile_slack_observed_client_messages(
        &mut self,
        team_id: &str,
        conversation_id: &str,
        authored_client_message_ids: &SlackAuthoredClientMessageIds,
        cx: &mut Context<Self>,
    ) {
        if authored_client_message_ids.is_empty() {
            return;
        }
        let message_was_observed = |request: &SlackSendRequest| {
            request.team_id == team_id
                && request.conversation_id == conversation_id
                && authored_client_message_ids.contains(&(
                    request.client_message_id.clone(),
                    request.self_user_id.clone(),
                ))
        };
        let observed_requests = self
            .slack_outbound_delivery_ledger
            .iter()
            .map(|delivery| &delivery.request)
            .filter(|request| message_was_observed(request))
            .cloned()
            .collect::<Vec<_>>();
        if observed_requests.is_empty() {
            return;
        }
        for request in &observed_requests {
            self.slack_pending_sends.remove(&request.generation);
        }
        for request in observed_requests {
            if let Some(delivery) = self.take_slack_outbound_delivery(&request) {
                self.discard_slack_outbound_delivery_draft(delivery, cx);
            }
        }
        cx.notify();
    }
}
