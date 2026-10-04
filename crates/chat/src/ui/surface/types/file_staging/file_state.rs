use super::{
    SlackComposerFileFailure, SlackComposerFileRecovery, SlackFileStagingCancellationOutcome,
    SlackFileStagingFailure, SlackFileStagingOperationId, SlackFileStagingOutcome,
    SlackRemoteDraftFileReference, SlackRetainedFileOwnership, SlackStagedFile, SlackUploadFile,
};

#[derive(Clone, Debug)]
pub(crate) enum SlackComposerFileState {
    Queued {
        operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
    },
    Uploading {
        operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
    },
    Staged {
        staged: SlackStagedFile,
    },
    Error {
        operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
        failure: SlackComposerFileFailure,
    },
    Recovering {
        upload: SlackUploadFile,
        recovery: SlackComposerFileRecovery,
    },
    RetainedUnknown {
        operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
        ownership: SlackRetainedFileOwnership,
    },
    RetryBlocked {
        prior_operation_id: SlackFileStagingOperationId,
        pending_retry_operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
        ownership: SlackRetainedFileOwnership,
    },
    AlreadyShared {
        operation_id: SlackFileStagingOperationId,
    },
    AlreadyDraftOwned {
        operation_id: SlackFileStagingOperationId,
    },
    RemoteLoading {
        reference: SlackRemoteDraftFileReference,
    },
    RemoteReady {
        reference: SlackRemoteDraftFileReference,
    },
    RemoteError {
        reference: SlackRemoteDraftFileReference,
        diagnostic: String,
    },
    #[cfg(test)]
    Fixture,
}

