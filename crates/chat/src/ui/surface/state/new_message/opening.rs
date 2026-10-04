use super::{
    next_slack_new_message_generation, Arc, Context, SlackConversationKind,
    SlackConversationOpenReceipt, SlackConversationOpenRequest, SlackMainRoute,
    SlackNewMessageDestination, SlackNewMessageOpenLoad, SurfaceState, WorkspaceApi,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn queue_slack_new_message_people_open(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(team_id) = self.slack_new_message_team_id.clone() else {
            return;
        };
        if !self.slack_workspace_api_capabilities.open_conversation {
            self.slack_new_message_error =
                Some("Slack conversations.open is unavailable for this workspace.".to_string());
            cx.notify();
            return;
        }
        let user_ids = self
            .slack_new_message_selected_people
            .iter()
            .map(|person| person.user_id.to_string())
            .collect::<Vec<_>>();
        let request = match SlackConversationOpenRequest::new(team_id, user_ids) {
            Ok(request) => request,
            Err(error) => {
                self.slack_new_message_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.slack_new_message_open_generation =
            next_slack_new_message_generation(self.slack_new_message_open_generation);
        self.slack_new_message_pending_open = Some(SlackNewMessageOpenLoad {
            generation: self.slack_new_message_open_generation,
            request,
        });
        self.start_next_slack_new_message_open(cx);
    }

    pub(in crate::ui::surface::state) fn start_next_slack_new_message_open(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_new_message_open_request.is_some() {
            return;
        }
        let Some(load) = self.slack_new_message_pending_open.take() else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_new_message_error =
                Some("Slack New message requires a connected workspace.".to_string());
            cx.notify();
            return;
        };
        self.slack_new_message_open_request = Some(load.clone());
        self.slack_new_message_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, load),
            cx,
            |(workspace_api, load): (Arc<dyn WorkspaceApi>, SlackNewMessageOpenLoad)| {
                let result = workspace_api.open_slack_conversation(load.request.clone());
                (load, result)
            },
            |this, (load, result), cx| {
                this.finish_slack_new_message_open(load, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn finish_slack_new_message_open(
        &mut self,
        load: SlackNewMessageOpenLoad,
        result: Result<SlackConversationOpenReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_new_message_open_request.as_ref() != Some(&load) {
            return;
        }
        self.slack_new_message_open_request = None;
        if self.slack_main_route == SlackMainRoute::NewMessage
            && self.slack_new_message_open_generation == load.generation
        {
            match result {
                Ok(receipt)
                    if receipt.team_id == load.request.team_id()
                        && receipt.user_ids == load.request.user_ids() =>
                {
                    let label = self
                        .slack_new_message_selected_people
                        .iter()
                        .map(|person| person.label.as_ref())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let kind = if receipt.user_ids.len() == 1 {
                        SlackConversationKind::DirectMessage
                    } else {
                        SlackConversationKind::GroupMessage
                    };
                    let conversation_id = receipt.conversation_id;
                    self.slack_new_message_destination = Some(SlackNewMessageDestination {
                        conversation_id: conversation_id.clone().into(),
                        label: label.into(),
                        kind,
                    });
                    self.slack_new_message_error = None;
                    self.select_slack_conversation_for_new_message(&conversation_id, cx);
                }
                Ok(_) => {
                    self.slack_new_message_error =
                        Some("Slack conversations.open returned another request.".to_string());
                }
                Err(error) => self.slack_new_message_error = Some(error),
            }
            cx.notify();
        }
        self.start_next_slack_new_message_open(cx);
    }
}
