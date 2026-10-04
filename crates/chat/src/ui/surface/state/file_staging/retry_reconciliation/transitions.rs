use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingCleanupOutcome,
    SlackFileStagingReconcileOutcome,
};

use super::super::{
    receipts::retained_slack_file_ownership_from_cleanup, SlackFileRetryContinuation,
};
use crate::ui::surface::{
    SlackComposerFile, SlackComposerFileRecovery, SlackComposerFileState, SlackFileStagingLocator,
    SlackFileStagingRetryRequest, SlackRetainedFileOwnership,
};

pub(super) fn apply_slack_retry_reconciliation_outcome(
    file: &mut SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    upload: crate::ui::SlackUploadFile,
    outcome: SlackFileStagingReconcileOutcome,
) -> SlackFileRetryContinuation {
    match outcome {
        SlackFileStagingReconcileOutcome::Staged(staged) => {
            *file.state_mut() = SlackComposerFileState::Staged { staged };
            SlackFileRetryContinuation::None
        }
        SlackFileStagingReconcileOutcome::StillUnknown { .. } => {
            set_slack_retry_cleanup_state(file, request, upload);
            SlackFileRetryContinuation::Cleanup
        }
        SlackFileStagingReconcileOutcome::Cancelled(outcome) => {
            apply_slack_retry_cancellation(file, request, upload, outcome)
        }
        SlackFileStagingReconcileOutcome::Failed(failure) => {
            *file.state_mut() = SlackComposerFileState::RetryBlocked {
                prior_operation_id: request.locator.operation_id.clone(),
                pending_retry_operation_id: request.next_operation_id.clone(),
                upload,
                ownership: SlackRetainedFileOwnership::ReconciliationFailed(failure),
            };
            SlackFileRetryContinuation::None
        }
    }
}

fn apply_slack_retry_cancellation(
    file: &mut SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    upload: crate::ui::SlackUploadFile,
    outcome: SlackFileStagingCancellationOutcome,
) -> SlackFileRetryContinuation {
    match outcome {
        SlackFileStagingCancellationOutcome::NoPersistentFile
        | SlackFileStagingCancellationOutcome::ConfirmedDeleted => {
            queue_slack_file_retry(file, request, upload)
        }
        SlackFileStagingCancellationOutcome::Shared => {
            *file.state_mut() = SlackComposerFileState::AlreadyShared {
                operation_id: request.locator.operation_id.clone(),
            };
            SlackFileRetryContinuation::AlreadyShared
        }
        SlackFileStagingCancellationOutcome::DraftOwned => {
            *file.state_mut() = SlackComposerFileState::AlreadyDraftOwned {
                operation_id: request.locator.operation_id.clone(),
            };
            SlackFileRetryContinuation::AlreadyDraftOwned
        }
        SlackFileStagingCancellationOutcome::Pending
        | SlackFileStagingCancellationOutcome::RetainedUnknown { .. }
        | SlackFileStagingCancellationOutcome::Failed(_) => {
            set_slack_retry_cleanup_state(file, request, upload);
            SlackFileRetryContinuation::Cleanup
        }
    }
}

pub(super) fn apply_slack_retry_cleanup_outcome(
    file: &mut SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    upload: crate::ui::SlackUploadFile,
    outcome: SlackFileStagingCleanupOutcome,
) -> SlackFileRetryContinuation {
    match outcome {
        SlackFileStagingCleanupOutcome::NoPersistentFile
        | SlackFileStagingCleanupOutcome::ConfirmedDeleted => {
            queue_slack_file_retry(file, request, upload)
        }
        SlackFileStagingCleanupOutcome::Shared => {
            *file.state_mut() = SlackComposerFileState::AlreadyShared {
                operation_id: request.locator.operation_id.clone(),
            };
            SlackFileRetryContinuation::AlreadyShared
        }
        SlackFileStagingCleanupOutcome::DraftOwned => {
            *file.state_mut() = SlackComposerFileState::AlreadyDraftOwned {
                operation_id: request.locator.operation_id.clone(),
            };
            SlackFileRetryContinuation::AlreadyDraftOwned
        }
        outcome => {
            *file.state_mut() = SlackComposerFileState::RetryBlocked {
                prior_operation_id: request.locator.operation_id.clone(),
                pending_retry_operation_id: request.next_operation_id.clone(),
                upload,
                ownership: retained_slack_file_ownership_from_cleanup(
                    &request.locator.operation_id,
                    outcome,
                ),
            };
            SlackFileRetryContinuation::None
        }
    }
}

fn queue_slack_file_retry(
    file: &mut SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    upload: crate::ui::SlackUploadFile,
) -> SlackFileRetryContinuation {
    *file.state_mut() = SlackComposerFileState::Queued {
        operation_id: request.next_operation_id.clone(),
        upload,
    };
    SlackFileRetryContinuation::Queue(SlackFileStagingLocator {
        operation_id: request.next_operation_id.clone(),
        ..request.locator.clone()
    })
}

fn set_slack_retry_cleanup_state(
    file: &mut SlackComposerFile,
    request: &SlackFileStagingRetryRequest,
    upload: crate::ui::SlackUploadFile,
) {
    *file.state_mut() = SlackComposerFileState::Recovering {
        upload,
        recovery: SlackComposerFileRecovery::CleaningForRetry {
            prior_operation_id: request.locator.operation_id.clone(),
            next_operation_id: request.next_operation_id.clone(),
        },
    };
}
