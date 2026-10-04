use std::{collections::HashSet, sync::atomic::Ordering};

use crate::model::{
    SlackFileId, SlackFileStagingAllocatedAttempts, SlackFileStagingCleanupOutcome,
    SlackFileStagingDiagnostic, SlackFileStagingOperationFailure, SlackFileStagingOperationId,
    SlackStagedFile,
};

use crate::live::api::SlackApiClient;

use super::{
    diagnostic, ensure_receipt_is_latest, invalid_state, operation_mut, SlackFileShareReservation,
    SlackFileStagingLedger, SlackFileStagingState, SlackOwnedCleanupTargets,
    SlackShareUnknownCleanupJob, SlackSharedCleanupJob,
};

impl SlackFileStagingLedger {
    pub(super) fn reserve_share(
        &self,
        file_ids: &[SlackFileId],
    ) -> Result<SlackFileShareReservation, SlackFileStagingOperationFailure> {
        let requested = file_ids.iter().collect::<HashSet<_>>();
        let mut operations = self.lock()?;
        let mut operation_ids = Vec::new();
        for (operation_id, operation) in &operations.by_id {
            let Some(attempts) = operation.state.allocated_attempts() else {
                continue;
            };
            let owns_requested_file = attempts
                .iter()
                .any(|attempt| requested.contains(attempt.file_id()));
            if !owns_requested_file {
                continue;
            }
            if share_reservation_matches(operation_id, attempts, &operation.state, &requested)? {
                operation_ids.push(operation_id.clone());
            }
        }
        for operation_id in &operation_ids {
            let operation = operation_mut(&mut operations, operation_id)?;
            let SlackFileStagingState::Staged { attempts, receipt } = &operation.state else {
                return Err(invalid_state(operation_id, "reserve file share"));
            };
            operation.state = SlackFileStagingState::Sharing {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
            };
        }
        Ok(SlackFileShareReservation { operation_ids })
    }

