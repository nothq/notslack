use super::{
    prepare_slack_sidebar_snapshot, slack_channel_is_starred, ClipboardItem, Context,
    PreparedSlackSidebarSnapshot, SlackChannelPermalinkRequest, SlackChannelStarRequest,
    SlackStarMutation, SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn slack_channel_star_mutation(&self) -> SlackStarMutation {
        if self.slack_active_channel_is_starred() {
            SlackStarMutation::Remove
        } else {
            SlackStarMutation::Add
        }
    }

    pub(crate) fn slack_active_channel_is_starred(&self) -> bool {
        self.slack_workspace().is_some_and(|workspace| {
            slack_channel_is_starred(&workspace.sections, workspace.conversation_id.as_str())
        })
    }

    pub(crate) fn toggle_slack_active_channel_star(&mut self, cx: &mut Context<Self>) {
        let mutation = self.slack_channel_star_mutation();
        self.start_slack_channel_star_mutation(mutation, cx);
    }

    pub(in crate::ui::surface::state) fn start_slack_channel_star_mutation(
        &mut self,
        mutation: SlackStarMutation,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.mutate_stars
            || self.slack_pending_channel_star.is_some()
            || self.slack_channel_star_mutation() != mutation
        {
            self.close_slack_channel_menu(cx);
            return;
        }
        let Some((team_id, conversation_id)) = self
            .slack_workspace()
            .map(|workspace| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        else {
            self.close_slack_channel_menu(cx);
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.close_slack_channel_menu(cx);
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        let request = self.next_slack_channel_star_request(team_id, conversation_id, mutation);
        let collapsed_sections = self.slack_collapsed_sections.clone();
        let muted_conversations = self.slack_muted_conversations.clone();
        self.close_slack_channel_menu(cx);
        self.slack_pending_channel_star = Some(request.clone());
        self.slack_error = None;
        cx.notify();

        let completion_request = request.clone();
        self.spawn_background_task(
            request,
            cx,
            move |request| {
                workspace_api
                    .mutate_slack_channel_star(&request.conversation_id, request.mutation)
                    .map(|sidebar| {
                        prepare_slack_sidebar_snapshot(
                            sidebar,
                            &collapsed_sections,
                            &muted_conversations,
                        )
                    })
            },
            move |this, result, cx| {
                this.finish_slack_channel_star_mutation(&completion_request, result, cx);
            },
        );
    }

    fn next_slack_channel_star_request(
        &mut self,
        team_id: String,
        conversation_id: String,
        mutation: SlackStarMutation,
    ) -> SlackChannelStarRequest {
        self.slack_channel_star_generation = self
            .slack_channel_star_generation
            .checked_add(1)
            .expect("Slack channel star request generation overflowed");
        SlackChannelStarRequest {
            generation: self.slack_channel_star_generation,
            team_id,
            conversation_id,
            mutation,
        }
    }

    pub(in crate::ui::surface::state) fn finish_slack_channel_star_mutation(
        &mut self,
        request: &SlackChannelStarRequest,
        result: Result<PreparedSlackSidebarSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_channel_star.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_channel_star = None;
        if self.slack_channel_star_generation != request.generation
            || !self
                .slack_channel_request_target_is_current(&request.team_id, &request.conversation_id)
        {
            cx.notify();
            return;
        }
        match result {
            Ok(prepared)
                if prepared.snapshot.team_id == request.team_id
                    && prepared.snapshot.conversation_id == request.conversation_id
                    && slack_channel_is_starred(
                        &prepared.snapshot.sections,
                        &request.conversation_id,
                    ) == request.mutation.active_after() =>
            {
                self.apply_prepared_slack_sidebar_snapshot(prepared, cx);
            }
            Ok(_) => {
                self.slack_error =
                    Some("Slack returned mismatched channel star state.".to_string());
                cx.notify();
            }
            Err(message) => {
                self.slack_error = Some(message);
                cx.notify();
            }
        }
    }

    pub(in crate::ui::surface::state) fn start_slack_channel_permalink_copy(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_channel_permalink
            || self.slack_pending_channel_permalink.is_some()
        {
            self.close_slack_channel_menu(cx);
            return;
        }
        let Some((team_id, conversation_id)) = self
            .slack_workspace()
            .map(|workspace| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        else {
            self.close_slack_channel_menu(cx);
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.close_slack_channel_menu(cx);
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        self.slack_channel_permalink_generation = self
            .slack_channel_permalink_generation
            .checked_add(1)
            .expect("Slack channel permalink request generation overflowed");
        let request = SlackChannelPermalinkRequest {
            generation: self.slack_channel_permalink_generation,
            team_id,
            conversation_id,
        };
        self.close_slack_channel_menu(cx);
        self.slack_pending_channel_permalink = Some(request.clone());
        self.slack_error = None;
        cx.notify();

        let completion_request = request.clone();
        self.spawn_background_task(
            request,
            cx,
            move |request| workspace_api.load_slack_channel_permalink(&request.conversation_id),
            move |this, result, cx| {
                this.finish_slack_channel_permalink_copy(&completion_request, result, cx);
            },
        );
    }

    fn finish_slack_channel_permalink_copy(
        &mut self,
        request: &SlackChannelPermalinkRequest,
        result: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_channel_permalink.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_channel_permalink = None;
        if self.slack_channel_permalink_generation != request.generation
            || !self
                .slack_channel_request_target_is_current(&request.team_id, &request.conversation_id)
        {
            cx.notify();
            return;
        }
        match result {
            Ok(permalink) => cx.write_to_clipboard(ClipboardItem::new_string(permalink)),
            Err(message) => self.slack_error = Some(message),
        }
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn slack_channel_request_target_is_current(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> bool {
        self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == team_id && workspace.conversation_id == conversation_id
        }) && self
            .slack_pending_conversation_id
            .as_deref()
            .is_none_or(|pending| pending == conversation_id)
    }
}
