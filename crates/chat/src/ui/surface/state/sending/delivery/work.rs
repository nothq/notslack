use std::sync::Arc;

use crate::ui::surface::{
    build_slack_local_delivery_row, SlackLocalDeliveryRowInput, SlackMessageDeliveryState,
    SlackOutboundDelivery, SlackSendPayload, SlackSendRequest, SurfaceState,
};
use crate::ui::{SlackMessage, SlackMessageClientId, SlackMessageDraft};

use super::super::{PreparedSlackSend, SlackSendWork};

impl SurfaceState {
    pub(super) fn build_slack_send_work(
        &mut self,
        prepared: PreparedSlackSend,
    ) -> Result<(SlackOutboundDelivery, SlackSendWork), String> {
        self.slack_send_generation = self
            .slack_send_generation
            .checked_add(1)
            .expect("Slack send request generation overflowed");
        let client_message_id = self
            .slack_send_client_message_id
            .get_or_insert_with(SlackMessageClientId::generate)
            .clone();
        let request = SlackSendRequest {
            generation: self.slack_send_generation,
            draft_token: self.slack_send_draft_token,
            client_message_id,
            team_id: prepared.team_id,
            self_user_id: prepared.self_user_id,
            conversation_id: prepared.conversation_id,
            draft_source: prepared.draft_source,
            draft_text: prepared.draft_text,
        };
        let mut accepted_draft = prepared.accepted_draft;
        accepted_draft.client_message_id = Some(request.client_message_id.clone());
        let attachments = accepted_draft.files.cloned_attachments();
        let local_row = build_slack_local_delivery_row(SlackLocalDeliveryRowInput {
            client_message_id: &request.client_message_id,
            author: &prepared.author,
            self_user_id: &request.self_user_id,
            avatar_label: prepared.avatar_label.as_deref(),
            avatar_image_url: prepared.avatar_image_url.as_deref(),
            draft: prepared.message_draft.as_ref(),
            attachments: &attachments,
            timezone: prepared.timezone,
            team_id: &request.team_id,
            delivery: SlackMessageDeliveryState::Pending {
                label: "Sending…".into(),
            },
        });
        let payload =
            build_slack_send_payload(&request, prepared.message_draft, prepared.file_ids)?;
        let delivery = SlackOutboundDelivery {
            request: request.clone(),
            retry_payload: matches!(payload.as_ref(), SlackSendPayload::Message(_))
                .then(|| payload.clone()),
            accepted_draft_handle: prepared.accepted_draft_handle,
            accepted_draft,
            row: local_row,
        };
        let work = SlackSendWork {
            request,
            payload,
            previous_message: self.slack_previous_message_for_send(&delivery.request),
        };
        Ok((delivery, work))
    }

    pub(super) fn slack_previous_message_for_send(
        &self,
        request: &SlackSendRequest,
    ) -> Option<Box<SlackMessage>> {
        self.slack_conversation_snapshot
            .as_ref()
            .filter(|snapshot| {
                snapshot.team_id == request.team_id
                    && snapshot.conversation_id == request.conversation_id
            })
            .and_then(|snapshot| snapshot.messages.last())
            .cloned()
            .map(Box::new)
    }
}

fn build_slack_send_payload(
    request: &SlackSendRequest,
    message_draft: Option<SlackMessageDraft>,
    file_ids: Arc<[crate::model::SlackFileId]>,
) -> Result<Arc<SlackSendPayload>, String> {
    if file_ids.is_empty() {
        return Ok(Arc::new(SlackSendPayload::Message(
            message_draft.expect("validated Slack text send must have a message draft"),
        )));
    }
    let target = crate::model::SlackFileShareTarget::conversation(request.conversation_id.clone())?;
    let share = match message_draft {
        Some(message) => crate::model::SlackFileShareRequest::message(
            target,
            file_ids.to_vec(),
            request.client_message_id.clone(),
            message,
        )?,
        None => crate::model::SlackFileShareRequest::files_only(
            target,
            file_ids.to_vec(),
            request.client_message_id.clone(),
        )?,
    };
    Ok(Arc::new(SlackSendPayload::Share(share)))
}
