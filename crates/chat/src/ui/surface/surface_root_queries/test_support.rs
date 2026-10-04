use super::{Context, SurfaceRoot};

impl SurfaceRoot {
    pub(crate) fn slack_error<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_error.clone())
    }

    pub(crate) fn set_test_slack_error<AppState: 'static>(
        &mut self,
        message: &str,
        cx: &mut Context<AppState>,
    ) {
        let message = message.to_string();
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_error = Some(message);
        });
    }

    pub(crate) fn enter_test_slack_new_message_target<AppState: 'static>(
        &mut self,
        draft_key: &str,
        cx: &mut Context<AppState>,
    ) {
        let draft_key = draft_key.to_string();
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            let (conversation_id, label, kind) = {
                let workspace = surface
                    .slack_workspace()
                    .expect("test New Message target requires a workspace");
                (
                    workspace.conversation_id.clone(),
                    workspace.channel_name.clone(),
                    workspace.channel_kind,
                )
            };
            surface.slack_new_message_destination =
                Some(crate::ui::surface::SlackNewMessageDestination {
                    conversation_id: conversation_id.into(),
                    label: label.into(),
                    kind,
                });
            surface.slack_new_message_active_draft_key = Some(draft_key);
            surface.slack_main_route = crate::ui::surface::SlackMainRoute::NewMessage;
            surface.restore_slack_new_message_draft();
        });
    }

    pub(crate) fn leave_test_slack_new_message_target<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.store_slack_new_message_draft(cx);
            surface.slack_main_route = crate::ui::surface::SlackMainRoute::Conversation;
            surface.slack_new_message_destination = None;
            surface.slack_new_message_active_draft_key = None;
            surface.restore_slack_send_draft(None);
        });
    }

    pub(crate) fn insert_test_slack_pending_send<AppState: 'static>(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<AppState>,
    ) {
        let conversation_id = conversation_id.to_string();
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            let workspace = surface
                .slack_workspace()
                .expect("test pending send requires a workspace");
            let request = crate::ui::surface::SlackSendRequest {
                generation: u64::MAX,
                draft_token: 0,
                client_message_id: crate::ui::SlackMessageClientId::generate(),
                team_id: workspace.team_id.clone(),
                self_user_id: workspace
                    .self_user_id
                    .clone()
                    .expect("test pending send requires a self user"),
                conversation_id: conversation_id.clone(),
                draft_source: crate::ui::surface::SlackSendDraftSource::Conversation {
                    conversation_id,
                },
                draft_text: "pending elsewhere".to_string(),
            };
            surface
                .slack_pending_sends
                .insert(request.generation, request);
        });
    }

    pub(crate) fn begin_test_slack_conversation_refresh<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.ensure_slack_conversation_live_sync(cx);
            let target = surface
                .slack_conversation_live_target
                .clone()
                .expect("test refresh requires an active Slack conversation");
            surface.slack_conversation_refresh_timer_generation = None;
            surface.slack_conversation_refresh_timer_serial = surface
                .slack_conversation_refresh_timer_serial
                .checked_add(1)
                .expect("test Slack refresh timer serial overflowed");
            surface.slack_conversation_refresh_request =
                Some(crate::ui::surface::SlackConversationRefreshRequest {
                    generation: surface.slack_conversation_live_generation,
                    revision: surface.slack_conversation_revision,
                    target,
                });
        });
    }

    pub(crate) fn finish_test_slack_conversation_refresh_without_changes<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            let request = surface
                .slack_conversation_refresh_request
                .clone()
                .expect("test refresh completion requires an in-flight request");
            surface.finish_slack_conversation_refresh(&request, Ok(None), cx);
        });
    }
}
