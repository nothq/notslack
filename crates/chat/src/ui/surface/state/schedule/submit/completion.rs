use super::super::{Context, SurfaceState};
use super::execution::append_refresh_error;
use super::SlackScheduleTaskResult;
use crate::ui::surface::{
    SlackSchedulePendingSubmission, SlackScheduledDraftDisposition, SlackScheduledMutationIdentity,
    SlackScheduledPendingOrigin, SlackScheduledPendingPhase,
};

mod recovery;

impl SurfaceState {
    pub(super) fn finish_slack_schedule_submission(
        &mut self,
        result: SlackScheduleTaskResult,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.slack_schedule_pending.as_ref() else {
            return;
        };
        if pending.identity != result.identity
            || self.slack_schedule_generation != result.identity.generation
        {
            return;
        }
        let refresh_error = self.apply_slack_schedule_refresh(&result.identity, result.refreshed);
        match result.outcome {
            Ok(_) => {
                let pending = self
                    .slack_schedule_pending
                    .take()
                    .expect("confirmed schedule must retain its pending owner");
                self.confirm_slack_schedule_owner(pending, refresh_error, cx);
            }
            Err(failure) if failure.is_definite() => {
                let pending = self
                    .slack_schedule_pending
                    .take()
                    .expect("definite schedule failure must retain its pending owner");
                self.restore_slack_schedule_owner_after_failure(
                    pending,
                    failure.diagnostic().to_string(),
                    refresh_error,
                    cx,
                );
            }
            Err(crate::model::SlackScheduledDraftMutationFailure::Unknown { diagnostic }) => {
                self.begin_slack_schedule_unknown_reconciliation(
                    result.identity,
                    diagnostic,
                    refresh_error,
                    cx,
                );
            }
            Err(
                crate::model::SlackScheduledDraftMutationFailure::NotSent { .. }
                | crate::model::SlackScheduledDraftMutationFailure::Rejected { .. },
            ) => unreachable!("definite scheduled-draft failures are handled above"),
        }
    }

    fn begin_slack_schedule_unknown_reconciliation(
        &mut self,
        identity: SlackScheduledMutationIdentity,
        diagnostic: String,
        refresh_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let mirrors_active_composer = self
            .slack_schedule_pending
            .as_ref()
            .is_some_and(|pending| self.slack_schedule_pending_origin_is_current(pending));
        let owner = self
            .slack_schedule_pending
            .as_ref()
            .expect("unknown schedule outcome must retain its pending owner")
            .owner
            .clone();
        let pending = self
            .slack_schedule_pending
            .as_mut()
            .expect("unknown schedule outcome must retain its pending owner");
        pending.phase = SlackScheduledPendingPhase::ReconcilingUnknown {
            attempt: 1,
            diagnostic: diagnostic.clone(),
        };
        self.slack_schedule_submission_error = Some(diagnostic.clone());
        if mirrors_active_composer {
            let diagnostic = append_refresh_error(diagnostic, refresh_error);
            self.set_slack_schedule_owner_error(&owner, &diagnostic);
        }
        cx.notify();
        self.schedule_slack_unknown_reconciliation(identity, 1, cx);
    }

    pub(super) fn confirm_slack_schedule_owner(
        &mut self,
        pending: SlackSchedulePendingSubmission,
        refresh_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let should_focus = self.slack_schedule_pending_origin_is_current(&pending);
        let cleanup_api = pending.workspace_api.clone();
        match &pending.origin {
            SlackScheduledPendingOrigin::Composer => {
                if let Some(key) = pending.owner.draft_key() {
                    self.confirm_slack_draft_schedule_promotion(
                        key,
                        pending.identity.generation,
                        pending.accepted_draft.token,
                    );
                }
            }
            SlackScheduledPendingOrigin::ScheduledEdit { .. } => {}
        }
        let deferred_remote_removals = match pending.origin {
            SlackScheduledPendingOrigin::Composer => Vec::new(),
            SlackScheduledPendingOrigin::ScheduledEdit {
                deferred_remote_removals,
                ..
            } => deferred_remote_removals,
        };
        let owner = pending.owner;
        self.finish_slack_scheduled_draft(
            SlackScheduledDraftDisposition {
                owner: owner.clone(),
                draft: pending.accepted_draft,
                deferred_remote_removals,
                cleanup_api: Some(cleanup_api),
            },
            cx,
        );
        self.slack_schedule_submission_error = None;
        if should_focus {
            self.clear_slack_schedule_owner_error(&owner);
            if let Some(refresh_error) = refresh_error {
                self.set_slack_schedule_owner_error(&owner, &refresh_error);
            }
            self.focus_slack_schedule_owner(&owner);
        }
        self.sync_slack_remote_draft_hydration(cx);
        cx.notify();
    }

    pub(super) fn slack_schedule_pending_origin_is_current(
        &self,
        pending: &SlackSchedulePendingSubmission,
    ) -> bool {
        match &pending.origin {
            SlackScheduledPendingOrigin::Composer => {
                self.slack_schedule_owner_surface_is_current(&pending.owner)
            }
            SlackScheduledPendingOrigin::ScheduledEdit {
                prior_draft_handle, ..
            } => {
                self.current_slack_main_composer_draft_handle().as_ref() == Some(prior_draft_handle)
            }
        }
    }
}
