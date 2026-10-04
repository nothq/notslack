mod state;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::model::{
    SlackAllocatedUpload, SlackFileStagingAllocatedAttempts, SlackFileStagingCancellationOutcome,
    SlackFileStagingOperationId, SlackFileStagingOutcome, SlackFileUploadMetadata, SlackUploadFile,
};

use crate::live::api::{SlackApiClient, SlackObservedUpload};

use super::{
    allocate_upload, complete_upload, operation_outcome, SlackFileStagingAttempt,
    SlackFileStagingLedger, SlackUploadAllocation,
};

enum SlackBeginAllocation {
    Ready,
    CancelledWithoutAllocation,
    CancelledWithAllocation,
}

enum SlackRecordedAllocation {
    Ready,
    Duplicate,
    Cancelled,
}

enum SlackBeginCompletion {
    Ready(SlackFileStagingAllocatedAttempts),
    Cancelled,
}

enum SlackPrecompletionDisposition {
    Retry,
    Finished(SlackFileStagingOutcome),
    CancelledWithoutAllocation,
    CancelledWithAllocation,
}

#[derive(Clone, Copy)]
struct SlackStageAttemptContext<'a> {
    api: &'a SlackApiClient,
    operation_id: &'a SlackFileStagingOperationId,
    attempt: SlackFileStagingAttempt,
    cancellation: &'a Arc<AtomicBool>,
}

enum SlackStageAttemptOutcome {
    Retry,
    Finished(SlackFileStagingOutcome),
}

enum SlackStageAllocation {
    Ready {
        allocated: SlackAllocatedUpload,
        upload_url: String,
    },
    Retry,
    Finished(SlackFileStagingOutcome),
}

impl From<SlackStageAttemptOutcome> for SlackStageAllocation {
    fn from(outcome: SlackStageAttemptOutcome) -> Self {
        match outcome {
            SlackStageAttemptOutcome::Retry => Self::Retry,
            SlackStageAttemptOutcome::Finished(outcome) => Self::Finished(outcome),
        }
    }
}

enum SlackStageProgress {
    Ready,
    Retry,
    Finished(SlackFileStagingOutcome),
}

impl From<SlackStageAttemptOutcome> for SlackStageProgress {
    fn from(outcome: SlackStageAttemptOutcome) -> Self {
        match outcome {
            SlackStageAttemptOutcome::Retry => Self::Retry,
            SlackStageAttemptOutcome::Finished(outcome) => Self::Finished(outcome),
        }
    }
}

impl SlackStageProgress {
    fn into_attempt_outcome(self) -> Option<SlackStageAttemptOutcome> {
        match self {
            Self::Ready => None,
            Self::Retry => Some(SlackStageAttemptOutcome::Retry),
            Self::Finished(outcome) => Some(SlackStageAttemptOutcome::Finished(outcome)),
        }
    }
}

impl SlackFileStagingLedger {
    pub(super) fn stage(
        &self,
        api: &SlackApiClient,
        operation_id: &SlackFileStagingOperationId,
        file: SlackUploadFile,
    ) -> SlackFileStagingOutcome {
        let cancellation = match self.register(operation_id) {
            Ok(cancellation) => cancellation,
            Err(failure) => return operation_outcome(failure),
        };
        let metadata = file.metadata().clone();
        let mut attempt = SlackFileStagingAttempt::Initial;
        loop {
            let context = SlackStageAttemptContext {
                api,
                operation_id,
                attempt,
                cancellation: &cancellation,
            };
            match self.run_stage_attempt(context, &file, &metadata) {
                SlackStageAttemptOutcome::Retry => attempt = SlackFileStagingAttempt::Retry,
                SlackStageAttemptOutcome::Finished(outcome) => return outcome,
            }
        }
    }