    pub(super) fn abort_share(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileShareReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let cancelled = {
            let mut operations = self.lock()?;
            for operation_id in &reservation.operation_ids {
                let operation = operation_mut(&mut operations, operation_id)?;
                let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(operation_id, "abort file share"));
                };
                ensure_receipt_is_latest(operation_id, attempts, receipt)?;
            }
            let mut cancelled = Vec::new();
            for operation_id in reservation.operation_ids {
                let operation = operation_mut(&mut operations, &operation_id)?;
                let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(&operation_id, "abort file share"));
                };
                operation.state = SlackFileStagingState::Staged {
                    attempts: attempts.clone(),
                    receipt: receipt.clone(),
                };
                if operation.cancellation.load(Ordering::Acquire) {
                    cancelled.push(operation_id);
                }
            }
            cancelled
        };
        for operation_id in cancelled {
            match self.cleanup(api, &operation_id) {
                SlackFileStagingCleanupOutcome::ConfirmedDeleted
                | SlackFileStagingCleanupOutcome::RetainedUnknown { .. } => {}
                SlackFileStagingCleanupOutcome::Failed(failure) => return Err(failure),
                _ => {
                    return Err(invalid_state(
                        &operation_id,
                        "finish cancelled file-share cleanup",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn protect_share_unknown(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileShareReservation,
        message: String,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let share_diagnostic = diagnostic(message);
        let cleanup = self.prepare_share_unknown_jobs(reservation, &share_diagnostic)?;
        for job in cleanup {
            let operation_id = job.operation_id.clone();
            match self.finish_share_unknown_cleanup(api, job) {
                SlackFileStagingCleanupOutcome::RetainedUnknown { .. } => {}
                SlackFileStagingCleanupOutcome::Failed(failure) => return Err(failure),
                _ => {
                    return Err(invalid_state(
                        &operation_id,
                        "finish ambiguous-share cleanup",
                    ));
                }
            }
        }
        Ok(())
    }

    fn prepare_share_unknown_jobs(
        &self,
        reservation: SlackFileShareReservation,
        share_diagnostic: &SlackFileStagingDiagnostic,
    ) -> Result<Vec<SlackShareUnknownCleanupJob>, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        for operation_id in &reservation.operation_ids {
            let operation = operation_mut(&mut operations, operation_id)?;
            let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                return Err(invalid_state(operation_id, "protect ambiguous file share"));
            };
            ensure_receipt_is_latest(operation_id, attempts, receipt)?;
        }
        let mut cleanup = Vec::new();
        for operation_id in reservation.operation_ids {
            let operation = operation_mut(&mut operations, &operation_id)?;
            let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                return Err(invalid_state(&operation_id, "protect ambiguous file share"));
            };
            let attempts = attempts.clone();
            let receipt = receipt.clone();
            match SlackOwnedCleanupTargets::from_attempts(&attempts, Some(receipt.file_id())) {
                Some(targets) => {
                    operation.state = SlackFileStagingState::ShareUnknownCleaning {
                        attempts: attempts.clone(),
                        receipt: receipt.clone(),
                        diagnostic: share_diagnostic.clone(),
                        targets: targets.clone(),
                    };
                    cleanup.push(SlackShareUnknownCleanupJob {
                        operation_id,
                        attempts,
                        receipt,
                        diagnostic: share_diagnostic.clone(),
                        targets,
                    });
                }
                None => {
                    operation.state = SlackFileStagingState::ShareUnknown {
                        attempts,
                        receipt,
                        diagnostic: share_diagnostic.clone(),
                    };
                }
            }
        }
        Ok(cleanup)
    }

    pub(super) fn confirm_share(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileShareReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let cleanup = {
            let mut operations = self.lock()?;
            for operation_id in &reservation.operation_ids {
                let operation = operation_mut(&mut operations, operation_id)?;
                let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(operation_id, "confirm file share"));
                };
                ensure_receipt_is_latest(operation_id, attempts, receipt)?;
            }
            let mut cleanup = Vec::new();
            for operation_id in reservation.operation_ids {
                let operation = operation_mut(&mut operations, &operation_id)?;
                let SlackFileStagingState::Sharing { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(&operation_id, "confirm file share"));
                };
                let attempts = attempts.clone();
                let receipt = receipt.clone();
                match SlackOwnedCleanupTargets::from_attempts(&attempts, Some(receipt.file_id())) {
                    Some(targets) => {
                        operation.state = SlackFileStagingState::SharedCleaning {
                            attempts: attempts.clone(),
                            receipt: receipt.clone(),
                            targets: targets.clone(),
                        };
                        cleanup.push(SlackSharedCleanupJob {
                            operation_id,
                            attempts,
                            receipt,
                            targets,
                        });
                    }
                    None => {
                        operation.state = SlackFileStagingState::Shared { attempts, receipt };
                    }
                }
            }
            cleanup
        };
        for job in cleanup {
            let operation_id = job.operation_id.clone();
            match self.finish_shared_cleanup(api, job) {
                SlackFileStagingCleanupOutcome::Shared
                | SlackFileStagingCleanupOutcome::RetainedUnknown { .. } => {}
                SlackFileStagingCleanupOutcome::Failed(failure) => return Err(failure),
                _ => {
                    return Err(invalid_state(&operation_id, "finish shared-file cleanup"));
                }
            }
        }
        Ok(())
    }
}

fn share_reservation_matches(
    operation_id: &SlackFileStagingOperationId,
    attempts: &SlackFileStagingAllocatedAttempts,
    state: &SlackFileStagingState,
    requested: &HashSet<&SlackFileId>,
) -> Result<bool, SlackFileStagingOperationFailure> {
    let receipt = match state {
        SlackFileStagingState::Staged { receipt, .. }
        | SlackFileStagingState::Shared { receipt, .. } => receipt,
        _ => {
            return Err(invalid_state(
                operation_id,
                "share an operation-owned file before staging is complete",
            ));
        }
    };
    ensure_share_targets_latest(operation_id, attempts, receipt, requested)?;
    Ok(matches!(state, SlackFileStagingState::Staged { .. })
        && requested.contains(receipt.file_id()))
}

fn ensure_share_targets_latest(
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
            "share an abandoned upload attempt",
        ));
    }
    Ok(())
}
