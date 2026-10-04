use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::model::{
    SlackAllocatedUpload, SlackFileStagingAllocatedAttempts, SlackFileStagingFailure,
    SlackFileStagingOperationFailure, SlackFileStagingOperationId, SlackFileStagingOutcome,
    SlackStagedFile,
};

use super::super::{
    diagnostic, invalid_state, operation_mut, operation_outcome, SlackFileStagingAttempt,
    SlackFileStagingLedger, SlackFileStagingOperation, SlackFileStagingState,
    SlackUploadCompletion,
};
use super::{
    SlackBeginAllocation, SlackBeginCompletion, SlackPrecompletionDisposition,
    SlackRecordedAllocation,
};

impl SlackFileStagingLedger {
    pub(super) fn register(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> Result<Arc<AtomicBool>, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        if operations.by_id.contains_key(operation_id) {
            return Err(SlackFileStagingOperationFailure::DuplicateOperation {
                operation_id: operation_id.clone(),
            });
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        operations.by_id.insert(
            operation_id.clone(),
            SlackFileStagingOperation {
                cancellation: Arc::clone(&cancellation),
                state: SlackFileStagingState::Preparing,
            },
        );
        Ok(cancellation)
    }

    pub(super) fn begin_allocation(
        &self,
        operation_id: &SlackFileStagingOperationId,
        attempt: SlackFileStagingAttempt,
    ) -> Result<SlackBeginAllocation, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        let cancelled = operation.cancellation.load(Ordering::Acquire);
        match (&operation.state, attempt, cancelled) {
            (SlackFileStagingState::Preparing, SlackFileStagingAttempt::Initial, false) => {
                operation.state = SlackFileStagingState::AllocatingFirst;
                Ok(SlackBeginAllocation::Ready)
            }
            (
                SlackFileStagingState::RetryingWithoutAllocation,
                SlackFileStagingAttempt::Retry,
                false,
            ) => {
                operation.state = SlackFileStagingState::AllocatingRetryWithoutAllocation;
                Ok(SlackBeginAllocation::Ready)
            }
            (
                SlackFileStagingState::RetryingWithAllocations { attempts },
                SlackFileStagingAttempt::Retry,
                false,
            ) => {
                operation.state = SlackFileStagingState::AllocatingRetryWithAllocations {
                    attempts: attempts.clone(),
                };
                Ok(SlackBeginAllocation::Ready)
            }
            (
                SlackFileStagingState::Preparing | SlackFileStagingState::RetryingWithoutAllocation,
                _,
                true,
            ) => {
                operation.state = SlackFileStagingState::CancelledBeforeAllocation;
                Ok(SlackBeginAllocation::CancelledWithoutAllocation)
            }
            (
                SlackFileStagingState::RetryingWithAllocations { attempts },
                SlackFileStagingAttempt::Retry,
                true,
            ) => {
                operation.state = SlackFileStagingState::CancelledBeforeCompletion {
                    attempts: attempts.clone(),
                };
                Ok(SlackBeginAllocation::CancelledWithAllocation)
            }
            _ => Err(invalid_state(operation_id, "begin allocation")),
        }
    }

