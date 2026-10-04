use std::sync::atomic::Ordering;

use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingCancellationOutcome,
    SlackFileStagingCleanupOutcome, SlackFileStagingOperationFailure, SlackFileStagingOperationId,
    SlackFileStagingReconcileOutcome, SlackStagedFile,
};

use crate::live::api::SlackApiClient;

use super::{
    diagnostic, invalid_state, observe_file_info, operation_mut, SlackFileInfoObservation,
    SlackFileStagingLedger, SlackFileStagingState,
};

enum SlackCancellationAction {
    Pending,
    NoPersistentFile,
    ConfirmedDeleted,
    Shared,
    DraftOwned,
    Cleanup,
}

fn cleanup_to_cancellation(
    outcome: SlackFileStagingCleanupOutcome,
) -> SlackFileStagingCancellationOutcome {
    match outcome {
        SlackFileStagingCleanupOutcome::Pending => SlackFileStagingCancellationOutcome::Pending,
        SlackFileStagingCleanupOutcome::NoPersistentFile => {
            SlackFileStagingCancellationOutcome::NoPersistentFile
        }
        SlackFileStagingCleanupOutcome::ConfirmedDeleted => {
            SlackFileStagingCancellationOutcome::ConfirmedDeleted
        }
        SlackFileStagingCleanupOutcome::Shared => SlackFileStagingCancellationOutcome::Shared,
        SlackFileStagingCleanupOutcome::DraftOwned => {
            SlackFileStagingCancellationOutcome::DraftOwned
        }
        SlackFileStagingCleanupOutcome::RetainedUnknown {
            allocated_attempts,
            diagnostic,
        } => SlackFileStagingCancellationOutcome::RetainedUnknown {
            allocated_attempts,
            diagnostic,
        },
        SlackFileStagingCleanupOutcome::Failed(failure) => {
            SlackFileStagingCancellationOutcome::Failed(failure)
        }
    }
}
impl SlackFileStagingLedger {
    pub(super) fn cancel(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingCancellationOutcome {
        let action = match self.prepare_cancellation(operation_id) {
            Ok(action) => action,
            Err(failure) => return SlackFileStagingCancellationOutcome::Failed(failure),
        };
        match action {
            SlackCancellationAction::Pending => SlackFileStagingCancellationOutcome::Pending,
            SlackCancellationAction::NoPersistentFile => {
                SlackFileStagingCancellationOutcome::NoPersistentFile
            }
            SlackCancellationAction::ConfirmedDeleted => {
                SlackFileStagingCancellationOutcome::ConfirmedDeleted
            }
            SlackCancellationAction::Shared => SlackFileStagingCancellationOutcome::Shared,
            SlackCancellationAction::DraftOwned => SlackFileStagingCancellationOutcome::DraftOwned,
            SlackCancellationAction::Cleanup => {
                cleanup_to_cancellation(self.cleanup(api, operation_id))
            }
        }
    }

    fn prepare_cancellation(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> Result<SlackCancellationAction, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        operation.cancellation.store(true, Ordering::Release);
        Ok(cancellation_action(&operation.state))
    }

    pub(super) fn reconcile(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingReconcileOutcome {
        let (attempts, file_id) = {
            let mut operations = match self.lock() {
                Ok(operations) => operations,
                Err(failure) => return SlackFileStagingReconcileOutcome::Failed(failure),
            };
            let operation = match operation_mut(&mut operations, operation_id) {
                Ok(operation) => operation,
                Err(failure) => return SlackFileStagingReconcileOutcome::Failed(failure),
            };
            match &operation.state {
                SlackFileStagingState::Staged { receipt, .. } => {
                    return SlackFileStagingReconcileOutcome::Staged(receipt.clone());
                }
                SlackFileStagingState::CompletionUnknown { attempts } => {
                    let attempts = attempts.clone();
                    let file_id = attempts.latest().file_id().clone();
                    operation.state = SlackFileStagingState::Reconciling {
                        attempts: attempts.clone(),
                    };
                    (attempts, file_id)
                }
                _ => {
                    return SlackFileStagingReconcileOutcome::Failed(invalid_state(
                        operation_id,
                        "reconcile completion",
                    ));
                }
            }
        };

        self.finish_reconciliation(
            api,
            operation_id,
            attempts,
            observe_file_info(api, &file_id),
        )
    }

    fn finish_reconciliation(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
        attempts: SlackFileStagingAllocatedAttempts,
        observation: SlackFileInfoObservation,
    ) -> SlackFileStagingReconcileOutcome {
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return SlackFileStagingReconcileOutcome::Failed(failure),
        };
        let operation = match operation_mut(&mut operations, operation_id) {
            Ok(operation) => operation,
            Err(failure) => return SlackFileStagingReconcileOutcome::Failed(failure),
        };
        if !reconciliation_state_matches(&operation.state, &attempts) {
            return SlackFileStagingReconcileOutcome::Failed(invalid_state(
                operation_id,
                "finish completion reconciliation",
            ));
        }
        match observation {
            SlackFileInfoObservation::Present => {
                let receipt = SlackStagedFile::new(operation_id.clone(), attempts.latest().clone());
                operation.state = SlackFileStagingState::Staged {
                    attempts,
                    receipt: receipt.clone(),
                };
                let cancelled = operation.cancellation.load(Ordering::Acquire);
                drop(operations);
                if cancelled {
                    SlackFileStagingReconcileOutcome::Cancelled(cleanup_to_cancellation(
                        self.cleanup(api, operation_id),
                    ))
                } else {
                    SlackFileStagingReconcileOutcome::Staged(receipt)
                }
            }
            SlackFileInfoObservation::StillUnknown {
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::CompletionUnknown {
                    attempts: attempts.clone(),
                };
                let cancelled = operation.cancellation.load(Ordering::Acquire);
                drop(operations);
                if cancelled {
                    SlackFileStagingReconcileOutcome::Cancelled(cleanup_to_cancellation(
                        self.cleanup(api, operation_id),
                    ))
                } else {
                    SlackFileStagingReconcileOutcome::StillUnknown {
                        allocated_attempts: attempts,
                        diagnostic: diagnostic(message),
                    }
                }
            }
        }
    }
}

fn cancellation_action(state: &SlackFileStagingState) -> SlackCancellationAction {
    match state {
        SlackFileStagingState::Preparing
        | SlackFileStagingState::AllocatingFirst
        | SlackFileStagingState::RetryingWithoutAllocation
        | SlackFileStagingState::AllocatingRetryWithoutAllocation
        | SlackFileStagingState::RetryingWithAllocations { .. }
        | SlackFileStagingState::AllocatingRetryWithAllocations { .. }
        | SlackFileStagingState::Transferring { .. }
        | SlackFileStagingState::Completing { .. }
        | SlackFileStagingState::Reconciling { .. }
        | SlackFileStagingState::Cleaning { .. }
        | SlackFileStagingState::Sharing { .. }
        | SlackFileStagingState::ShareUnknownCleaning { .. }
        | SlackFileStagingState::SharedCleaning { .. }
        | SlackFileStagingState::Drafting { .. }
        | SlackFileStagingState::DraftUnknownCleaning { .. }
        | SlackFileStagingState::DraftOwnedCleaning { .. } => SlackCancellationAction::Pending,
        SlackFileStagingState::FailedBeforeAllocation
        | SlackFileStagingState::CancelledBeforeAllocation => {
            SlackCancellationAction::NoPersistentFile
        }
        SlackFileStagingState::FailedBeforeCompletion { .. }
        | SlackFileStagingState::CancelledBeforeCompletion { .. }
        | SlackFileStagingState::CompletionNotSent { .. }
        | SlackFileStagingState::CompletionRejected { .. }
        | SlackFileStagingState::CompletionUnknown { .. }
        | SlackFileStagingState::Staged { .. }
        | SlackFileStagingState::CleanupUnknown { .. }
        | SlackFileStagingState::ShareUnknown { .. }
        | SlackFileStagingState::ShareUnknownCleanupUnknown { .. }
        | SlackFileStagingState::SharedCleanupUnknown { .. }
        | SlackFileStagingState::DraftUnknown { .. }
        | SlackFileStagingState::DraftUnknownCleanupUnknown { .. }
        | SlackFileStagingState::DraftOwnedCleanupUnknown { .. } => {
            SlackCancellationAction::Cleanup
        }
        SlackFileStagingState::Deleted { .. } => SlackCancellationAction::ConfirmedDeleted,
        SlackFileStagingState::Shared { .. } => SlackCancellationAction::Shared,
        SlackFileStagingState::DraftOwned { .. } => SlackCancellationAction::DraftOwned,
    }
}

fn reconciliation_state_matches(
    state: &SlackFileStagingState,
    attempts: &SlackFileStagingAllocatedAttempts,
) -> bool {
    matches!(
        state,
        SlackFileStagingState::Reconciling {
            attempts: active_attempts,
        } if active_attempts == attempts
    )
}
