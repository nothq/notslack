use super::super::{normalize_slack_thread_parent_row, Context, SurfaceState};

impl SurfaceState {
    pub(crate) fn close_slack_thread_panel(&mut self, cx: &mut Context<Self>) {
        if self.slack_thread_panel.is_none() {
            return;
        }
        self.cancel_slack_message_navigation();
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.origin.later_item_key().is_some())
        {
            self.slack_later_selected_key = None;
        }
        self.reset_slack_thread_context();
        cx.notify();
    }

    pub(crate) fn reset_slack_thread_context(&mut self) {
        self.clear_slack_thread_context(true);
    }

    pub(in crate::ui::surface::state) fn discard_slack_thread_context(&mut self) {
        self.clear_slack_thread_context(false);
    }

    fn clear_slack_thread_context(&mut self, store_draft: bool) {
        self.slack_thread_generation = self
            .slack_thread_generation
            .checked_add(1)
            .expect("Slack thread request generation overflowed");
        if matches!(
            self.slack_composer_aux_target.as_ref(),
            Some(crate::ui::surface::SlackComposerTarget::Reply(
                crate::ui::surface::SlackReplyComposerTarget::ThreadPanel { .. }
            ))
        ) {
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
            self.slack_mention_picker_state = None;
        }
        if store_draft {
            self.check_in_current_slack_thread_draft();
        }
        self.reset_slack_thread_read_context();
        self.slack_thread_panel = None;
    }

    pub(super) fn check_in_current_slack_thread_draft(&mut self) {
        if self.slack_thread_panel.is_none() {
            return;
        }
        let replacement_id = self.next_slack_composer_draft_id();
        let Some((key, draft)) = self.slack_thread_panel.as_mut().map(|panel| {
            (
                panel.reply_draft_key.clone(),
                std::mem::replace(
                    panel.reply_draft.get_mut(),
                    crate::ui::surface::SlackComposerDraft::new(replacement_id),
                ),
            )
        }) else {
            return;
        };
        self.store_slack_composer_draft(key, draft);
    }

    pub(crate) fn reset_slack_later_thread_context(&mut self) {
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.origin.later_item_key().is_some())
        {
            self.reset_slack_thread_context();
        }
    }

    pub(crate) fn reconcile_slack_thread_parent(&mut self) {
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| !panel.origin.is_conversation())
        {
            return;
        }
        let Some(parent_message_id) = self
            .slack_thread_panel
            .as_ref()
            .map(|panel| panel.parent_message_id.clone())
        else {
            return;
        };
        let Some(mut parent_row) = self
            .slack_message_rows
            .iter()
            .find(|row| row.id == parent_message_id)
            .cloned()
        else {
            self.reset_slack_thread_context();
            return;
        };
        normalize_slack_thread_parent_row(&mut parent_row);
        self.slack_thread_panel
            .as_mut()
            .expect("Slack thread panel disappeared during parent reconciliation")
            .parent_hydrated = true;
        self.slack_thread_panel
            .as_mut()
            .expect("Slack thread panel disappeared during parent reconciliation")
            .parent_row = parent_row;
        self.slack_thread_panel
            .as_ref()
            .expect("Slack thread panel disappeared after parent reconciliation")
            .list_state
            .remeasure_items(0..1);
    }
}
