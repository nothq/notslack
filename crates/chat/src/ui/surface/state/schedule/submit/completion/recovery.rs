use super::super::super::super::{Context, SurfaceState};
use super::super::super::{slack_remote_loading_file_locators, SlackScheduleDraftRestore};
use super::super::execution::append_refresh_error;
use crate::ui::surface::{
    SlackComposerScheduleRecovery, SlackSchedulePendingSubmission, SlackScheduledEditRecovery,
    SlackScheduledPendingOrigin,
};

impl SurfaceState {
    pub(super) fn restore_slack_schedule_owner_after_failure(
        &mut self,
        pending: SlackSchedulePendingSubmission,
        diagnostic: String,
        refresh_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let owner = pending.owner.clone();
        let origin_is_current = self.slack_schedule_pending_origin_is_current(&pending);
        let failure_diagnostic = append_refresh_error(diagnostic, refresh_error);
        let restored_active = if matches!(&pending.origin, SlackScheduledPendingOrigin::Composer) {
            self.restore_slack_composer_schedule_owner_after_failure(
                pending,
                &failure_diagnostic,
                cx,
            )
        } else {
            self.restore_slack_scheduled_edit_owner_after_failure(
                pending,
                origin_is_current,
                &failure_diagnostic,
                cx,
            )
        };
        self.finish_slack_schedule_failure_recovery(owner, failure_diagnostic, restored_active, cx);
    }

    fn restore_slack_composer_schedule_owner_after_failure(
        &mut self,
        pending: SlackSchedulePendingSubmission,
        failure_diagnostic: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let SlackSchedulePendingSubmission {
            identity,
            owner,
            accepted_draft,
            origin,
            file_ids,
            workspace_api,
            ..
        } = pending;
        assert!(
            matches!(origin, SlackScheduledPendingOrigin::Composer),
            "composer schedule recovery requires its composer origin"
        );
        let accepted_token = accepted_draft.token;
        let remote_loads = slack_remote_loading_file_locators(&owner, &accepted_draft);
        let restored = self.restore_slack_schedule_draft(&owner, accepted_draft);
        let restored_active = matches!(restored, Ok(SlackScheduleDraftRestore::Active));
        match restored {
            Ok(_) => {
                if let Some(key) = owner.draft_key().cloned() {
                    self.release_slack_draft_schedule_claim(
                        key,
                        identity.generation,
                        accepted_token,
                        cx,
                    );
                }
            }
            Err(draft) => {
                self.slack_composer_schedule_recovery = Some(SlackComposerScheduleRecovery {
                    identity,
                    owner: owner.clone(),
                    draft: *draft,
                    file_ids,
                    workspace_api: workspace_api.clone(),
                    failure_diagnostic: failure_diagnostic.to_string(),
                });
            }
        }
        self.enqueue_slack_remote_draft_file_loads_for_owned_schedule_draft(
            &owner,
            remote_loads,
            workspace_api,
            cx,
        );
        restored_active
    }

    fn restore_slack_scheduled_edit_owner_after_failure(
        &mut self,
        pending: SlackSchedulePendingSubmission,
        origin_is_current: bool,
        failure_diagnostic: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let SlackSchedulePendingSubmission {
            owner,
            accepted_draft,
            origin,
            ..
        } = pending;
        let SlackScheduledPendingOrigin::ScheduledEdit {
            edit,
            deferred_remote_removals,
            ..
        } = origin
        else {
            unreachable!("scheduled-edit recovery requires its scheduled-edit origin");
        };
        let can_restore_active = origin_is_current && self.slack_active_scheduled_edit.is_none();
        if can_restore_active {
            self.restore_failed_slack_scheduled_edit(
                *edit,
                accepted_draft,
                deferred_remote_removals,
                cx,
            );
        } else {
            let source = owner
                .main_source()
                .expect("scheduled-edit recovery must retain its main source")
                .clone();
            self.slack_scheduled_edit_recovery = Some(SlackScheduledEditRecovery {
                edit: *edit,
                source,
                edit_draft: accepted_draft,
                deferred_remote_removals,
                failure_diagnostic: failure_diagnostic.to_string(),
            });
        }
        can_restore_active
    }

    fn finish_slack_schedule_failure_recovery(
        &mut self,
        owner: crate::ui::surface::SlackScheduleDraftOwner,
        failure_diagnostic: String,
        restored_active: bool,
        cx: &mut Context<Self>,
    ) {
        let diagnostic = self
            .slack_composer_schedule_recovery_diagnostic()
            .unwrap_or(failure_diagnostic);
        self.slack_schedule_submission_error = Some(diagnostic.clone());
        if restored_active {
            self.set_slack_schedule_owner_error(&owner, &diagnostic);
            self.focus_slack_schedule_owner(&owner);
        }
        self.sync_slack_remote_draft_hydration(cx);
        cx.notify();
    }
}
