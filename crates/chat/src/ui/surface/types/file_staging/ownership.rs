use super::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingCancellationOutcome,
    SlackFileStagingDiagnostic, SlackFileStagingFailure, SlackFileStagingOperationFailure,
    SlackFileStagingOperationId,
};

#[derive(Clone, Debug)]
pub(crate) enum SlackComposerFileFailure {
    NotSubmitted(SlackFileStagingDiagnostic),
    Staging(SlackFileStagingFailure),
    Cancelled(SlackFileStagingCancellationOutcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackFileRetryRecoveryKind {
    Reconcile,
    Cleanup,
}

#[derive(Clone, Debug)]
pub(crate) enum SlackRetainedFileOwnership {
    CompletionUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CancellationUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CleanupUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CancellationPending,
    CleanupPending,
    ReconciliationFailed(SlackFileStagingOperationFailure),
    CleanupFailed(SlackFileStagingOperationFailure),
}

impl SlackRetainedFileOwnership {
    pub(crate) fn retry_recovery_kind(&self) -> SlackFileRetryRecoveryKind {
        match self {
            Self::CompletionUnknown { .. } | Self::ReconciliationFailed(_) => {
                SlackFileRetryRecoveryKind::Reconcile
            }
            Self::CancellationUnknown { .. }
            | Self::CleanupUnknown { .. }
            | Self::CancellationPending
            | Self::CleanupPending
            | Self::CleanupFailed(_) => SlackFileRetryRecoveryKind::Cleanup,
        }
    }

    pub(crate) fn diagnostic(&self) -> String {
        match self {
            Self::CompletionUnknown {
                allocated_attempts,
                diagnostic,
            }
            | Self::CancellationUnknown {
                allocated_attempts,
                diagnostic,
            }
            | Self::CleanupUnknown {
                allocated_attempts,
                diagnostic,
            } => format!(
                "{diagnostic} Retained Slack file: {}.",
                allocated_attempts.latest().file_id()
            ),
            Self::CancellationPending => {
                "Slack is still cancelling this file-staging operation.".to_string()
            }
            Self::CleanupPending => {
                "Slack has not confirmed cleanup of this file-staging operation.".to_string()
            }
            Self::ReconciliationFailed(failure) => failure.to_string(),
            Self::CleanupFailed(failure) => failure.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SlackComposerFileRecovery {
    ReconcilingForRetry {
        prior_operation_id: SlackFileStagingOperationId,
        next_operation_id: SlackFileStagingOperationId,
    },
    CleaningForRetry {
        prior_operation_id: SlackFileStagingOperationId,
        next_operation_id: SlackFileStagingOperationId,
    },
}

impl SlackComposerFileRecovery {
    pub(crate) fn prior_operation_id(&self) -> &SlackFileStagingOperationId {
        match self {
            Self::ReconcilingForRetry {
                prior_operation_id, ..
            }
            | Self::CleaningForRetry {
                prior_operation_id, ..
            } => prior_operation_id,
        }
    }

    pub(crate) fn next_operation_id(&self) -> &SlackFileStagingOperationId {
        match self {
            Self::ReconcilingForRetry {
                next_operation_id, ..
            }
            | Self::CleaningForRetry {
                next_operation_id, ..
            } => next_operation_id,
        }
    }
}