    fn run_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
        file: &SlackUploadFile,
        metadata: &SlackFileUploadMetadata,
    ) -> SlackStageAttemptOutcome {
        if let Some(outcome) = self.begin_stage_attempt(context) {
            return SlackStageAttemptOutcome::Finished(outcome);
        }
        let (allocated, upload_url) = match self.allocate_stage_attempt(context, metadata) {
            SlackStageAllocation::Ready {
                allocated,
                upload_url,
            } => (allocated, upload_url),
            SlackStageAllocation::Retry => return SlackStageAttemptOutcome::Retry,
            SlackStageAllocation::Finished(outcome) => {
                return SlackStageAttemptOutcome::Finished(outcome);
            }
        };
        if let Some(outcome) = self
            .record_stage_attempt(context, allocated)
            .into_attempt_outcome()
        {
            return outcome;
        }
        if let Some(outcome) = self
            .transfer_stage_attempt(context, file, &upload_url)
            .into_attempt_outcome()
        {
            return outcome;
        }
        SlackStageAttemptOutcome::Finished(self.complete_stage_attempt(context))
    }

    fn begin_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
    ) -> Option<SlackFileStagingOutcome> {
        match self.begin_allocation(context.operation_id, context.attempt) {
            Ok(SlackBeginAllocation::Ready) => None,
            Ok(SlackBeginAllocation::CancelledWithoutAllocation) => {
                Some(SlackFileStagingOutcome::Cancelled(
                    SlackFileStagingCancellationOutcome::NoPersistentFile,
                ))
            }
            Ok(SlackBeginAllocation::CancelledWithAllocation) => Some(
                SlackFileStagingOutcome::Cancelled(self.cancel(context.api, context.operation_id)),
            ),
            Err(failure) => Some(operation_outcome(failure)),
        }
    }

    fn allocate_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
        metadata: &SlackFileUploadMetadata,
    ) -> SlackStageAllocation {
        match allocate_upload(context.api, metadata) {
            SlackUploadAllocation::Ready {
                allocated,
                upload_url,
            } => SlackStageAllocation::Ready {
                allocated,
                upload_url,
            },
            SlackUploadAllocation::AllocatedWithoutUploadUrl {
                allocated,
                diagnostic,
            } => {
                match self.record_allocation(context.operation_id, context.attempt, allocated) {
                    Ok(SlackRecordedAllocation::Ready | SlackRecordedAllocation::Duplicate) => {}
                    Ok(SlackRecordedAllocation::Cancelled) => {
                        return SlackStageAllocation::Finished(SlackFileStagingOutcome::Cancelled(
                            self.cancel(context.api, context.operation_id),
                        ));
                    }
                    Err(failure) => {
                        return SlackStageAllocation::Finished(operation_outcome(failure));
                    }
                }
                self.precompletion_outcome(context, diagnostic).into()
            }
            SlackUploadAllocation::Failed { diagnostic } => {
                self.precompletion_outcome(context, diagnostic).into()
            }
        }
    }

    fn record_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
        allocated: SlackAllocatedUpload,
    ) -> SlackStageProgress {
        match self.record_allocation(context.operation_id, context.attempt, allocated) {
            Ok(SlackRecordedAllocation::Ready) => SlackStageProgress::Ready,
            Ok(SlackRecordedAllocation::Duplicate) => self
                .precompletion_outcome(
                    context,
                    "Slack files.getUploadURLExternal reused an earlier upload ticket".to_string(),
                )
                .into(),
            Ok(SlackRecordedAllocation::Cancelled) => SlackStageProgress::Finished(
                SlackFileStagingOutcome::Cancelled(self.cancel(context.api, context.operation_id)),
            ),
            Err(failure) => SlackStageProgress::Finished(operation_outcome(failure)),
        }
    }

    fn transfer_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
        file: &SlackUploadFile,
        upload_url: &str,
    ) -> SlackStageProgress {
        match context
            .api
            .upload_file_observed(upload_url, file, Arc::clone(context.cancellation))
        {
            SlackObservedUpload::Transferred => SlackStageProgress::Ready,
            SlackObservedUpload::Cancelled => {
                if let Err(failure) = self.cancel_transfer(context.operation_id) {
                    return SlackStageProgress::Finished(operation_outcome(failure));
                }
                SlackStageProgress::Finished(SlackFileStagingOutcome::Cancelled(
                    self.cancel(context.api, context.operation_id),
                ))
            }
            SlackObservedUpload::Rejected { diagnostic }
            | SlackObservedUpload::Unknown { diagnostic } => {
                self.precompletion_outcome(context, diagnostic).into()
            }
        }
    }

    fn precompletion_outcome(
        &self,
        context: SlackStageAttemptContext<'_>,
        diagnostic: String,
    ) -> SlackStageAttemptOutcome {
        match self.precompletion_failure(context.operation_id, context.attempt, diagnostic) {
            SlackPrecompletionDisposition::Retry => SlackStageAttemptOutcome::Retry,
            SlackPrecompletionDisposition::Finished(outcome) => {
                SlackStageAttemptOutcome::Finished(outcome)
            }
            SlackPrecompletionDisposition::CancelledWithoutAllocation => {
                SlackStageAttemptOutcome::Finished(SlackFileStagingOutcome::Cancelled(
                    SlackFileStagingCancellationOutcome::NoPersistentFile,
                ))
            }
            SlackPrecompletionDisposition::CancelledWithAllocation => {
                SlackStageAttemptOutcome::Finished(SlackFileStagingOutcome::Cancelled(
                    self.cancel(context.api, context.operation_id),
                ))
            }
        }
    }

    fn complete_stage_attempt(
        &self,
        context: SlackStageAttemptContext<'_>,
    ) -> SlackFileStagingOutcome {
        let attempts = match self.begin_completion(context.operation_id) {
            Ok(SlackBeginCompletion::Ready(attempts)) => attempts,
            Ok(SlackBeginCompletion::Cancelled) => {
                return SlackFileStagingOutcome::Cancelled(
                    self.cancel(context.api, context.operation_id),
                );
            }
            Err(failure) => return operation_outcome(failure),
        };
        let outcome = self.finish_completion(
            context.operation_id,
            complete_upload(context.api, &attempts),
        );
        if context.cancellation.load(Ordering::Acquire) {
            SlackFileStagingOutcome::Cancelled(self.cancel(context.api, context.operation_id))
        } else {
            outcome
        }
    }
}
