use std::sync::Arc;

use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingDiagnostic, SlackFileStagingOutcome,
};

use super::receipts::validate_slack_staging_receipt;
use super::{Context, SlackRemovedActiveFileOutcomes, SlackVisibleFileApplication, SurfaceState};
use crate::ui::surface::{
    SlackActiveFileStaging, SlackComposerFileState, SlackFileRemovalTombstone,
    SlackFileRemovalTombstoneState, SlackFileStagingLocator,
};
use crate::ui::WorkspaceApi;

impl SurfaceState {
    pub(super) fn pump_slack_file_staging(&mut self, cx: &mut Context<Self>) {
        while self.slack_file_staging.has_stage_capacity() {
            let Some(queued) = self.slack_file_staging.queued.pop_front() else {
                break;
            };
            let locator = queued.locator;
            let workspace_api = queued.workspace_api;
            let Some(upload) = self.mark_slack_file_staging_uploading(&locator) else {
                self.slack_file_staging
                    .operation_apis
                    .remove(&locator.operation_id);
                continue;
            };
            self.slack_file_staging.active.insert(
                locator.operation_id.clone(),
                SlackActiveFileStaging {
                    locator: locator.clone(),
                    workspace_api: workspace_api.clone(),
                },
            );
            self.spawn_background_task(
                (workspace_api, locator, upload),
                cx,
                |(workspace_api, locator, upload)| {
                    let outcome = workspace_api.stage_slack_file(&locator.operation_id, upload);
                    (locator, outcome)
                },
                |this, (locator, outcome), cx| {
                    this.apply_slack_file_staging_result(locator, outcome, cx);
                },
            );
        }
    }

    fn mark_slack_file_staging_uploading(
        &mut self,
        locator: &SlackFileStagingLocator,
    ) -> Option<crate::ui::SlackUploadFile> {
        let file = self.slack_file_for_staging_mut(locator)?;
        let upload = match file.state() {
            SlackComposerFileState::Queued {
                operation_id,
                upload,
            } if operation_id == &locator.operation_id => upload.clone(),
            _ => return None,
        };
        *file.state_mut() = SlackComposerFileState::Uploading {
            operation_id: locator.operation_id.clone(),
            upload: upload.clone(),
        };
        Some(upload)
    }

    pub(super) fn fail_queued_slack_file_staging(
        &mut self,
        locator: &SlackFileStagingLocator,
        diagnostic: SlackFileStagingDiagnostic,
    ) {
        let Some(file) = self.slack_file_for_staging_mut(locator) else {
            return;
        };
        let upload = match file.state() {
            SlackComposerFileState::Queued {
                operation_id,
                upload,
            } if operation_id == &locator.operation_id => upload.clone(),
            _ => return,
        };
        *file.state_mut() = SlackComposerFileState::Error {
            operation_id: locator.operation_id.clone(),
            upload,
            failure: crate::ui::surface::SlackComposerFileFailure::NotSubmitted(diagnostic),
        };
    }

    fn apply_slack_file_staging_result(
        &mut self,
        locator: SlackFileStagingLocator,
        outcome: SlackFileStagingOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(active) = self.slack_file_staging.active.remove(&locator.operation_id) else {
            return;
        };
        if active.locator != locator {
            self.slack_file_staging
                .active
                .insert(active.locator.operation_id.clone(), active);
            return;
        }
        let outcome = validate_slack_staging_receipt(&locator, outcome);
        let outcome = match self.apply_visible_slack_file_staging_result(&locator, outcome) {
            SlackVisibleFileApplication::Applied(ownership_terminal) => {
                self.slack_staged_file_became_autosave_ready(&locator, cx);
                if ownership_terminal {
                    self.slack_file_staging
                        .operation_apis
                        .remove(&locator.operation_id);
                }
                self.pump_slack_file_staging(cx);
                cx.notify();
                return;
            }
            SlackVisibleFileApplication::Removed(outcome) => outcome,
        };
        self.apply_removed_slack_file_staging_result(active.workspace_api, locator, outcome, cx);
        self.pump_slack_file_staging(cx);
        cx.notify();
    }

    fn apply_visible_slack_file_staging_result(
        &mut self,
        locator: &SlackFileStagingLocator,
        outcome: SlackFileStagingOutcome,
    ) -> SlackVisibleFileApplication<bool, SlackFileStagingOutcome> {
        let Some(file) = self.slack_file_for_staging_mut(locator) else {
            return SlackVisibleFileApplication::Removed(outcome);
        };
        let upload = match file.state() {
            SlackComposerFileState::Uploading {
                operation_id,
                upload,
            } if operation_id == &locator.operation_id => upload.clone(),
            _ => return SlackVisibleFileApplication::Removed(outcome),
        };
        let ownership_terminal = matches!(
            outcome,
            SlackFileStagingOutcome::Cancelled(
                SlackFileStagingCancellationOutcome::Shared
                    | SlackFileStagingCancellationOutcome::DraftOwned
            )
        );
        *file.state_mut() = SlackComposerFileState::from_staging_outcome(
            locator.operation_id.clone(),
            upload,
            outcome,
        );
        SlackVisibleFileApplication::Applied(ownership_terminal)
    }

    fn apply_removed_slack_file_staging_result(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        locator: SlackFileStagingLocator,
        staging: SlackFileStagingOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(tombstone) = self
            .slack_file_staging
            .removals
            .remove(&locator.operation_id)
        else {
            self.start_slack_file_removal_cleanup(workspace_api, locator, cx);
            return;
        };
        if tombstone.locator != locator {
            self.slack_file_staging
                .removals
                .insert(tombstone.locator.operation_id.clone(), tombstone);
            self.start_slack_file_removal_cleanup(workspace_api, locator, cx);
            return;
        }
        match tombstone.state {
            SlackFileRemovalTombstoneState::WaitingForStageAndCancel => {
                self.slack_file_staging.removals.insert(
                    locator.operation_id.clone(),
                    SlackFileRemovalTombstone {
                        locator,
                        state: SlackFileRemovalTombstoneState::WaitingForCancel { staging },
                    },
                );
            }
            SlackFileRemovalTombstoneState::WaitingForStage { cancellation } => {
                self.finish_removed_active_slack_file(
                    workspace_api,
                    locator,
                    SlackRemovedActiveFileOutcomes {
                        staging,
                        cancellation,
                    },
                    cx,
                );
            }
            state => {
                self.slack_file_staging.removals.insert(
                    locator.operation_id.clone(),
                    SlackFileRemovalTombstone { locator, state },
                );
            }
        }
    }
}