impl SlackComposerFileState {
    pub(crate) fn operation_id(&self) -> Option<&SlackFileStagingOperationId> {
        match self {
            Self::Queued { operation_id, .. }
            | Self::Uploading { operation_id, .. }
            | Self::Error { operation_id, .. }
            | Self::RetainedUnknown { operation_id, .. } => Some(operation_id),
            Self::Staged { staged, .. } => Some(staged.operation_id()),
            Self::Recovering { recovery, .. } => Some(recovery.prior_operation_id()),
            Self::RetryBlocked {
                prior_operation_id, ..
            } => Some(prior_operation_id),
            Self::AlreadyShared { operation_id, .. }
            | Self::AlreadyDraftOwned { operation_id, .. } => Some(operation_id),
            Self::RemoteLoading { .. } | Self::RemoteReady { .. } | Self::RemoteError { .. } => {
                None
            }
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn staged(&self) -> Option<&SlackStagedFile> {
        match self {
            Self::Staged { staged, .. } => Some(staged),
            Self::Queued { .. }
            | Self::Uploading { .. }
            | Self::Error { .. }
            | Self::Recovering { .. }
            | Self::RetainedUnknown { .. }
            | Self::RetryBlocked { .. }
            | Self::AlreadyShared { .. }
            | Self::AlreadyDraftOwned { .. }
            | Self::RemoteLoading { .. }
            | Self::RemoteReady { .. }
            | Self::RemoteError { .. } => None,
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn projected_slack_file_id(&self) -> Option<&crate::model::SlackFileId> {
        match self {
            Self::Staged { staged, .. } => Some(staged.file_id()),
            Self::RemoteLoading { reference }
            | Self::RemoteReady { reference }
            | Self::RemoteError { reference, .. } => Some(reference.file_id()),
            Self::Queued { .. }
            | Self::Uploading { .. }
            | Self::Error { .. }
            | Self::Recovering { .. }
            | Self::RetainedUnknown { .. }
            | Self::RetryBlocked { .. }
            | Self::AlreadyShared { .. }
            | Self::AlreadyDraftOwned { .. } => None,
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn summary_status(&self) -> crate::model::ChatComposerFileStatus {
        match self {
            Self::Queued { .. } => crate::model::ChatComposerFileStatus::Queued,
            Self::Uploading { .. } | Self::Recovering { .. } => {
                crate::model::ChatComposerFileStatus::Uploading
            }
            Self::Staged { .. } => crate::model::ChatComposerFileStatus::Complete,
            Self::Error { .. } | Self::AlreadyShared { .. } | Self::AlreadyDraftOwned { .. } => {
                crate::model::ChatComposerFileStatus::Error
            }
            Self::RetainedUnknown { .. } | Self::RetryBlocked { .. } => {
                crate::model::ChatComposerFileStatus::RetainedUnknown
            }
            Self::RemoteLoading { .. } => crate::model::ChatComposerFileStatus::RemoteLoading,
            Self::RemoteReady { .. } => crate::model::ChatComposerFileStatus::RemoteReady,
            Self::RemoteError { .. } => crate::model::ChatComposerFileStatus::RemoteError,
            #[cfg(test)]
            Self::Fixture => crate::model::ChatComposerFileStatus::Complete,
        }
    }

    pub(crate) fn diagnostic(&self) -> Option<String> {
        match self {
            Self::Error { failure, .. } => Some(match failure {
                SlackComposerFileFailure::NotSubmitted(diagnostic) => diagnostic.to_string(),
                SlackComposerFileFailure::Staging(failure) => staging_failure_diagnostic(failure),
                SlackComposerFileFailure::Cancelled(outcome) => {
                    cancellation_outcome_diagnostic(outcome)
                }
            }),
            Self::RetainedUnknown { ownership, .. } => Some(ownership.diagnostic()),
            Self::RetryBlocked { ownership, .. } => Some(ownership.diagnostic()),
            Self::AlreadyShared { .. } => {
                Some("Slack reports that this staged file has already been shared.".to_string())
            }
            Self::AlreadyDraftOwned { .. } => {
                Some("Slack reports that this staged file is already owned by a draft.".to_string())
            }
            Self::RemoteError { diagnostic, .. } => Some(diagnostic.clone()),
            Self::Queued { .. }
            | Self::Uploading { .. }
            | Self::Staged { .. }
            | Self::Recovering { .. }
            | Self::RemoteLoading { .. }
            | Self::RemoteReady { .. } => None,
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn pending_retry_operation_id(&self) -> Option<&SlackFileStagingOperationId> {
        match self {
            Self::Recovering { recovery, .. } => Some(recovery.next_operation_id()),
            Self::RetryBlocked {
                pending_retry_operation_id,
                ..
            } => Some(pending_retry_operation_id),
            Self::Queued { .. }
            | Self::Uploading { .. }
            | Self::Staged { .. }
            | Self::Error { .. }
            | Self::RetainedUnknown { .. }
            | Self::AlreadyShared { .. }
            | Self::AlreadyDraftOwned { .. }
            | Self::RemoteLoading { .. }
            | Self::RemoteReady { .. }
            | Self::RemoteError { .. } => None,
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn remote_reference(&self) -> Option<&SlackRemoteDraftFileReference> {
        match self {
            Self::RemoteLoading { reference }
            | Self::RemoteReady { reference }
            | Self::RemoteError { reference, .. } => Some(reference),
            Self::Queued { .. }
            | Self::Uploading { .. }
            | Self::Staged { .. }
            | Self::Error { .. }
            | Self::Recovering { .. }
            | Self::RetainedUnknown { .. }
            | Self::RetryBlocked { .. }
            | Self::AlreadyShared { .. }
            | Self::AlreadyDraftOwned { .. } => None,
            #[cfg(test)]
            Self::Fixture => None,
        }
    }

    pub(crate) fn from_staging_outcome(
        operation_id: SlackFileStagingOperationId,
        upload: SlackUploadFile,
        outcome: SlackFileStagingOutcome,
    ) -> Self {
        match outcome {
            SlackFileStagingOutcome::Staged(staged) => Self::Staged { staged },
            SlackFileStagingOutcome::Failed(SlackFileStagingFailure::CompletionUnknown {
                allocated_attempts,
                diagnostic,
            }) => Self::RetainedUnknown {
                operation_id,
                upload,
                ownership: SlackRetainedFileOwnership::CompletionUnknown {
                    allocated_attempts,
                    diagnostic,
                },
            },
            SlackFileStagingOutcome::Failed(failure) => Self::Error {
                operation_id,
                upload,
                failure: SlackComposerFileFailure::Staging(failure),
            },
            SlackFileStagingOutcome::Cancelled(
                SlackFileStagingCancellationOutcome::RetainedUnknown {
                    allocated_attempts,
                    diagnostic,
                },
            ) => Self::RetainedUnknown {
                operation_id,
                upload,
                ownership: SlackRetainedFileOwnership::CancellationUnknown {
                    allocated_attempts,
                    diagnostic,
                },
            },
            SlackFileStagingOutcome::Cancelled(SlackFileStagingCancellationOutcome::Pending) => {
                Self::RetainedUnknown {
                    operation_id,
                    upload,
                    ownership: SlackRetainedFileOwnership::CancellationPending,
                }
            }
            SlackFileStagingOutcome::Cancelled(SlackFileStagingCancellationOutcome::Shared) => {
                Self::AlreadyShared { operation_id }
            }
            SlackFileStagingOutcome::Cancelled(SlackFileStagingCancellationOutcome::DraftOwned) => {
                Self::AlreadyDraftOwned { operation_id }
            }
            SlackFileStagingOutcome::Cancelled(outcome) => Self::Error {
                operation_id,
                upload,
                failure: SlackComposerFileFailure::Cancelled(outcome),
            },
        }
    }
}

fn staging_failure_diagnostic(failure: &SlackFileStagingFailure) -> String {
    match failure {
        SlackFileStagingFailure::InvalidUpload { diagnostic }
        | SlackFileStagingFailure::RetryExhaustedBeforeAllocation { diagnostic }
        | SlackFileStagingFailure::RetryExhaustedBeforeCompletion { diagnostic, .. }
        | SlackFileStagingFailure::CompletionNotSent { diagnostic, .. }
        | SlackFileStagingFailure::CompletionRejected { diagnostic, .. }
        | SlackFileStagingFailure::CompletionUnknown { diagnostic, .. } => diagnostic.to_string(),
        SlackFileStagingFailure::Operation(failure) => failure.to_string(),
    }
}

pub(super) fn cancellation_outcome_diagnostic(
    outcome: &SlackFileStagingCancellationOutcome,
) -> String {
    match outcome {
        SlackFileStagingCancellationOutcome::Pending => {
            "Slack is still cancelling this file-staging operation.".to_string()
        }
        SlackFileStagingCancellationOutcome::NoPersistentFile => {
            "Slack cancelled this file before it became persistent.".to_string()
        }
        SlackFileStagingCancellationOutcome::ConfirmedDeleted => {
            "Slack cancelled and deleted this staged file.".to_string()
        }
        SlackFileStagingCancellationOutcome::Shared => {
            "Slack reports that this staged file has already been shared.".to_string()
        }
        SlackFileStagingCancellationOutcome::DraftOwned => {
            "Slack reports that this staged file is already owned by a draft.".to_string()
        }
        SlackFileStagingCancellationOutcome::RetainedUnknown { diagnostic, .. } => {
            diagnostic.to_string()
        }
        SlackFileStagingCancellationOutcome::Failed(failure) => failure.to_string(),
    }
}

pub(super) fn staging_outcome_diagnostic(outcome: &SlackFileStagingOutcome) -> Option<String> {
    match outcome {
        SlackFileStagingOutcome::Staged(_) => None,
        SlackFileStagingOutcome::Failed(failure) => Some(staging_failure_diagnostic(failure)),
        SlackFileStagingOutcome::Cancelled(cancellation) => {
            Some(cancellation_outcome_diagnostic(cancellation))
        }
    }
}
