use super::{Context, SlackScheduleOverlay, SlackSchedulePostAt, SurfaceState};

impl SurfaceState {
    pub(crate) fn cancel_slack_scheduled_edit(&mut self, cx: &mut Context<Self>) {
        if self.restore_slack_scheduled_edit_prior_draft(cx) {
            self.slack_schedule_overlay = None;
            self.slack_schedule_submission_error = None;
            self.slack_error = None;
            cx.notify();
        }
    }

    pub(crate) fn save_slack_scheduled_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self
            .slack_active_scheduled_edit
            .as_ref()
            .map(|active| active.edit.clone())
        else {
            return;
        };
        let Some(owner) = self.current_slack_main_schedule_owner() else {
            return;
        };
        if self
            .slack_schedule_draft_identity(&owner)
            .is_none_or(|identity| identity.conversation_id != edit.conversation_id)
        {
            self.show_slack_schedule_error(
                &owner,
                "Wait for the scheduled message conversation to finish loading.",
                cx,
            );
            return;
        }
        let post_at = SlackSchedulePostAt(edit.post_at_unix_seconds);
        self.submit_slack_schedule(owner, post_at, cx);
    }

    pub(crate) fn control_submit_slack_schedule(
        &mut self,
        post_at_unix_seconds: i64,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatScheduleState, String> {
        if self.slack_schedule_pending.is_some() {
            return Err("a Slack scheduled draft create or update is already pending".to_string());
        }
        if let Some(diagnostic) = self.slack_composer_schedule_recovery_diagnostic() {
            return Err(diagnostic);
        }
        let owner = self
            .current_slack_main_schedule_owner()
            .ok_or_else(|| "Chat has no loaded Slack schedule target".to_string())?;
        self.submit_slack_schedule(owner, SlackSchedulePostAt(post_at_unix_seconds), cx);
        let state = self.control_slack_schedule_state()?;
        if state.pending_create_or_update.is_none() {
            return Err(state.submission_error.unwrap_or_else(|| {
                "Slack scheduled draft submission could not be initiated".to_string()
            }));
        }
        Ok(state)
    }

    pub(crate) fn submit_slack_composer(&mut self, cx: &mut Context<Self>) {
        if self.slack_schedule_blocks_current_composer_mutation()
            || self.slack_composer_capture_blocks_current_draft()
        {
            return;
        }
        if self.slack_active_scheduled_edit.is_some() {
            self.save_slack_scheduled_edit(cx);
        } else {
            self.send_slack_message(cx);
        }
    }

    pub(crate) fn open_slack_custom_schedule(&mut self, cx: &mut Context<Self>) {
        let Some(overlay) = self.slack_schedule_overlay.as_mut() else {
            return;
        };
        let SlackScheduleOverlay::Menu(menu) = &overlay.phase else {
            return;
        };
        let custom = menu.custom.clone();
        let owner = overlay.owner.clone();
        overlay.phase = SlackScheduleOverlay::Custom(custom);
        self.slack_schedule_submission_error = None;
        self.clear_slack_schedule_owner_error(&owner);
        cx.notify();
    }
}
