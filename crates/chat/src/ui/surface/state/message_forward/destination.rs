use super::{
    slack_message_forward_open_receipt_matches, Arc, Context, SlackConversationOpenReceipt,
    SlackConversationOpenRequest, SlackDestinationTarget, SlackMessageForwardDestination,
    SurfaceState, WorkspaceApi, SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(crate) fn select_slack_message_forward_destination(
        &mut self,
        visible_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some((generation, team_id, row)) =
            self.slack_message_forward_modal.as_ref().and_then(|modal| {
                let row_index = *modal.visible_row_indices.get(visible_index)?;
                let row = modal.rows.get(row_index)?.clone();
                Some((modal.generation, modal.source.team_id.clone(), row))
            })
        else {
            return;
        };
        let person_user_id = match row.target.clone() {
            SlackDestinationTarget::Conversation {
                conversation_id, ..
            } => {
                self.install_slack_message_forward_destination(
                    row,
                    Some(conversation_id.into()),
                    false,
                );
                None
            }
            SlackDestinationTarget::Person { user_id } => {
                if !self.slack_workspace_api_capabilities.open_conversation {
                    return;
                }
                self.install_slack_message_forward_destination(row, None, true);
                Some(user_id)
            }
        };
        cx.notify();
        let Some(user_id) = person_user_id else {
            return;
        };
        let request = match SlackConversationOpenRequest::new(team_id, vec![user_id]) {
            Ok(request) => request,
            Err(error) => {
                if let Some(destination) = self
                    .slack_message_forward_modal
                    .as_mut()
                    .and_then(|modal| modal.destination.as_mut())
                {
                    destination.opening = false;
                }
                if let Some(modal) = self.slack_message_forward_modal.as_mut() {
                    modal.error = Some(error);
                }
                cx.notify();
                return;
            }
        };
        self.begin_slack_message_forward_conversation_open(generation, request, cx);
    }

    fn install_slack_message_forward_destination(
        &mut self,
        row: crate::ui::surface::SlackNewMessageCandidateRow,
        conversation_id: Option<gpui::SharedString>,
        opening: bool,
    ) {
        let modal = self
            .slack_message_forward_modal
            .as_mut()
            .expect("forward modal disappeared while selecting a destination");
        modal.destination = Some(SlackMessageForwardDestination {
            target: row.target,
            kind: row.kind,
            label: row.label,
            conversation_id,
            opening,
        });
        modal.query.clear();
        modal.normalized_query = Default::default();
        modal.visible_row_indices = Arc::default();
        modal.selected_index = None;
        modal.error = None;
        self.slack_message_forward_focus_pending = false;
        self.slack_message_forward_note_focus_pending = true;
    }

    pub(in crate::ui::surface::state) fn begin_slack_message_forward_conversation_open(
        &mut self,
        generation: u64,
        request: SlackConversationOpenRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if let Some(modal) = self.slack_message_forward_modal.as_mut() {
                if let Some(destination) = modal.destination.as_mut() {
                    destination.opening = false;
                }
                modal.error = Some("Slack forwarding requires a connected workspace.".to_string());
            }
            cx.notify();
            return;
        };
        let completion_request = request.clone();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackConversationOpenRequest)| {
                workspace_api.open_slack_conversation(request)
            },
            move |this, result, cx| {
                this.finish_slack_message_forward_conversation_open(
                    generation,
                    &completion_request,
                    result,
                    cx,
                );
            },
        );
    }

    fn finish_slack_message_forward_conversation_open(
        &mut self,
        generation: u64,
        request: &SlackConversationOpenRequest,
        result: Result<SlackConversationOpenReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self
            .slack_message_forward_modal
            .as_mut()
            .filter(|modal| modal.generation == generation)
        else {
            return;
        };
        let Some(destination) = modal.destination.as_mut() else {
            return;
        };
        let SlackDestinationTarget::Person { user_id } = &destination.target else {
            return;
        };
        if request.user_ids().len() != 1 || request.user_ids()[0].as_str() != user_id.as_str() {
            return;
        }
        destination.opening = false;
        match result {
            Ok(receipt) if slack_message_forward_open_receipt_matches(request, &receipt) => {
                destination.conversation_id = Some(receipt.conversation_id.into());
                modal.error = None;
            }
            Ok(_) => {
                modal.error = Some("Slack conversations.open returned another request.".to_string())
            }
            Err(error) => modal.error = Some(error),
        }
        cx.notify();
    }

    pub(crate) fn clear_slack_message_forward_destination(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self
            .slack_message_forward_modal
            .as_mut()
            .filter(|modal| !modal.forwarding)
        else {
            return;
        };
        modal.destination = None;
        modal.error = None;
        self.rebuild_slack_message_forward_results();
        self.queue_slack_message_forward_visible_images(
            0,
            SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS,
            cx,
        );
        self.slack_message_forward_focus_pending = true;
        self.slack_message_forward_note_focus_pending = false;
        cx.notify();
    }
}
