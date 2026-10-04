use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingCleanupOutcome,
    SlackFileStagingDiagnostic, SlackFileStagingFailure, SlackFileStagingOperationFailure,
    SlackFileStagingOperationId, SlackFileStagingOutcome, SlackFileStagingReconcileOutcome,
};

use super::SlackFileRetryStart;
use crate::ui::surface::{
    SlackComposerFileFailure, SlackComposerFileState, SlackFileRetryRecoveryKind,
    SlackFileStagingLocator, SlackRetainedFileOwnership,
};

pub(super) fn validate_slack_staging_receipt(
    locator: &SlackFileStagingLocator,
    outcome: SlackFileStagingOutcome,
) -> SlackFileStagingOutcome {
    let SlackFileStagingOutcome::Staged(staged) = outcome else {
        return outcome;
    };
    if staged.operation_id() == &locator.operation_id {
        return SlackFileStagingOutcome::Staged(staged);
    }
    SlackFileStagingOutcome::Failed(SlackFileStagingFailure::Operation(
        SlackFileStagingOperationFailure::InvalidState {
            operation_id: locator.operation_id.clone(),
            diagnostic: SlackFileStagingDiagnostic::new(
                "Slack file staging returned a receipt for another operation.".to_string(),
            ),
        },
    ))
}

pub(super) fn validate_slack_reconciliation_receipt(
    locator: &SlackFileStagingLocator,
    outcome: SlackFileStagingReconcileOutcome,
) -> SlackFileStagingReconcileOutcome {
    let SlackFileStagingReconcileOutcome::Staged(staged) = outcome else {
        return outcome;
    };
    if staged.operation_id() == &locator.operation_id {
        return SlackFileStagingReconcileOutcome::Staged(staged);
    }
    SlackFileStagingReconcileOutcome::Failed(SlackFileStagingOperationFailure::InvalidState {
        operation_id: locator.operation_id.clone(),
        diagnostic: SlackFileStagingDiagnostic::new(
            "Slack file reconciliation returned a receipt for another operation.".to_string(),
        ),
    })
}

pub(super) fn slack_file_retry_start(
    state: &SlackComposerFileState,
) -> Result<SlackFileRetryStart, String> {
    match state {
        SlackComposerFileState::Error {
            upload, failure, ..
        } => slack_file_error_retry_start(upload, failure),
        SlackComposerFileState::RetainedUnknown {
            upload, ownership, ..
        }
        | SlackComposerFileState::RetryBlocked {
            upload, ownership, ..
        } => match ownership.retry_recovery_kind() {
            SlackFileRetryRecoveryKind::Reconcile => {
                Ok(SlackFileRetryStart::Reconcile(upload.clone()))
            }
            SlackFileRetryRecoveryKind::Cleanup => Ok(SlackFileRetryStart::Cleanup(upload.clone())),
        },
        SlackComposerFileState::AlreadyShared { .. } => {
            Err("Slack reports that this staged file has already been shared.".to_string())
        }
        SlackComposerFileState::AlreadyDraftOwned { .. } => {
            Err("Slack reports that this staged file is already owned by a draft.".to_string())
        }
        SlackComposerFileState::Staged { .. } => {
            Err("This Slack composer file has already completed staging.".to_string())
        }
        SlackComposerFileState::Queued { .. } => {
            Err("This Slack composer file is already queued for staging.".to_string())
        }
        SlackComposerFileState::Uploading { .. } => {
            Err("This Slack composer file is already uploading.".to_string())
        }
        SlackComposerFileState::Recovering { .. } => {
            Err("This Slack composer file retry is already in progress.".to_string())
        }
        SlackComposerFileState::RemoteLoading { .. }
        | SlackComposerFileState::RemoteReady { .. }
        | SlackComposerFileState::RemoteError { .. } => {
            Err("Remote Slack draft files use metadata retry semantics.".to_string())
        }
        #[cfg(test)]
        SlackComposerFileState::Fixture => {
            Err("Fixture attachments do not support Slack file staging.".to_string())
        }
    }
}

fn slack_file_error_retry_start(
    upload: &crate::ui::SlackUploadFile,
    failure: &SlackComposerFileFailure,
) -> Result<SlackFileRetryStart, String> {
    match failure {
        SlackComposerFileFailure::NotSubmitted(_)
        | SlackComposerFileFailure::Staging(
            SlackFileStagingFailure::InvalidUpload { .. }
            | SlackFileStagingFailure::RetryExhaustedBeforeAllocation { .. },
        )
        | SlackComposerFileFailure::Cancelled(
            SlackFileStagingCancellationOutcome::NoPersistentFile
            | SlackFileStagingCancellationOutcome::ConfirmedDeleted,
        ) => Ok(SlackFileRetryStart::Queue(upload.clone())),
        SlackComposerFileFailure::Staging(SlackFileStagingFailure::CompletionUnknown {
            ..
        }) => Ok(SlackFileRetryStart::Reconcile(upload.clone())),
        SlackComposerFileFailure::Staging(
            SlackFileStagingFailure::RetryExhaustedBeforeCompletion { .. }
            | SlackFileStagingFailure::CompletionNotSent { .. }
            | SlackFileStagingFailure::CompletionRejected { .. }
            | SlackFileStagingFailure::Operation(_),
        )
        | SlackComposerFileFailure::Cancelled(
            SlackFileStagingCancellationOutcome::Pending
            | SlackFileStagingCancellationOutcome::RetainedUnknown { .. }
            | SlackFileStagingCancellationOutcome::Failed(_),
        ) => Ok(SlackFileRetryStart::Cleanup(upload.clone())),
        SlackComposerFileFailure::Cancelled(SlackFileStagingCancellationOutcome::Shared) => {
            Err("Slack reports that this staged file has already been shared.".to_string())
        }
        SlackComposerFileFailure::Cancelled(SlackFileStagingCancellationOutcome::DraftOwned) => {
            Err("Slack reports that this staged file is already owned by a draft.".to_string())
        }
    }
}

pub(super) fn retained_slack_file_ownership_from_cleanup(
    operation_id: &SlackFileStagingOperationId,
    outcome: SlackFileStagingCleanupOutcome,
) -> SlackRetainedFileOwnership {
    match outcome {
        SlackFileStagingCleanupOutcome::Pending => SlackRetainedFileOwnership::CleanupPending,
        SlackFileStagingCleanupOutcome::RetainedUnknown {
            allocated_attempts,
            diagnostic,
        } => SlackRetainedFileOwnership::CleanupUnknown {
            allocated_attempts,
            diagnostic,
        },
        SlackFileStagingCleanupOutcome::Failed(failure) => {
            SlackRetainedFileOwnership::CleanupFailed(failure)
        }
        SlackFileStagingCleanupOutcome::NoPersistentFile
        | SlackFileStagingCleanupOutcome::ConfirmedDeleted
        | SlackFileStagingCleanupOutcome::Shared
        | SlackFileStagingCleanupOutcome::DraftOwned => SlackRetainedFileOwnership::CleanupFailed(
            SlackFileStagingOperationFailure::InvalidState {
                operation_id: operation_id.clone(),
                diagnostic: SlackFileStagingDiagnostic::new(
                    "Slack file cleanup reached an invalid retry transition.".to_string(),
                ),
            },
        ),
    }
}
