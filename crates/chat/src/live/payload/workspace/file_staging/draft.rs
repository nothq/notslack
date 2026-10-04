use std::{collections::HashSet, sync::atomic::Ordering};

use crate::model::{
    SlackFileId, SlackFileStagingCleanupOutcome, SlackFileStagingDiagnostic,
    SlackFileStagingOperationFailure, SlackFileStagingOperationId, SlackScheduledDraftLocalFile,
};

use crate::live::api::SlackApiClient;

use super::{
    diagnostic, ensure_draft_reservation_targets_latest, ensure_receipt_is_latest, invalid_state,
    operation_mut, SlackDraftOwnedCleanupJob, SlackDraftUnknownCleanupJob,
    SlackFileDraftReservation, SlackFileStagingLedger, SlackFileStagingOperations,
    SlackFileStagingState, SlackOwnedCleanupTargets,
};

impl SlackFileStagingLedger {
    pub(super) fn reserve_draft(
        &self,
        file_ids: &[SlackFileId],
    ) -> Result<SlackFileDraftReservation, SlackFileStagingOperationFailure> {
        let requested = file_ids.iter().collect::<HashSet<_>>();
        let mut operations = self.lock()?;
        let mut operation_ids = Vec::new();
        for (operation_id, operation) in &operations.by_id {
            let Some(attempts) = operation.state.allocated_attempts() else {
                continue;
            };
            if !attempts
                .iter()
                .any(|attempt| requested.contains(attempt.file_id()))
            {
                continue;
            }
            match &operation.state {
                SlackFileStagingState::Staged { receipt, .. } => {
                    ensure_draft_reservation_targets_latest(
                        operation_id,
                        attempts,
                        receipt,
                        &requested,
                    )?;
                    if requested.contains(receipt.file_id()) {
                        operation_ids.push(operation_id.clone());
                    }
                }
                SlackFileStagingState::Shared { receipt, .. }
                | SlackFileStagingState::SharedCleaning { receipt, .. }
                | SlackFileStagingState::SharedCleanupUnknown { receipt, .. }
                | SlackFileStagingState::DraftOwned { receipt, .. }
                | SlackFileStagingState::DraftOwnedCleaning { receipt, .. }
                | SlackFileStagingState::DraftOwnedCleanupUnknown { receipt, .. } => {
                    ensure_draft_reservation_targets_latest(
                        operation_id,
                        attempts,
                        receipt,
                        &requested,
                    )?;
                }
                _ => {
                    return Err(invalid_state(
                        operation_id,
                        "transfer an active or ambiguous locally tracked file to Slack draft ownership",
                    ));
                }
            }
        }
        begin_draft_reservations(&mut operations, &operation_ids)?;
        Ok(SlackFileDraftReservation { operation_ids })
    }

