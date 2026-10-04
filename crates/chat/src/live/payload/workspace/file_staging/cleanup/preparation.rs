use std::sync::atomic::Ordering;

use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingDiagnostic,
    SlackFileStagingOperationFailure, SlackFileStagingOperationId, SlackStagedFile,
};

use super::super::{
    ensure_receipt_is_latest, invalid_state, operation_mut, SlackFileStagingLedger,
    SlackFileStagingOperation, SlackFileStagingState, SlackOwnedCleanupTargets,
};
use super::execution::SlackCleanupPreparation;

impl SlackFileStagingLedger {
    pub(super) fn prepare_cleanup(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        operation.cancellation.store(true, Ordering::Release);
        prepare_cleanup_state(operation_id, operation, operation.state.clone())
    }
}

fn prepare_cleanup_state(
    operation_id: &SlackFileStagingOperationId,
    operation: &mut SlackFileStagingOperation,
    state: SlackFileStagingState,
) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
    if is_share_cleanup_state(&state) {
        return prepare_share_cleanup(operation_id, operation, state);
    }
    if is_draft_cleanup_state(&state) {
        return prepare_draft_cleanup(operation_id, operation, state);
    }
    prepare_unowned_cleanup(operation_id, operation, state)
}

fn is_share_cleanup_state(state: &SlackFileStagingState) -> bool {
    matches!(
        state,
        SlackFileStagingState::ShareUnknown { .. }
            | SlackFileStagingState::ShareUnknownCleanupUnknown { .. }
            | SlackFileStagingState::SharedCleanupUnknown { .. }
            | SlackFileStagingState::Shared { .. }
    )
}

fn is_draft_cleanup_state(state: &SlackFileStagingState) -> bool {
    matches!(
        state,
        SlackFileStagingState::DraftUnknown { .. }
            | SlackFileStagingState::DraftUnknownCleanupUnknown { .. }
            | SlackFileStagingState::DraftOwnedCleanupUnknown { .. }
            | SlackFileStagingState::DraftOwned { .. }
    )
}

fn prepare_unowned_cleanup(
    operation_id: &SlackFileStagingOperationId,
    operation: &mut SlackFileStagingOperation,
    state: SlackFileStagingState,
) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
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
        | SlackFileStagingState::DraftOwnedCleaning { .. } => Ok(SlackCleanupPreparation::Pending),
        SlackFileStagingState::FailedBeforeAllocation
        | SlackFileStagingState::CancelledBeforeAllocation => {
            Ok(SlackCleanupPreparation::NoPersistentFile)
        }
        SlackFileStagingState::FailedBeforeCompletion { attempts }
        | SlackFileStagingState::CancelledBeforeCompletion { attempts }
        | SlackFileStagingState::CompletionNotSent { attempts }
        | SlackFileStagingState::CompletionRejected { attempts }
        | SlackFileStagingState::CompletionUnknown { attempts }
        | SlackFileStagingState::Staged { attempts, .. } => {
            prepare_unshared_cleanup(operation_id, operation, attempts)
        }
        SlackFileStagingState::CleanupUnknown {
            attempts,
            unresolved,
        } => Ok(set_unshared_cleanup(operation, attempts, unresolved)),
        SlackFileStagingState::Deleted { .. } => Ok(SlackCleanupPreparation::ConfirmedDeleted),
        _ => unreachable!("ownership cleanup states are dispatched before unowned cleanup"),
    }
}

fn prepare_unshared_cleanup(
    operation_id: &SlackFileStagingOperationId,
    operation: &mut SlackFileStagingOperation,
    attempts: SlackFileStagingAllocatedAttempts,
) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
    let targets = SlackOwnedCleanupTargets::from_attempts(&attempts, None)
        .ok_or_else(|| invalid_state(operation_id, "prepare cleanup without targets"))?;
    Ok(set_unshared_cleanup(operation, attempts, targets))
}

