use std::sync::Arc;

use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingCleanupOutcome,
    SlackFileStagingDiagnostic, SlackFileStagingOperationFailure, SlackFileStagingOutcome,
};

use super::{Context, SlackRemovedActiveFileOutcomes, SurfaceState};
use crate::ui::surface::{
    SlackFileRemovalTombstone, SlackFileRemovalTombstoneState, SlackFileStagingLocator,
    SlackRetainedFileOwnership,
};
use crate::ui::WorkspaceApi;

impl SurfaceState {
    pub(super) fn spawn_slack_file_staging_cancellation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        locator: SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (workspace_api, locator),
            cx,
            |(workspace_api, locator): (Arc<dyn WorkspaceApi>, SlackFileStagingLocator)| {
                let outcome = workspace_api.cancel_slack_file_staging(&locator.operation_id);
                (workspace_api, locator, outcome)
            },
            |this, (workspace_api, locator, outcome), cx| {
                this.apply_slack_file_staging_cancellation(workspace_api, locator, outcome, cx);
            },
        );
    }

    fn apply_slack_file_staging_cancellation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        locator: SlackFileStagingLocator,
        cancellation: SlackFileStagingCancellationOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(tombstone) = self
            .slack_file_staging
            .removals
            .remove(&locator.operation_id)
        else {
            return;
        };
        if tombstone.locator != locator {
            self.slack_file_staging
                .removals
                .insert(tombstone.locator.operation_id.clone(), tombstone);
            return;
        }
        match tombstone.state {
            SlackFileRemovalTombstoneState::WaitingForStageAndCancel => {
                self.slack_file_staging.removals.insert(
                    locator.operation_id.clone(),
                    SlackFileRemovalTombstone {
                        locator,
                        state: SlackFileRemovalTombstoneState::WaitingForStage { cancellation },
                    },
                );
            }
            SlackFileRemovalTombstoneState::WaitingForCancel { staging } => {
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
        cx.notify();
    }

    pub(super) fn finish_removed_active_slack_file(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        locator: SlackFileStagingLocator,
        outcomes: SlackRemovedActiveFileOutcomes,
        cx: &mut Context<Self>,
    ) {
        if slack_file_removal_is_terminal(&outcomes.staging, &outcomes.cancellation) {
            self.slack_file_staging
                .operation_apis
                .remove(&locator.operation_id);
            return;
        }
        self.start_slack_file_removal_cleanup(workspace_api, locator, cx);
    }

    pub(super) fn begin_slack_file_removal_cleanup(
        &mut self,
        locator: SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self
            .slack_file_staging
            .operation_apis
            .get(&locator.operation_id)
            .cloned()
        else {
            self.retain_unavailable_slack_file_cleanup(locator);
            cx.notify();
            return;
        };
        self.start_slack_file_removal_cleanup(workspace_api, locator, cx);
    }

    pub(super) fn start_slack_file_removal_cleanup(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        locator: SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) {
        self.slack_file_staging.removals.insert(
            locator.operation_id.clone(),
            SlackFileRemovalTombstone {
                locator: locator.clone(),
                state: SlackFileRemovalTombstoneState::Cleaning,
            },
        );
        self.spawn_background_task(
            (workspace_api, locator),
            cx,
            |(workspace_api, locator): (Arc<dyn WorkspaceApi>, SlackFileStagingLocator)| {
                let outcome = workspace_api.cleanup_slack_staged_file(&locator.operation_id);
                (locator, outcome)
            },
            |this, (locator, outcome), cx| {
                this.apply_slack_file_removal_cleanup(locator, outcome, cx);
            },
        );
    }

    fn apply_slack_file_removal_cleanup(
        &mut self,
        locator: SlackFileStagingLocator,
        outcome: SlackFileStagingCleanupOutcome,
        cx: &mut Context<Self>,
    ) {
        let Some(tombstone) = self
            .slack_file_staging
            .removals
            .remove(&locator.operation_id)
        else {
            return;
        };
        if tombstone.locator != locator
            || !matches!(tombstone.state, SlackFileRemovalTombstoneState::Cleaning)
        {
            self.slack_file_staging
                .removals
                .insert(tombstone.locator.operation_id.clone(), tombstone);
            return;
        }
        let retained = retained_slack_file_ownership(outcome);
        if let Some(ownership) = retained {
            self.slack_file_staging.removals.insert(
                locator.operation_id.clone(),
                SlackFileRemovalTombstone {
                    locator,
                    state: SlackFileRemovalTombstoneState::RetainedUnknown(ownership),
                },
            );
        } else {
            self.slack_file_staging
                .operation_apis
                .remove(&locator.operation_id);
        }
        cx.notify();
    }

    pub(super) fn retain_unavailable_slack_file_cleanup(
        &mut self,
        locator: SlackFileStagingLocator,
    ) {
        self.slack_file_staging.removals.insert(
            locator.operation_id.clone(),
            SlackFileRemovalTombstone {
                locator,
                state: SlackFileRemovalTombstoneState::RetainedUnknown(
                    SlackRetainedFileOwnership::CleanupFailed(
                        SlackFileStagingOperationFailure::Unavailable {
                            diagnostic: SlackFileStagingDiagnostic::new(
                                "Slack file cleanup requires a connected workspace.".to_string(),
                            ),
                        },
                    ),
                ),
            },
        );
    }
}

fn retained_slack_file_ownership(
    outcome: SlackFileStagingCleanupOutcome,
) -> Option<SlackRetainedFileOwnership> {
    match outcome {
        SlackFileStagingCleanupOutcome::NoPersistentFile
        | SlackFileStagingCleanupOutcome::ConfirmedDeleted
        | SlackFileStagingCleanupOutcome::Shared
        | SlackFileStagingCleanupOutcome::DraftOwned => None,
        SlackFileStagingCleanupOutcome::Pending => Some(SlackRetainedFileOwnership::CleanupPending),
        SlackFileStagingCleanupOutcome::RetainedUnknown {
            allocated_attempts,
            diagnostic,
        } => Some(SlackRetainedFileOwnership::CleanupUnknown {
            allocated_attempts,
            diagnostic,
        }),
        SlackFileStagingCleanupOutcome::Failed(failure) => {
            Some(SlackRetainedFileOwnership::CleanupFailed(failure))
        }
    }
}

fn slack_file_removal_is_terminal(
    staging: &SlackFileStagingOutcome,
    cancellation: &SlackFileStagingCancellationOutcome,
) -> bool {
    matches!(
        staging,
        SlackFileStagingOutcome::Cancelled(
            SlackFileStagingCancellationOutcome::NoPersistentFile
                | SlackFileStagingCancellationOutcome::ConfirmedDeleted
                | SlackFileStagingCancellationOutcome::Shared
                | SlackFileStagingCancellationOutcome::DraftOwned
        )
    ) || matches!(
        cancellation,
        SlackFileStagingCancellationOutcome::NoPersistentFile
            | SlackFileStagingCancellationOutcome::ConfirmedDeleted
            | SlackFileStagingCancellationOutcome::Shared
            | SlackFileStagingCancellationOutcome::DraftOwned
    )
}
