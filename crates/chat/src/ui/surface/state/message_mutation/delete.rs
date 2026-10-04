use super::{
    prepare_slack_conversation_refresh, prepare_slack_conversation_snapshot, Context,
    PreparedSlackConversationSnapshot, SlackConversationKind, SlackMessage,
    SlackMessageDeleteModal, SlackMessageDeleteRequest, SlackMessageForwardSource,
    SlackMessageTimestamp, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn open_slack_message_delete(&mut self, message_id: &str, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.delete_message
            || self.slack_pending_message_delete.is_some()
        {
            return;
        }
        let Some(message) = self.current_own_slack_message(message_id) else {
            return;
        };
        let Some(row) = self
            .slack_message_rows
            .iter()
            .find(|row| row.id == message_id)
        else {
            return;
        };
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let Ok(message_timestamp) = SlackMessageTimestamp::parse(message_id) else {
            return;
        };
        let source = SlackMessageForwardSource {
            team_id: workspace.team_id.clone(),
            conversation_id: workspace.conversation_id.clone(),
            message_timestamp,
            private_message: matches!(
                workspace.channel_kind,
                SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
            ),
            author: row.author.clone().into(),
            timestamp_label: row.timestamp.clone().into(),
            preview: message.body.clone().into(),
        };
        self.slack_message_menu = None;
        self.slack_message_menu_focus_pending = false;
        self.slack_reaction_picker = None;
        self.slack_message_delete_modal = Some(SlackMessageDeleteModal {
            source,
            deleting: false,
            error: None,
        });
        self.slack_error = None;
        cx.notify();
    }

    pub(crate) fn close_slack_message_delete(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_message_delete_modal
            .as_ref()
            .is_none_or(|modal| modal.deleting)
        {
            return;
        }
        self.slack_message_delete_modal = None;
        cx.notify();
    }

    pub(crate) fn submit_slack_message_delete(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.delete_message
            || self.slack_pending_message_delete.is_some()
        {
            return;
        }
        let Some(source) = self
            .slack_message_delete_modal
            .as_ref()
            .filter(|modal| !modal.deleting)
            .map(|modal| modal.source.clone())
        else {
            return;
        };
        if !self.slack_message_action_target_is_current(&source.team_id, &source.conversation_id)
            || self
                .current_own_slack_message(source.message_timestamp.as_str())
                .is_none()
        {
            self.slack_message_delete_modal = None;
            cx.notify();
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if let Some(modal) = self.slack_message_delete_modal.as_mut() {
                modal.error =
                    Some("Slack message deletion requires a connected workspace.".to_string());
            }
            cx.notify();
            return;
        };
        let existing = self
            .slack_conversation_snapshot
            .clone()
            .expect("current Slack delete target requires a conversation snapshot");
        let request = self.begin_slack_message_delete_request(source);
        cx.notify();

        let completion_request = request.clone();
        self.spawn_background_task(
            (request, existing),
            cx,
            move |(request, existing)| {
                let refreshed = workspace_api
                    .delete_slack_message(&request.conversation_id, &request.message_timestamp)?;
                let fallback = refreshed.clone();
                prepare_slack_conversation_refresh(existing, refreshed).map(|prepared| {
                    prepared.unwrap_or_else(|| prepare_slack_conversation_snapshot(fallback))
                })
            },
            move |this, result, cx| {
                this.finish_slack_message_delete(&completion_request, result, cx);
            },
        );
    }

    fn begin_slack_message_delete_request(
        &mut self,
        source: SlackMessageForwardSource,
    ) -> SlackMessageDeleteRequest {
        self.slack_message_delete_generation = self
            .slack_message_delete_generation
            .checked_add(1)
            .expect("Slack message delete generation overflowed");
        let request = SlackMessageDeleteRequest {
            generation: self.slack_message_delete_generation,
            team_id: source.team_id,
            conversation_id: source.conversation_id,
            message_timestamp: source.message_timestamp,
        };
        self.slack_pending_message_delete = Some(request.clone());
        let modal = self
            .slack_message_delete_modal
            .as_mut()
            .expect("Slack delete modal disappeared while starting deletion");
        modal.deleting = true;
        modal.error = None;
        request
    }

    pub(in crate::ui::surface::state) fn finish_slack_message_delete(
        &mut self,
        request: &SlackMessageDeleteRequest,
        result: Result<PreparedSlackConversationSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_delete.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_message_delete = None;
        if self.slack_message_delete_generation != request.generation
            || !self
                .slack_message_action_target_is_current(&request.team_id, &request.conversation_id)
        {
            return;
        }
        match result {
            Ok(prepared)
                if prepared.snapshot.team_id == request.team_id
                    && prepared.snapshot.conversation_id == request.conversation_id
                    && !prepared
                        .snapshot
                        .messages
                        .iter()
                        .any(|message| message.id == request.message_timestamp.as_str()) =>
            {
                self.slack_message_delete_modal = None;
                self.apply_prepared_slack_conversation_snapshot(prepared, cx);
            }
            Ok(_) => self.finish_slack_message_delete_error(
                "Slack returned a snapshot that still contains the deleted message.".to_string(),
                cx,
            ),
            Err(error) => self.finish_slack_message_delete_error(error, cx),
        }
    }

    pub(in crate::ui::surface::state) fn finish_slack_message_delete_error(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_message_delete_modal.as_mut() else {
            return;
        };
        modal.deleting = false;
        modal.error = Some(error);
        cx.notify();
    }

    pub(crate) fn reset_slack_message_mutation_context(&mut self) {
        if self.slack_message_edit.take().is_some()
            || self.slack_pending_message_edit.take().is_some()
        {
            self.slack_message_edit_generation = self
                .slack_message_edit_generation
                .checked_add(1)
                .expect("Slack message edit generation overflowed");
        }
        if self.slack_message_delete_modal.take().is_some()
            || self.slack_pending_message_delete.take().is_some()
        {
            self.slack_message_delete_generation = self
                .slack_message_delete_generation
                .checked_add(1)
                .expect("Slack message delete generation overflowed");
        }
    }

    pub(in crate::ui::surface::state) fn current_own_slack_message(
        &self,
        message_id: &str,
    ) -> Option<&SlackMessage> {
        let workspace = self.slack_workspace()?;
        let self_user_id = workspace.self_user_id.as_deref()?;
        let snapshot = self.slack_conversation_snapshot.as_ref()?;
        if snapshot.team_id != workspace.team_id
            || snapshot.conversation_id != workspace.conversation_id
        {
            return None;
        }
        snapshot.messages.iter().find(|message| {
            message.id == message_id && message.user_id.as_deref() == Some(self_user_id)
        })
    }
}