    pub(super) fn record_allocation(
        &self,
        operation_id: &SlackFileStagingOperationId,
        attempt: SlackFileStagingAttempt,
        allocated: SlackAllocatedUpload,
    ) -> Result<SlackRecordedAllocation, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        let (attempts, duplicate) = match (&operation.state, attempt) {
            (SlackFileStagingState::AllocatingFirst, SlackFileStagingAttempt::Initial)
            | (
                SlackFileStagingState::AllocatingRetryWithoutAllocation,
                SlackFileStagingAttempt::Retry,
            ) => (SlackFileStagingAllocatedAttempts::new(allocated), false),
            (
                SlackFileStagingState::AllocatingRetryWithAllocations { attempts },
                SlackFileStagingAttempt::Retry,
            ) => {
                let duplicate = attempts
                    .iter()
                    .any(|prior| prior.file_id() == allocated.file_id());
                let mut attempts = attempts.clone();
                attempts.push(allocated);
                (attempts, duplicate)
            }
            _ => return Err(invalid_state(operation_id, "record allocation")),
        };
        if operation.cancellation.load(Ordering::Acquire) {
            operation.state = SlackFileStagingState::CancelledBeforeCompletion { attempts };
            return Ok(SlackRecordedAllocation::Cancelled);
        }
        operation.state = SlackFileStagingState::Transferring { attempt, attempts };
        Ok(if duplicate {
            SlackRecordedAllocation::Duplicate
        } else {
            SlackRecordedAllocation::Ready
        })
    }

    pub(super) fn precompletion_failure(
        &self,
        operation_id: &SlackFileStagingOperationId,
        attempt: SlackFileStagingAttempt,
        message: String,
    ) -> SlackPrecompletionDisposition {
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => {
                return SlackPrecompletionDisposition::Finished(operation_outcome(failure));
            }
        };
        let operation = match operation_mut(&mut operations, operation_id) {
            Ok(operation) => operation,
            Err(failure) => {
                return SlackPrecompletionDisposition::Finished(operation_outcome(failure));
            }
        };
        if operation.cancellation.load(Ordering::Acquire) {
            cancelled_precompletion_failure(operation, operation_id)
        } else {
            active_precompletion_failure(operation, operation_id, attempt, message)
        }
    }

    pub(super) fn cancel_transfer(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        operation.cancellation.store(true, Ordering::Release);
        let SlackFileStagingState::Transferring { attempts, .. } = &operation.state else {
            return Err(invalid_state(operation_id, "cancel transfer"));
        };
        operation.state = SlackFileStagingState::CancelledBeforeCompletion {
            attempts: attempts.clone(),
        };
        Ok(())
    }

    pub(super) fn begin_completion(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> Result<SlackBeginCompletion, SlackFileStagingOperationFailure> {
        let mut operations = self.lock()?;
        let operation = operation_mut(&mut operations, operation_id)?;
        let SlackFileStagingState::Transferring { attempts, .. } = &operation.state else {
            return Err(invalid_state(operation_id, "begin completion"));
        };
        let attempts = attempts.clone();
        if operation.cancellation.load(Ordering::Acquire) {
            operation.state = SlackFileStagingState::CancelledBeforeCompletion { attempts };
            return Ok(SlackBeginCompletion::Cancelled);
        }
        operation.state = SlackFileStagingState::Completing {
            attempts: attempts.clone(),
        };
        Ok(SlackBeginCompletion::Ready(attempts))
    }

    pub(super) fn finish_completion(
        &self,
        operation_id: &SlackFileStagingOperationId,
        completion: SlackUploadCompletion,
    ) -> SlackFileStagingOutcome {
        let mut operations = match self.lock() {
            Ok(operations) => operations,
            Err(failure) => return operation_outcome(failure),
        };
        let operation = match operation_mut(&mut operations, operation_id) {
            Ok(operation) => operation,
            Err(failure) => return operation_outcome(failure),
        };
        let SlackFileStagingState::Completing { attempts } = &operation.state else {
            return operation_outcome(invalid_state(operation_id, "finish completion"));
        };
        let attempts = attempts.clone();
        match completion {
            SlackUploadCompletion::Accepted => {
                finish_accepted_completion(operation, operation_id, attempts)
            }
            SlackUploadCompletion::NotSent {
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::CompletionNotSent {
                    attempts: attempts.clone(),
                };
                SlackFileStagingOutcome::Failed(SlackFileStagingFailure::CompletionNotSent {
                    allocated_attempts: attempts,
                    diagnostic: diagnostic(message),
                })
            }
            SlackUploadCompletion::Rejected {
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::CompletionRejected {
                    attempts: attempts.clone(),
                };
                SlackFileStagingOutcome::Failed(SlackFileStagingFailure::CompletionRejected {
                    allocated_attempts: attempts,
                    diagnostic: diagnostic(message),
                })
            }
            SlackUploadCompletion::Unknown {
                diagnostic: message,
            } => {
                operation.state = SlackFileStagingState::CompletionUnknown {
                    attempts: attempts.clone(),
                };
                SlackFileStagingOutcome::Failed(SlackFileStagingFailure::CompletionUnknown {
                    allocated_attempts: attempts,
                    diagnostic: diagnostic(message),
                })
            }
        }
    }
}

