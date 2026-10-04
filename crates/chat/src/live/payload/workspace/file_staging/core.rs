use std::collections::HashSet;

use crate::model::{
    SlackFileId, SlackFileStagingAllocatedAttempts, SlackFileStagingDiagnostic,
    SlackFileStagingFailure, SlackFileStagingOperationFailure, SlackFileStagingOperationId,
    SlackFileStagingOutcome, SlackStagedFile,
};

use super::{SlackFileStagingOperation, SlackFileStagingOperations};

pub(super) fn operation_mut<'a>(
    operations: &'a mut SlackFileStagingOperations,
    operation_id: &SlackFileStagingOperationId,
) -> Result<&'a mut SlackFileStagingOperation, SlackFileStagingOperationFailure> {
    operations.by_id.get_mut(operation_id).ok_or_else(|| {
        SlackFileStagingOperationFailure::UnknownOperation {
            operation_id: operation_id.clone(),
        }
    })
}

pub(super) fn ensure_receipt_is_latest(
    operation_id: &SlackFileStagingOperationId,
    attempts: &SlackFileStagingAllocatedAttempts,
    receipt: &SlackStagedFile,
) -> Result<(), SlackFileStagingOperationFailure> {
    if receipt.operation_id() != operation_id || receipt.allocated() != attempts.latest() {
        return Err(invalid_state(
            operation_id,
            "use a staged receipt that does not identify the final allocated attempt",
        ));
    }
    Ok(())
}

pub(super) fn ensure_draft_reservation_targets_latest(
    operation_id: &SlackFileStagingOperationId,
    attempts: &SlackFileStagingAllocatedAttempts,
    receipt: &SlackStagedFile,
    requested: &HashSet<&SlackFileId>,
) -> Result<(), SlackFileStagingOperationFailure> {
    ensure_receipt_is_latest(operation_id, attempts, receipt)?;
    if attempts.iter().any(|attempt| {
        requested.contains(attempt.file_id()) && attempt.file_id() != receipt.file_id()
    }) {
        return Err(invalid_state(
            operation_id,
            "transfer an abandoned upload attempt to Slack draft ownership",
        ));
    }
    Ok(())
}

pub(super) fn invalid_state(
    operation_id: &SlackFileStagingOperationId,
    operation: &'static str,
) -> SlackFileStagingOperationFailure {
    SlackFileStagingOperationFailure::InvalidState {
        operation_id: operation_id.clone(),
        diagnostic: diagnostic(format!(
            "Slack file-staging operation cannot {operation} from its current state"
        )),
    }
}

pub(super) fn internal_failure(message: impl Into<String>) -> SlackFileStagingOperationFailure {
    SlackFileStagingOperationFailure::Internal {
        diagnostic: diagnostic(message),
    }
}

pub(super) fn diagnostic(message: impl Into<String>) -> SlackFileStagingDiagnostic {
    SlackFileStagingDiagnostic::new(message.into())
}

pub(super) fn operation_outcome(
    failure: SlackFileStagingOperationFailure,
) -> SlackFileStagingOutcome {
    SlackFileStagingOutcome::Failed(SlackFileStagingFailure::Operation(failure))
}
