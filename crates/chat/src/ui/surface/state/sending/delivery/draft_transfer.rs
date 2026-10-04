use crate::ui::surface::{SlackSendRequest, SurfaceState};
use crate::ui::SlackMessageClientId;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn clear_matching_slack_send_draft(
        &mut self,
        request: &SlackSendRequest,
    ) {
        if !self.slack_send_request_target_is_current(request)
            || self.slack_composer_text != request.draft_text
            || self.slack_send_draft_token != request.draft_token
            || self.slack_send_client_message_id.as_ref() != Some(&request.client_message_id)
        {
            return;
        }
        let active_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("sending a Slack draft requires its exact active composer identity");
        let transferred_draft = self.take_slack_send_draft();
        assert_eq!(
            active_handle.draft_id, transferred_draft.id,
            "Slack send draft transfer must retain its exact draft identity"
        );
        assert!(
            self.slack_outbound_delivery_ledger.iter().any(|delivery| {
                delivery.request == *request
                    && delivery.accepted_draft_handle == active_handle
                    && delivery
                        .accepted_draft
                        .files
                        .same_identity_and_order(&transferred_draft.files)
            }),
            "Slack send draft files must be owned by the outbound delivery before clearing the composer"
        );
        self.slack_composer_text.clear();
        self.slack_composer_document.borrow_mut().clear();
        self.advance_slack_send_draft_revision();
        self.slack_send_client_message_id = Some(SlackMessageClientId::generate());
    }
}