fn finish_accepted_completion(
    operation: &mut SlackFileStagingOperation,
    operation_id: &SlackFileStagingOperationId,
    attempts: SlackFileStagingAllocatedAttempts,
) -> SlackFileStagingOutcome {
    let receipt = SlackStagedFile::new(operation_id.clone(), attempts.latest().clone());
    operation.state = SlackFileStagingState::Staged {
        attempts,
        receipt: receipt.clone(),
    };
    SlackFileStagingOutcome::Staged(receipt)
}

fn active_precompletion_failure(
    operation: &mut SlackFileStagingOperation,
    operation_id: &SlackFileStagingOperationId,
    attempt: SlackFileStagingAttempt,
    message: String,
) -> SlackPrecompletionDisposition {
    match (&operation.state, attempt) {
        (SlackFileStagingState::AllocatingFirst, SlackFileStagingAttempt::Initial) => {
            operation.state = SlackFileStagingState::RetryingWithoutAllocation;
            SlackPrecompletionDisposition::Retry
        }
        (
            SlackFileStagingState::AllocatingRetryWithoutAllocation,
            SlackFileStagingAttempt::Retry,
        ) => {
            operation.state = SlackFileStagingState::FailedBeforeAllocation;
            SlackPrecompletionDisposition::Finished(SlackFileStagingOutcome::Failed(
                SlackFileStagingFailure::RetryExhaustedBeforeAllocation {
                    diagnostic: diagnostic(message),
                },
            ))
        }
        (
            SlackFileStagingState::AllocatingRetryWithAllocations { attempts },
            SlackFileStagingAttempt::Retry,
        )
        | (
            SlackFileStagingState::Transferring {
                attempt: SlackFileStagingAttempt::Retry,
                attempts,
            },
            SlackFileStagingAttempt::Retry,
        ) => retry_exhausted_before_completion(operation, attempts.clone(), message),
        (
            SlackFileStagingState::Transferring {
                attempt: SlackFileStagingAttempt::Initial,
                attempts,
            },
            SlackFileStagingAttempt::Initial,
        ) => {
            operation.state = SlackFileStagingState::RetryingWithAllocations {
                attempts: attempts.clone(),
            };
            SlackPrecompletionDisposition::Retry
        }
        _ => SlackPrecompletionDisposition::Finished(operation_outcome(invalid_state(
            operation_id,
            "record pre-completion failure",
        ))),
    }
}

fn retry_exhausted_before_completion(
    operation: &mut SlackFileStagingOperation,
    attempts: SlackFileStagingAllocatedAttempts,
    message: String,
) -> SlackPrecompletionDisposition {
    operation.state = SlackFileStagingState::FailedBeforeCompletion {
        attempts: attempts.clone(),
    };
    SlackPrecompletionDisposition::Finished(SlackFileStagingOutcome::Failed(
        SlackFileStagingFailure::RetryExhaustedBeforeCompletion {
            allocated_attempts: attempts,
            diagnostic: diagnostic(message),
        },
    ))
}

fn cancelled_precompletion_failure(
    operation: &mut SlackFileStagingOperation,
    operation_id: &SlackFileStagingOperationId,
) -> SlackPrecompletionDisposition {
    match &operation.state {
        SlackFileStagingState::AllocatingFirst
        | SlackFileStagingState::AllocatingRetryWithoutAllocation => {
            operation.state = SlackFileStagingState::CancelledBeforeAllocation;
            SlackPrecompletionDisposition::CancelledWithoutAllocation
        }
        SlackFileStagingState::AllocatingRetryWithAllocations { attempts }
        | SlackFileStagingState::Transferring { attempts, .. } => {
            operation.state = SlackFileStagingState::CancelledBeforeCompletion {
                attempts: attempts.clone(),
            };
            SlackPrecompletionDisposition::CancelledWithAllocation
        }
        _ => SlackPrecompletionDisposition::Finished(operation_outcome(invalid_state(
            operation_id,
            "record pre-completion failure",
        ))),
    }
}
