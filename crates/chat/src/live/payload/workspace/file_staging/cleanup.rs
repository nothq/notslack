mod execution;
mod ownership;
mod preparation;

use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingCleanupOutcome, SlackFileStagingOperationId,
};

use crate::live::api::SlackApiClient;

use super::{
    diagnostic, invalid_state, operation_mut, SlackDraftOwnedCleanupJob,
    SlackDraftUnknownCleanupJob, SlackFileStagingLedger, SlackFileStagingState,
    SlackOwnedCleanupTargets, SlackShareUnknownCleanupJob, SlackSharedCleanupJob,
};
use execution::{delete_owned_files, SlackCleanupExecution, SlackCleanupPreparation};

impl SlackFileStagingLedger {
    pub(super) fn cleanup(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingCleanupOutcome {
        let preparation = match self.prepare_cleanup(operation_id) {
            Ok(preparation) => preparation,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        self.finish_prepared_cleanup(api, operation_id, preparation)
    }

    fn finish_prepared_cleanup(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
        preparation: SlackCleanupPreparation,
    ) -> SlackFileStagingCleanupOutcome {
        match preparation {
            SlackCleanupPreparation::Unshared { attempts, targets } => {
                self.finish_unshared_cleanup(api, operation_id, attempts, targets)
            }
            SlackCleanupPreparation::Shared {
                attempts,
                receipt,
                targets,
            } => self.finish_shared_cleanup(
                api,
                SlackSharedCleanupJob {
                    operation_id: operation_id.clone(),
                    attempts,
                    receipt,
                    targets,
                },
            ),
            SlackCleanupPreparation::ShareUnknown {
                attempts,
                receipt,
                diagnostic,
                targets,
            } => self.finish_share_unknown_cleanup(
                api,
                SlackShareUnknownCleanupJob {
                    operation_id: operation_id.clone(),
                    attempts,
                    receipt,
                    diagnostic,
                    targets,
                },
            ),
            SlackCleanupPreparation::RetainedShareUnknown {
                attempts,
                diagnostic,
            } => retained_cleanup(attempts, diagnostic),
            other => self.finish_prepared_draft_or_terminal_cleanup(api, operation_id, other),
        }
    }

    fn finish_prepared_draft_or_terminal_cleanup(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
        preparation: SlackCleanupPreparation,
    ) -> SlackFileStagingCleanupOutcome {
        match preparation {
            SlackCleanupPreparation::DraftOwned {
                attempts,
                receipt,
                targets,
            } => self.finish_draft_owned_cleanup(
                api,
                SlackDraftOwnedCleanupJob {
                    operation_id: operation_id.clone(),
                    attempts,
                    receipt,
                    targets,
                },
            ),
            SlackCleanupPreparation::DraftUnknown {
                attempts,
                receipt,
                diagnostic,
                targets,
            } => self.finish_draft_unknown_cleanup(
                api,
                SlackDraftUnknownCleanupJob {
                    operation_id: operation_id.clone(),
                    attempts,
                    receipt,
                    diagnostic,
                    targets,
                },
            ),
            SlackCleanupPreparation::RetainedDraftUnknown {
                attempts,
                diagnostic,
            } => retained_cleanup(attempts, diagnostic),
            SlackCleanupPreparation::Pending => SlackFileStagingCleanupOutcome::Pending,
            SlackCleanupPreparation::NoPersistentFile => {
                SlackFileStagingCleanupOutcome::NoPersistentFile
            }
            SlackCleanupPreparation::ConfirmedDeleted => {
                SlackFileStagingCleanupOutcome::ConfirmedDeleted
            }
            SlackCleanupPreparation::AlreadyShared => SlackFileStagingCleanupOutcome::Shared,
            SlackCleanupPreparation::AlreadyDraftOwned => {
                SlackFileStagingCleanupOutcome::DraftOwned
            }
            _ => unreachable!("share cleanup preparations are dispatched before draft cleanup"),
        }
    }

    fn finish_unshared_cleanup(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
        attempts: SlackFileStagingAllocatedAttempts,
        targets: SlackOwnedCleanupTargets,
    ) -> SlackFileStagingCleanupOutcome {
        let execution = delete_owned_files(api, &targets);
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        let operation = match operation_mut(&mut operations, operation_id) {
            Ok(operation) => operation,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        if !matches!(
            &operation.state,
            SlackFileStagingState::Cleaning {
                attempts: active_attempts,
                targets: active_targets,
            } if active_attempts == &attempts && active_targets == &targets
        ) {
            return SlackFileStagingCleanupOutcome::Failed(invalid_state(
                operation_id,
                "finish cleanup",
            ));
        }
        match execution {
            SlackCleanupExecution::Confirmed => {
                operation.state = SlackFileStagingState::Deleted { attempts };
                SlackFileStagingCleanupOutcome::ConfirmedDeleted
            }
            SlackCleanupExecution::Unresolved {
                targets: unresolved,
                diagnostic: message,
            } => {
                let outcome = SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts.clone(),
                    diagnostic: diagnostic(message),
                };
                operation.state = SlackFileStagingState::CleanupUnknown {
                    attempts,
                    unresolved,
                };
                outcome
            }
        }
    }

    pub(super) fn finish_shared_cleanup(
        &self,
        api: &SlackApiClient,
        job: SlackSharedCleanupJob,
    ) -> SlackFileStagingCleanupOutcome {
        let SlackSharedCleanupJob {
            operation_id,
            attempts,
            receipt,
            targets,
        } = job;
        let execution = delete_owned_files(api, &targets);
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        let operation = match operation_mut(&mut operations, &operation_id) {
            Ok(operation) => operation,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        if !matches!(
            &operation.state,
            SlackFileStagingState::SharedCleaning {
                attempts: active_attempts,
                receipt: active_receipt,
                targets: active_targets,
            } if active_attempts == &attempts
                && active_receipt == &receipt
                && active_targets == &targets
        ) {
            return SlackFileStagingCleanupOutcome::Failed(invalid_state(
                &operation_id,
                "finish shared-file cleanup",
            ));
        }
        match execution {
            SlackCleanupExecution::Confirmed => {
                operation.state = SlackFileStagingState::Shared { attempts, receipt };
                SlackFileStagingCleanupOutcome::Shared
            }
            SlackCleanupExecution::Unresolved {
                targets: unresolved,
                diagnostic: message,
            } => {
                let outcome = SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts.clone(),
                    diagnostic: diagnostic(message),
                };
                operation.state = SlackFileStagingState::SharedCleanupUnknown {
                    attempts,
                    receipt,
                    unresolved,
                };
                outcome
            }
        }
    }
}

fn retained_cleanup(
    attempts: SlackFileStagingAllocatedAttempts,
    diagnostic: crate::model::SlackFileStagingDiagnostic,
) -> SlackFileStagingCleanupOutcome {
    SlackFileStagingCleanupOutcome::RetainedUnknown {
        allocated_attempts: attempts,
        diagnostic,
    }
}
