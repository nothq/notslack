use super::{
    slack_schedule_draft_has_content, slack_schedule_menu_state, Context, SlackScheduleAnchor,
    SlackScheduleDraftOwner, SlackScheduleMenuState, SlackScheduleOverlay,
    SlackScheduleOverlayState, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn open_slack_send_options(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.current_slack_main_schedule_owner() else {
            return;
        };
        self.open_slack_send_options_for_owner(owner, self.slack_schedule_keyboard_anchor(), cx);
    }

    pub(crate) fn open_slack_send_options_for_owner(
        &mut self,
        owner: SlackScheduleDraftOwner,
        anchor: SlackScheduleAnchor,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.prepare_slack_schedule_menu(&owner, cx) else {
            return;
        };
        self.slack_aux_panel = None;
        self.slack_schedule_overlay = Some(SlackScheduleOverlayState {
            owner: owner.clone(),
            anchor,
            phase: SlackScheduleOverlay::Menu(menu),
        });
        self.slack_schedule_submission_error = None;
        self.clear_slack_schedule_owner_error(&owner);
        cx.notify();
    }

    fn prepare_slack_schedule_menu(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduleMenuState> {
        if let Some(diagnostic) = self.slack_schedule_blocking_owner_diagnostic() {
            self.fail_slack_schedule(owner, &diagnostic, cx);
            return None;
        }
        if self.slack_schedule_create_or_update_is_blocked() {
            return None;
        }
        if !self.has_slack_schedule_target(owner) {
            return None;
        }
        let Some(draft) = self.snapshot_slack_schedule_draft(owner) else {
            self.fail_slack_schedule(
                owner,
                "The selected Slack composer changed before scheduling could begin.",
                cx,
            );
            return None;
        };
        if !slack_schedule_draft_has_content(&draft) {
            self.fail_slack_schedule(owner, "Add a message or attachment before scheduling.", cx);
            return None;
        }
        if !draft.files.slack_file_ids_ready() {
            self.fail_slack_schedule(
                owner,
                "Wait for all Slack attachments to finish staging before scheduling.",
                cx,
            );
            return None;
        }
        if owner
            .draft_key()
            .is_some_and(|key| self.slack_thread_reply_is_pending(key))
        {
            return None;
        }
        let (timezone_id, timezone_label) = self.slack_workspace().and_then(|workspace| {
            Some((
                workspace.self_timezone_id.as_deref()?.trim().to_string(),
                workspace.self_timezone_label.as_deref()?.trim().to_string(),
            ))
        })?;
        match slack_schedule_menu_state(&timezone_id, timezone_label) {
            Ok(menu) => Some(menu),
            Err(message) => {
                self.fail_slack_schedule(owner, &message, cx);
                None
            }
        }
    }

    pub(crate) fn close_slack_schedule_overlay(&mut self, cx: &mut Context<Self>) {
        if self.slack_schedule_overlay.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn clear_slack_schedule_owner_error(&mut self, owner: &SlackScheduleDraftOwner) {
        match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                if self.slack_schedule_owner_surface_is_current(owner) {
                    self.slack_error = None;
                }
            }
            SlackScheduleDraftOwner::ThreadPanel {
                handle,
                panel_generation,
            } => {
                if let Some(panel) = self.slack_thread_panel.as_mut().filter(|panel| {
                    panel.generation == *panel_generation && panel.reply_draft_key == *handle.key()
                }) {
                    panel.reply_error = None;
                }
            }
            SlackScheduleDraftOwner::AllThreads { thread_key, .. } => {
                self.slack_all_threads_reply_errors
                    .remove(thread_key.as_ref());
            }
        }
    }

    pub(super) fn set_slack_schedule_owner_error(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        message: &str,
    ) {
        match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                if self.slack_schedule_owner_surface_is_current(owner) {
                    self.slack_error = Some(message.to_string());
                }
            }
            SlackScheduleDraftOwner::ThreadPanel {
                handle,
                panel_generation,
            } => {
                if let Some(panel) = self.slack_thread_panel.as_mut().filter(|panel| {
                    panel.generation == *panel_generation && panel.reply_draft_key == *handle.key()
                }) {
                    panel.reply_error = Some(message.to_string());
                }
            }
            SlackScheduleDraftOwner::AllThreads { thread_key, .. } => {
                self.slack_all_threads_reply_errors
                    .insert(thread_key.to_string(), message.to_string());
            }
        }
    }

    pub(super) fn focus_slack_schedule_owner(&mut self, owner: &SlackScheduleDraftOwner) {
        match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                if self.slack_schedule_owner_surface_is_current(owner) {
                    self.slack_composer_focused = true;
                }
            }
            SlackScheduleDraftOwner::ThreadPanel {
                handle,
                panel_generation,
            } => {
                if let Some(panel) = self.slack_thread_panel.as_mut().filter(|panel| {
                    panel.generation == *panel_generation && panel.reply_draft_key == *handle.key()
                }) {
                    panel.reply_composer_focused = true;
                }
            }
            SlackScheduleDraftOwner::AllThreads { .. } => {}
        }
    }

    pub(crate) fn dismiss_slack_schedule_layer(&mut self, cx: &mut Context<Self>) {
        if let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        {
            if custom.picker.take().is_some() {
                cx.notify();
                return;
            }
        }
        self.close_slack_schedule_overlay(cx);
    }

    pub(crate) fn reset_slack_schedule_context(&mut self, cx: &mut Context<Self>) {
        let retained_diagnostic = self.slack_schedule_blocking_owner_diagnostic();
        self.slack_schedule_overlay = None;
        self.slack_schedule_submission_error = retained_diagnostic;
        self.restore_slack_scheduled_edit_prior_draft(cx);
    }
}
