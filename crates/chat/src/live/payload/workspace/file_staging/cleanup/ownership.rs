use crate::model::SlackFileStagingCleanupOutcome;

use crate::live::api::SlackApiClient;

use super::super::{
    diagnostic, invalid_state, operation_mut, SlackDraftOwnedCleanupJob,
    SlackDraftUnknownCleanupJob, SlackFileStagingLedger, SlackFileStagingState,
    SlackShareUnknownCleanupJob,
};
use super::execution::{delete_owned_files, SlackCleanupExecution};

impl SlackFileStagingLedger {
    pub(in crate::live::payload::workspace::file_staging) fn finish_share_unknown_cleanup(
        &self,
        api: &SlackApiClient,
        job: SlackShareUnknownCleanupJob,
    ) -> SlackFileStagingCleanupOutcome {
        let execution = delete_owned_files(api, &job.targets);
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        let operation = match operation_mut(&mut operations, &job.operation_id) {
            Ok(operation) => operation,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        if !share_unknown_cleanup_matches(&operation.state, &job) {
            return SlackFileStagingCleanupOutcome::Failed(invalid_state(
                &job.operation_id,
                "finish ambiguous-share cleanup",
            ));
        }
        let SlackShareUnknownCleanupJob {
            attempts,
            receipt,
            diagnostic: share_diagnostic,
            ..
        } = job;
        match execution {
            SlackCleanupExecution::Confirmed => {
                operation.state = SlackFileStagingState::ShareUnknown {
                    attempts: attempts.clone(),
                    receipt,
                    diagnostic: share_diagnostic.clone(),
                };
                SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts,
                    diagnostic: share_diagnostic,
                }
            }
            SlackCleanupExecution::Unresolved {
                targets: unresolved,
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::ShareUnknownCleanupUnknown {
                    attempts: attempts.clone(),
                    receipt,
                    diagnostic: share_diagnostic,
                    unresolved,
                };
                SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts,
                    diagnostic: diagnostic(message),
                }
            }
        }
    }

    pub(in crate::live::payload::workspace::file_staging) fn finish_draft_owned_cleanup(
        &self,
        api: &SlackApiClient,
        job: SlackDraftOwnedCleanupJob,
    ) -> SlackFileStagingCleanupOutcome {
        let SlackDraftOwnedCleanupJob {
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
            SlackFileStagingState::DraftOwnedCleaning {
                attempts: active_attempts,
                receipt: active_receipt,
                targets: active_targets,
            } if active_attempts == &attempts
                && active_receipt == &receipt
                && active_targets == &targets
        ) {
            return SlackFileStagingCleanupOutcome::Failed(invalid_state(
                &operation_id,
                "finish draft-owned file cleanup",
            ));
        }
        match execution {
            SlackCleanupExecution::Confirmed => {
                operation.state = SlackFileStagingState::DraftOwned { attempts, receipt };
                SlackFileStagingCleanupOutcome::DraftOwned
            }
            SlackCleanupExecution::Unresolved {
                targets: unresolved,
                diagnostic: message,
            } => {
                let outcome = SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts.clone(),
                    diagnostic: diagnostic(message),
                };
                operation.state = SlackFileStagingState::DraftOwnedCleanupUnknown {
                    attempts,
                    receipt,
                    unresolved,
                };
                outcome
            }
        }
    }

    pub(in crate::live::payload::workspace::file_staging) fn finish_draft_unknown_cleanup(
        &self,
        api: &SlackApiClient,
        job: SlackDraftUnknownCleanupJob,
    ) -> SlackFileStagingCleanupOutcome {
        let execution = delete_owned_files(api, &job.targets);
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        let operation = match operation_mut(&mut operations, &job.operation_id) {
            Ok(operation) => operation,
            Err(failure) => return SlackFileStagingCleanupOutcome::Failed(failure),
        };
        if !draft_unknown_cleanup_matches(&operation.state, &job) {
            return SlackFileStagingCleanupOutcome::Failed(invalid_state(
                &job.operation_id,
                "finish ambiguous draft-ownership cleanup",
            ));
        }
        let SlackDraftUnknownCleanupJob {
            attempts,
            receipt,
            diagnostic: draft_diagnostic,
            ..
        } = job;
        match execution {
            SlackCleanupExecution::Confirmed => {
                operation.state = SlackFileStagingState::DraftUnknown {
                    attempts: attempts.clone(),
                    receipt,
                    diagnostic: draft_diagnostic.clone(),
                };
                SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts,
                    diagnostic: draft_diagnostic,
                }
            }
            SlackCleanupExecution::Unresolved {
                targets: unresolved,
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::DraftUnknownCleanupUnknown {
                    attempts: attempts.clone(),
                    receipt,
                    diagnostic: draft_diagnostic,
                    unresolved,
                };
                SlackFileStagingCleanupOutcome::RetainedUnknown {
                    allocated_attempts: attempts,
                    diagnostic: diagnostic(message),
                }
            }
        }
    }
}

fn share_unknown_cleanup_matches(
    state: &SlackFileStagingState,
    job: &SlackShareUnknownCleanupJob,
) -> bool {
    matches!(
        state,
        SlackFileStagingState::ShareUnknownCleaning {
            attempts,
            receipt,
            diagnostic,
            targets,
        } if attempts == &job.attempts
            && receipt == &job.receipt
            && diagnostic == &job.diagnostic
            && targets == &job.targets
    )
}

fn draft_unknown_cleanup_matches(
    state: &SlackFileStagingState,
    job: &SlackDraftUnknownCleanupJob,
) -> bool {
    matches!(
        state,
        SlackFileStagingState::DraftUnknownCleaning {
            attempts,
            receipt,
            diagnostic,
            targets,
        } if attempts == &job.attempts
            && receipt == &job.receipt
            && diagnostic == &job.diagnostic
            && targets == &job.targets
    )
}
