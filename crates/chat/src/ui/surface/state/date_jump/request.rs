use super::{
    prepare_slack_conversation_snapshot, px, slack_date_jump_end_timestamp, slack_date_jump_today,
    slack_message_timezone, spawn_background_task_for_entity, Arc, Context, Date, ListOffset,
    PreparedSlackConversationSnapshot, SlackDateJumpRequest, SlackDateJumpTarget, SurfaceState,
    WorkspaceApi,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn current_slack_date_jump_today(
        &self,
    ) -> Result<Date, String> {
        let timezone = self
            .slack_workspace()
            .map(|workspace| slack_message_timezone(workspace.self_timezone_id.as_deref()))
            .ok_or_else(|| "Slack date navigation requires an active conversation.".to_string())?;
        slack_date_jump_today(timezone)
    }

    pub(in crate::ui::surface::state) fn jump_slack_to_date(
        &mut self,
        local_date: Date,
        cx: &mut Context<Self>,
    ) {
        self.slack_date_jump_overlay = None;
        self.invalidate_slack_date_jump_request();
        if let Some(index) = self
            .slack_message_rows
            .iter()
            .position(|row| row.local_date == Some(local_date))
        {
            self.scroll_to_slack_date_jump_index(index, cx);
            return;
        }
        if !self
            .slack_workspace_api_capabilities
            .navigate_conversation_dates
        {
            self.fail_slack_date_jump(
                "Slack cannot load messages for the selected date.".to_string(),
                cx,
            );
            return;
        }
        let timezone = match self.slack_workspace() {
            Some(workspace) => slack_message_timezone(workspace.self_timezone_id.as_deref()),
            None => return,
        };
        let anchor_timestamp = match slack_date_jump_end_timestamp(local_date, timezone) {
            Ok(timestamp) => timestamp,
            Err(error) => {
                self.fail_slack_date_jump(error, cx);
                return;
            }
        };
        self.begin_slack_date_jump_request(
            SlackDateJumpTarget::Date {
                local_date,
                anchor_timestamp,
            },
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn jump_slack_to_beginning(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_date_jump_overlay = None;
        self.invalidate_slack_date_jump_request();
        if self
            .slack_conversation_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.history_next_cursor.is_none())
        {
            self.scroll_to_slack_date_jump_index(0, cx);
            return;
        }
        if !self
            .slack_workspace_api_capabilities
            .navigate_conversation_dates
        {
            self.fail_slack_date_jump(
                "Slack cannot load the beginning of this conversation.".to_string(),
                cx,
            );
            return;
        }
        self.begin_slack_date_jump_request(SlackDateJumpTarget::Beginning, cx);
    }

    fn begin_slack_date_jump_request(
        &mut self,
        target: SlackDateJumpTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let team_id = workspace.team_id.clone();
        let conversation_id = workspace.conversation_id.clone();
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.fail_slack_date_jump(
                "Slack date navigation requires a connected workspace.".to_string(),
                cx,
            );
            return;
        };
        self.slack_date_jump_generation = self
            .slack_date_jump_generation
            .checked_add(1)
            .expect("Slack date-jump request generation overflowed");
        let request = SlackDateJumpRequest {
            generation: self.slack_date_jump_generation,
            team_id,
            conversation_id,
            target,
        };
        self.slack_date_jump_request = Some(request.clone());
        self.slack_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackDateJumpRequest)| {
                let snapshot = match &request.target {
                    SlackDateJumpTarget::Date {
                        anchor_timestamp, ..
                    } => workspace_api
                        .load_slack_conversation_at(&request.conversation_id, anchor_timestamp),
                    SlackDateJumpTarget::Beginning => {
                        workspace_api.load_slack_conversation_beginning(&request.conversation_id)
                    }
                };
                let result = snapshot.map(prepare_slack_conversation_snapshot);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_date_jump_request(request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn finish_slack_date_jump_request(
        &mut self,
        request: SlackDateJumpRequest,
        result: Result<PreparedSlackConversationSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_date_jump_request.as_ref() != Some(&request) {
            return;
        }
        self.slack_date_jump_request = None;
        let request_is_current = self.slack_date_jump_generation == request.generation
            && self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == request.team_id
                    && workspace.conversation_id == request.conversation_id
            });
        if !request_is_current {
            cx.notify();
            return;
        }
        let prepared = match result {
            Ok(prepared)
                if prepared.snapshot.team_id == request.team_id
                    && prepared.snapshot.conversation_id == request.conversation_id =>
            {
                prepared
            }
            Ok(_) => {
                self.fail_slack_date_jump(
                    "Slack date navigation returned a different conversation.".to_string(),
                    cx,
                );
                return;
            }
            Err(error) => {
                self.fail_slack_date_jump(error, cx);
                return;
            }
        };
        self.apply_prepared_slack_conversation_snapshot(prepared, cx);
        let index = match request.target {
            SlackDateJumpTarget::Date { local_date, .. } => self
                .slack_message_rows
                .iter()
                .position(|row| row.local_date == Some(local_date))
                .or_else(|| {
                    self.slack_message_rows
                        .iter()
                        .rposition(|row| row.local_date.is_some_and(|date| date <= local_date))
                })
                .unwrap_or(0),
            SlackDateJumpTarget::Beginning => 0,
        };
        self.scroll_to_slack_date_jump_index(index, cx);
    }

    pub(in crate::ui::surface::state) fn scroll_to_slack_date_jump_index(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if index >= self.slack_message_rows.len() {
            self.fail_slack_date_jump(
                "Slack did not return any messages for that date.".to_string(),
                cx,
            );
            return;
        }
        self.slack_message_list_auto_position_active = false;
        self.slack_message_list_state.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.0),
        });
        self.slack_error = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn fail_slack_date_jump(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.slack_date_jump_overlay = None;
        self.invalidate_slack_date_jump_request();
        self.slack_error = Some(error);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn invalidate_slack_date_jump_request(&mut self) {
        if self.slack_date_jump_request.take().is_some() {
            self.slack_date_jump_generation = self
                .slack_date_jump_generation
                .checked_add(1)
                .expect("Slack date-jump request generation overflowed");
        }
    }
}
