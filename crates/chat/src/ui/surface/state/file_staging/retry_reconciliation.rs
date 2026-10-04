mod transitions;

use std::sync::Arc;

use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingCleanupOutcome,
    SlackFileStagingReconcileOutcome,
};

use super::receipts::{
    retained_slack_file_ownership_from_cleanup, validate_slack_reconciliation_receipt,
};
use super::{Context, SlackFileRetryContinuation, SlackVisibleFileApplication, SurfaceState};
use crate::ui::surface::{
    SlackComposerFile, SlackComposerFileRecovery, SlackComposerFileState,
    SlackFileRemovalTombstone, SlackFileRemovalTombstoneState, SlackFileStagingLocator,
    SlackFileStagingRetryRequest,
};
use crate::ui::WorkspaceApi;
use transitions::{apply_slack_retry_cleanup_outcome, apply_slack_retry_reconciliation_outcome};

impl SurfaceState {
    pub(super) fn slack_file_operation_api(
        &self,
        locator: &SlackFileStagingLocator,
    ) -> Result<Arc<dyn WorkspaceApi>, String> {
        self.slack_file_staging
            .operation_apis
            .get(&locator.operation_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Slack file-staging operation {} no longer has its owning workspace runtime.",
                    locator.operation_id
                )
            })
    }

    pub(super) fn spawn_slack_file_retry_reconciliation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackFileStagingRetryRequest)| {
                let outcome =
                    workspace_api.reconcile_slack_file_staging(&request.locator.operation_id);
                (workspace_api, request, outcome)
            },
            |this, (workspace_api, request, outcome), cx| {
                this.apply_slack_file_retry_reconciliation(workspace_api, request, outcome, cx);
            },
        );
    }

    fn apply_slack_file_retry_reconciliation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        outcome: SlackFileStagingReconcileOutcome,
        cx: &mut Context<Self>,
    ) {
        let outcome = validate_slack_reconciliation_receipt(&request.locator, outcome);
        match self.apply_visible_slack_file_retry_reconciliation(&request, outcome) {
            SlackVisibleFileApplication::Applied(continuation) => {
                self.slack_staged_file_became_autosave_ready(&request.locator, cx);
                self.continue_slack_file_retry(workspace_api, request, continuation, cx);
            }
            SlackVisibleFileApplication::Removed(outcome) => {
                self.apply_removed_slack_file_retry_reconciliation(
                    workspace_api,
                    request,
                    outcome,
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn apply_visible_slack_file_retry_reconciliation(
        &mut self,
        request: &SlackFileStagingRetryRequest,
        outcome: SlackFileStagingReconcileOutcome,
    ) -> SlackVisibleFileApplication<SlackFileRetryContinuation, SlackFileStagingReconcileOutcome>
    {
        let Some(file) = self.slack_file_for_staging_mut(&request.locator) else {
            return SlackVisibleFileApplication::Removed(outcome);
        };
        let Some(upload) =
            slack_retry_upload(file, request, SlackFileRetryRecoveryKind::Reconciliation)
        else {
            return SlackVisibleFileApplication::Removed(outcome);
        };
        let continuation = apply_slack_retry_reconciliation_outcome(file, request, upload, outcome);
        SlackVisibleFileApplication::Applied(continuation)
    }

    fn continue_slack_file_retry(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        continuation: SlackFileRetryContinuation,
        cx: &mut Context<Self>,
    ) {
        match continuation {
            SlackFileRetryContinuation::None => {}
            SlackFileRetryContinuation::Queue(locator) => {
                self.slack_file_staging
                    .operation_apis
                    .remove(&request.locator.operation_id);
                self.slack_file_staging.enqueue(locator, workspace_api);
                self.pump_slack_file_staging(cx);
            }
            SlackFileRetryContinuation::Cleanup => {
                self.spawn_slack_file_retry_cleanup(workspace_api, request, cx);
            }
            SlackFileRetryContinuation::AlreadyShared
            | SlackFileRetryContinuation::AlreadyDraftOwned => {
                self.slack_file_staging
                    .operation_apis
                    .remove(&request.locator.operation_id);
            }
        }
    }

    fn apply_removed_slack_file_retry_reconciliation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        outcome: SlackFileStagingReconcileOutcome,
        cx: &mut Context<Self>,
    ) {
        self.take_waiting_slack_file_recovery_tombstone(&request.locator);
        if matches!(
            outcome,
            SlackFileStagingReconcileOutcome::Cancelled(
                SlackFileStagingCancellationOutcome::NoPersistentFile
                    | SlackFileStagingCancellationOutcome::ConfirmedDeleted
                    | SlackFileStagingCancellationOutcome::Shared
                    | SlackFileStagingCancellationOutcome::DraftOwned
            )
        ) {
            self.slack_file_staging
                .operation_apis
                .remove(&request.locator.operation_id);
            return;
        }
        self.start_slack_file_removal_cleanup(workspace_api, request.locator, cx);
    }

    pub(super) fn spawn_slack_file_retry_cleanup(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackFileStagingRetryRequest)| {
                let outcome =
                    workspace_api.cleanup_slack_staged_file(&request.locator.operation_id);
                (workspace_api, request, outcome)
            },
            |this, (workspace_api, request, outcome), cx| {
                this.apply_slack_file_retry_cleanup(workspace_api, request, outcome, cx);
            },
        );
    }

    fn apply_slack_file_retry_cleanup(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackFileStagingRetryRequest,
        outcome: SlackFileStagingCleanupOutcome,
        cx: &mut Context<Self>,
    ) {
        match self.apply_visible_slack_file_retry_cleanup(&request, outcome) {
            SlackVisibleFileApplication::Applied(continuation) => {
                self.continue_slack_file_retry(workspace_api, request, continuation, cx);
            }
            SlackVisibleFileApplication::Removed(outcome) => {
                self.take_waiting_slack_file_recovery_tombstone(&request.locator);
                self.apply_removed_slack_file_retry_cleanup(request.locator, outcome);
            }
        }
        cx.notify();
    }

    fn apply_visible_slack_file_retry_cleanup(
        &mut self,
        request: &SlackFileStagingRetryRequest,
        outcome: SlackFileStagingCleanupOutcome,
    ) -> SlackVisibleFileApplication<SlackFileRetryContinuation, SlackFileStagingCleanupOutcome>
    {
        let Some(file) = self.slack_file_for_staging_mut(&request.locator) else {
            return SlackVisibleFileApplication::Removed(outcome);
        };
        let Some(upload) = slack_retry_upload(file, request, SlackFileRetryRecoveryKind::Cleanup)
        else {
            return SlackVisibleFileApplication::Removed(outcome);
        };
        let continuation = apply_slack_retry_cleanup_outcome(file, request, upload, outcome);
        SlackVisibleFileApplication::Applied(continuation)
    }

    fn take_waiting_slack_file_recovery_tombstone(
        &mut self,
        locator: &SlackFileStagingLocator,
    ) -> bool {
        let Some(tombstone) = self
            .slack_file_staging
            .removals
            .remove(&locator.operation_id)
        else {
            return false;
        };
        if tombstone.locator == *locator
            && matches!(
                tombstone.state,
                SlackFileRemovalTombstoneState::WaitingForRecovery
            )
        {
            return true;
        }
        self.slack_file_staging
            .removals
            .insert(tombstone.locator.operation_id.clone(), tombstone);
        false
    }

    fn apply_removed_slack_file_retry_cleanup(
        &mut self,
        locator: SlackFileStagingLocator,
        outcome: SlackFileStagingCleanupOutcome,
    ) {
        let retained = match outcome {
            SlackFileStagingCleanupOutcome::NoPersistentFile
            | SlackFileStagingCleanupOutcome::ConfirmedDeleted
            | SlackFileStagingCleanupOutcome::Shared
            | SlackFileStagingCleanupOutcome::DraftOwned => None,
            outcome => Some(retained_slack_file_ownership_from_cleanup(
                &locator.operation_id,
                outcome,
            )),
        };
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
    }
}

enum SlackFileRetryRecoveryKind {
    Reconciliation,
    Cleanup,
}

fn slack_retry_upload(
    file: &SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    kind: SlackFileRetryRecoveryKind,
) -> Option<crate::ui::SlackUploadFile> {
    let SlackComposerFileState::Recovering { upload, recovery } = file.state() else {
        return None;
    };
    let (prior_operation_id, next_operation_id) = match (kind, recovery) {
        (
            SlackFileRetryRecoveryKind::Reconciliation,
            SlackComposerFileRecovery::ReconcilingForRetry {
                prior_operation_id,
                next_operation_id,
            },
        )
        | (
            SlackFileRetryRecoveryKind::Cleanup,
            SlackComposerFileRecovery::CleaningForRetry {
                prior_operation_id,
                next_operation_id,
            },
        ) => (prior_operation_id, next_operation_id),
        _ => return None,
    };
    (prior_operation_id == &request.locator.operation_id
        && next_operation_id == &request.next_operation_id)
        .then(|| upload.clone())
}