    pub(super) fn abort_draft(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileDraftReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let cancelled = {
            let mut operations = self.lock()?;
            for operation_id in &reservation.operation_ids {
                let operation = operation_mut(&mut operations, operation_id)?;
                let SlackFileStagingState::Drafting { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(operation_id, "abort Slack draft ownership"));
                };
                ensure_receipt_is_latest(operation_id, attempts, receipt)?;
            }
            let mut cancelled = Vec::new();
            for operation_id in reservation.operation_ids {
                let operation = operation_mut(&mut operations, &operation_id)?;
                let SlackFileStagingState::Drafting { attempts, receipt } = &operation.state else {
                    return Err(invalid_state(&operation_id, "abort Slack draft ownership"));
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
                        "finish cancelled Slack draft-ownership cleanup",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn protect_draft_unknown(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileDraftReservation,
        message: String,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let draft_diagnostic = diagnostic(message);
        let cleanup = self.prepare_draft_unknown_jobs(reservation, &draft_diagnostic)?;
        for job in cleanup {
            let operation_id = job.operation_id.clone();
            match self.finish_draft_unknown_cleanup(api, job) {
                SlackFileStagingCleanupOutcome::RetainedUnknown { .. } => {}
                SlackFileStagingCleanupOutcome::Failed(failure) => return Err(failure),
                _ => {
                    return Err(invalid_state(
                        &operation_id,
                        "finish ambiguous Slack draft-ownership cleanup",
                    ));
                }
            }
        }
        Ok(())
    }

    fn prepare_draft_unknown_jobs(
        &self,
        reservation: SlackFileDraftReservation,
        draft_diagnostic: &SlackFileStagingDiagnostic,
    ) -> Result<Vec<SlackDraftUnknownCleanupJob>, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        validate_drafting_operations(
            &mut operations,
            &reservation.operation_ids,
            "protect ambiguous Slack draft ownership",
        )?;
        let mut cleanup = Vec::new();
        for operation_id in reservation.operation_ids {
            let operation = operation_mut(&mut operations, &operation_id)?;
            let SlackFileStagingState::Drafting { attempts, receipt } = &operation.state else {
                return Err(invalid_state(
                    &operation_id,
                    "protect ambiguous Slack draft ownership",
                ));
            };
            let attempts = attempts.clone();
            let receipt = receipt.clone();
            match SlackOwnedCleanupTargets::from_attempts(&attempts, Some(receipt.file_id())) {
                Some(targets) => {
                    operation.state = SlackFileStagingState::DraftUnknownCleaning {
                        attempts: attempts.clone(),
                        receipt: receipt.clone(),
                        diagnostic: draft_diagnostic.clone(),
                        targets: targets.clone(),
                    };
                    cleanup.push(SlackDraftUnknownCleanupJob {
                        operation_id,
                        attempts,
                        receipt,
                        diagnostic: draft_diagnostic.clone(),
                        targets,
                    });
                }
                None => {
                    operation.state = SlackFileStagingState::DraftUnknown {
                        attempts,
                        receipt,
                        diagnostic: draft_diagnostic.clone(),
                    };
                }
            }
        }
        Ok(cleanup)
    }

    pub(super) fn confirm_draft(
        &self,
        api: &SlackApiClient,
        reservation: SlackFileDraftReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let cleanup = self.prepare_draft_owned_jobs(reservation)?;
        for job in cleanup {
            let operation_id = job.operation_id.clone();
            match self.finish_draft_owned_cleanup(api, job) {
                SlackFileStagingCleanupOutcome::DraftOwned
                | SlackFileStagingCleanupOutcome::RetainedUnknown { .. } => {}
                SlackFileStagingCleanupOutcome::Failed(failure) => return Err(failure),
                _ => {
                    return Err(invalid_state(
                        &operation_id,
                        "finish draft-owned file cleanup",
                    ));
                }
            }
        }
        Ok(())
    }

    fn prepare_draft_owned_jobs(
        &self,
        reservation: SlackFileDraftReservation,
    ) -> Result<Vec<SlackDraftOwnedCleanupJob>, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        validate_drafting_operations(
            &mut operations,
            &reservation.operation_ids,
            "confirm Slack draft ownership",
        )?;
        let mut cleanup = Vec::new();
        for operation_id in reservation.operation_ids {
            let operation = operation_mut(&mut operations, &operation_id)?;
            let SlackFileStagingState::Drafting { attempts, receipt } = &operation.state else {
                return Err(invalid_state(
                    &operation_id,
                    "confirm Slack draft ownership",
                ));
            };
            let attempts = attempts.clone();
            let receipt = receipt.clone();
            match SlackOwnedCleanupTargets::from_attempts(&attempts, Some(receipt.file_id())) {
                Some(targets) => {
                    operation.state = SlackFileStagingState::DraftOwnedCleaning {
                        attempts: attempts.clone(),
                        receipt: receipt.clone(),
                        targets: targets.clone(),
                    };
                    cleanup.push(SlackDraftOwnedCleanupJob {
                        operation_id,
                        attempts,
                        receipt,
                        targets,
                    });
                }
                None => {
                    operation.state = SlackFileStagingState::DraftOwned { attempts, receipt };
                }
            }
        }
        Ok(cleanup)
    }

    pub(super) fn confirm_draft_unknown(
        &self,
        local_files: &[SlackScheduledDraftLocalFile],
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        validate_draft_unknown_files(&mut operations, local_files)?;
        confirm_draft_unknown_files(&mut operations, local_files)
    }
}

fn begin_draft_reservations(
    operations: &mut SlackFileStagingOperations,
    operation_ids: &[SlackFileStagingOperationId],
) -> Result<(), SlackFileStagingOperationFailure> {
    for operation_id in operation_ids {
        let operation = operation_mut(operations, operation_id)?;
        let SlackFileStagingState::Staged { attempts, receipt } = &operation.state else {
            return Err(invalid_state(operation_id, "reserve Slack draft ownership"));
        };
        operation.state = SlackFileStagingState::Drafting {
            attempts: attempts.clone(),
            receipt: receipt.clone(),
        };
    }
    Ok(())
}

fn validate_drafting_operations(
    operations: &mut SlackFileStagingOperations,
    operation_ids: &[SlackFileStagingOperationId],
    action: &'static str,
) -> Result<(), SlackFileStagingOperationFailure> {
    for operation_id in operation_ids {
        let operation = operation_mut(operations, operation_id)?;
        let SlackFileStagingState::Drafting { attempts, receipt } = &operation.state else {
            return Err(invalid_state(operation_id, action));
        };
        ensure_receipt_is_latest(operation_id, attempts, receipt)?;
    }
    Ok(())
}

fn validate_draft_unknown_files(
    operations: &mut SlackFileStagingOperations,
    local_files: &[SlackScheduledDraftLocalFile],
) -> Result<(), SlackFileStagingOperationFailure> {
    let mut unique_operation_ids = HashSet::with_capacity(local_files.len());
    let mut unique_file_ids = HashSet::with_capacity(local_files.len());
    for local_file in local_files {
        if !unique_operation_ids.insert(local_file.operation_id()) {
            return Err(invalid_state(
                local_file.operation_id(),
                "confirm duplicate ambiguous Slack draft ownership",
            ));
        }
        if !unique_file_ids.insert(local_file.file_id()) {
            return Err(invalid_state(
                local_file.operation_id(),
                "confirm duplicate ambiguous Slack draft file ownership",
            ));
        }
        let operation = operation_mut(operations, local_file.operation_id())?;
        match &operation.state {
            SlackFileStagingState::DraftUnknown {
                attempts, receipt, ..
            }
            | SlackFileStagingState::DraftUnknownCleanupUnknown {
                attempts, receipt, ..
            }
            | SlackFileStagingState::DraftOwned {
                attempts, receipt, ..
            }
            | SlackFileStagingState::DraftOwnedCleanupUnknown {
                attempts, receipt, ..
            } => {
                ensure_receipt_is_latest(local_file.operation_id(), attempts, receipt)?;
                if receipt.file_id() != local_file.file_id() {
                    return Err(invalid_state(
                        local_file.operation_id(),
                        "confirm ambiguous Slack draft ownership for a mismatched latest file",
                    ));
                }
            }
            _ => {
                return Err(invalid_state(
                    local_file.operation_id(),
                    "confirm ambiguous Slack draft ownership",
                ));
            }
        }
    }
    Ok(())
}

fn confirm_draft_unknown_files(
    operations: &mut SlackFileStagingOperations,
    local_files: &[SlackScheduledDraftLocalFile],
) -> Result<(), SlackFileStagingOperationFailure> {
    for local_file in local_files {
        let operation = operation_mut(operations, local_file.operation_id())?;
        operation.state = match &operation.state {
            SlackFileStagingState::DraftUnknown {
                attempts, receipt, ..
            } => SlackFileStagingState::DraftOwned {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
            },
            SlackFileStagingState::DraftUnknownCleanupUnknown {
                attempts,
                receipt,
                unresolved,
                ..
            } => SlackFileStagingState::DraftOwnedCleanupUnknown {
                attempts: attempts.clone(),
                receipt: receipt.clone(),
                unresolved: unresolved.clone(),
            },
            SlackFileStagingState::DraftOwned { .. }
            | SlackFileStagingState::DraftOwnedCleanupUnknown { .. } => continue,
            _ => {
                return Err(invalid_state(
                    local_file.operation_id(),
                    "finish confirming ambiguous Slack draft ownership",
                ));
            }
        };
    }
    Ok(())
}
