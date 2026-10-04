use std::sync::Arc;

use super::super::{
    next_slack_later_generation, Context, SlackLaterFilter, SlackRailView, SurfaceState,
    WorkspaceApi,
};
use crate::ui::SlackReminderMutation;

struct SlackLaterReminderMutationRequest {
    generation: u64,
    team_id: String,
    filter: SlackLaterFilter,
    mutation: SlackReminderMutation,
}

type SlackLaterReminderMutationResult = (SlackLaterReminderMutationRequest, Result<(), String>);

impl SurfaceState {
    pub(super) fn begin_slack_later_reminder_mutation(
        &mut self,
        mutation: SlackReminderMutation,
        cx: &mut Context<Self>,
    ) {
        if self.slack_later_reminder_mutating
            || !self.slack_workspace_api_capabilities.mutate_later_reminders
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let Some(team_id) = self.slack_later_team_id.clone() else {
            return;
        };
        let filter = if matches!(&mutation, SlackReminderMutation::Create { .. }) {
            SlackLaterFilter::Saved
        } else {
            self.slack_later_filter
        };
        self.slack_later_reminder_generation =
            next_slack_later_generation(self.slack_later_reminder_generation);
        let request = SlackLaterReminderMutationRequest {
            generation: self.slack_later_reminder_generation,
            team_id,
            filter,
            mutation,
        };
        self.slack_later_reminder_mutating = true;
        self.slack_later_reminder_error = None;
        if let Some(dialog) = self.slack_later_reminder_dialog.as_mut() {
            dialog.saving = true;
            dialog.error = None;
        }
        cx.notify();
        self.spawn_background_task(
            request,
            cx,
            move |request| mutate_slack_later_reminder(workspace_api, request),
            move |this, (request, result), cx| {
                this.finish_slack_later_reminder_mutation(request, result, cx);
            },
        );
    }

    fn finish_slack_later_reminder_mutation(
        &mut self,
        request: SlackLaterReminderMutationRequest,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if request.generation != self.slack_later_reminder_generation
            || self.slack_later_team_id.as_deref() != Some(request.team_id.as_str())
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            return;
        }
        self.slack_later_reminder_mutating = false;
        match result {
            Ok(()) => {
                self.slack_later_reminder_dialog = None;
                self.slack_later_reminder_error = None;
                self.invalidate_slack_later_after_reminder_mutation(request.filter, cx);
            }
            Err(error) => {
                if let Some(dialog) = self.slack_later_reminder_dialog.as_mut() {
                    dialog.saving = false;
                    dialog.error = Some(error);
                } else {
                    self.slack_later_reminder_error = Some(error);
                }
            }
        }
        cx.notify();
    }

    fn invalidate_slack_later_after_reminder_mutation(
        &mut self,
        filter: SlackLaterFilter,
        cx: &mut Context<Self>,
    ) {
        self.slack_later_generation = next_slack_later_generation(self.slack_later_generation);
        self.slack_later_filter = filter;
        self.slack_later_snapshot = None;
        self.slack_later_rows.clear();
        self.slack_later_list_state.reset(0);
        self.slack_later_hydration_queue.reset();
        self.slack_later_loading = false;
        self.slack_later_error = None;
        self.reset_slack_later_thread_context();
        if self.slack_active_rail_view == SlackRailView::Later {
            self.begin_slack_later_page(None, cx);
        }
    }
}

fn mutate_slack_later_reminder(
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackLaterReminderMutationRequest,
) -> SlackLaterReminderMutationResult {
    let result = workspace_api.mutate_slack_reminder(&request.mutation);
    (request, result)
}