fn set_unshared_cleanup(
    operation: &mut SlackFileStagingOperation,
    attempts: SlackFileStagingAllocatedAttempts,
    targets: SlackOwnedCleanupTargets,
) -> SlackCleanupPreparation {
    operation.state = SlackFileStagingState::Cleaning {
        attempts: attempts.clone(),
        targets: targets.clone(),
    };
    SlackCleanupPreparation::Unshared { attempts, targets }
}

fn prepare_share_cleanup(
    operation_id: &SlackFileStagingOperationId,
    operation: &mut SlackFileStagingOperation,
    state: SlackFileStagingState,
) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
    match state {
        SlackFileStagingState::ShareUnknown {
            attempts,
            receipt,
            diagnostic,
        } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            Ok(SlackCleanupPreparation::RetainedShareUnknown {
                attempts,
                diagnostic,
            })
        }
        SlackFileStagingState::ShareUnknownCleanupUnknown {
            attempts,
            receipt,
            diagnostic,
            unresolved,
        } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            operation.state = SlackFileStagingState::ShareUnknownCleaning {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
                diagnostic: diagnostic.clone(),
                targets: unresolved.clone(),
            };
            Ok(SlackCleanupPreparation::ShareUnknown {
                attempts,
                receipt,
                diagnostic,
                targets: unresolved,
            })
        }
        SlackFileStagingState::SharedCleanupUnknown {
            attempts,
            receipt,
            unresolved,
        } => {
            operation.state = SlackFileStagingState::SharedCleaning {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
                targets: unresolved.clone(),
            };
            Ok(SlackCleanupPreparation::Shared {
                attempts,
                receipt,
                targets: unresolved,
            })
        }
        SlackFileStagingState::Shared { attempts, receipt } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            Ok(SlackCleanupPreparation::AlreadyShared)
        }
        _ => unreachable!("share cleanup dispatcher received a non-share state"),
    }
}

fn prepare_draft_cleanup(
    operation_id: &SlackFileStagingOperationId,
    operation: &mut SlackFileStagingOperation,
    state: SlackFileStagingState,
) -> Result<SlackCleanupPreparation, SlackFileStagingOperationFailure> {
    match state {
        SlackFileStagingState::DraftUnknown {
            attempts,
            receipt,
            diagnostic,
        } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            Ok(SlackCleanupPreparation::RetainedDraftUnknown {
                attempts,
                diagnostic,
            })
        }
        SlackFileStagingState::DraftUnknownCleanupUnknown {
            attempts,
            receipt,
            diagnostic,
            unresolved,
        } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            Ok(set_unknown_draft_cleanup(
                operation, attempts, receipt, diagnostic, unresolved,
            ))
        }
        SlackFileStagingState::DraftOwnedCleanupUnknown {
            attempts,
            receipt,
            unresolved,
        } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            operation.state = SlackFileStagingState::DraftOwnedCleaning {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
                targets: unresolved.clone(),
            };
            Ok(SlackCleanupPreparation::DraftOwned {
                attempts,
                receipt,
                targets: unresolved,
            })
        }
        SlackFileStagingState::DraftOwned { attempts, receipt } => {
            ensure_receipt_is_latest(operation_id, &attempts, &receipt)?;
            Ok(SlackCleanupPreparation::AlreadyDraftOwned)
        }
        _ => unreachable!("draft cleanup dispatcher received a non-draft state"),
    }
}

fn set_unknown_draft_cleanup(
    operation: &mut SlackFileStagingOperation,
    attempts: SlackFileStagingAllocatedAttempts,
    receipt: SlackStagedFile,
    diagnostic: SlackFileStagingDiagnostic,
    unresolved: SlackOwnedCleanupTargets,
) -> SlackCleanupPreparation {
    operation.state = SlackFileStagingState::DraftUnknownCleaning {
        attempts: attempts.clone(),
        receipt: receipt.clone(),
        diagnostic: diagnostic.clone(),
        targets: unresolved.clone(),
    };
    SlackCleanupPreparation::DraftUnknown {
        attempts,
        receipt,
        diagnostic,
        targets: unresolved,
    }
}
