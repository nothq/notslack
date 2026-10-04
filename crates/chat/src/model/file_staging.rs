use crate::model::{SlackAllocatedUpload, SlackFileStagingOperationId, SlackStagedFile};

/// Human-readable context for a typed file-staging state.
///
/// This value is display-only. Callers must branch on the surrounding enum variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileStagingDiagnostic(String);

impl SlackFileStagingDiagnostic {
    pub fn new(message: String) -> Self {
        Self(message)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackFileStagingDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileStagingAllocatedAttempts {
    first: SlackAllocatedUpload,
    subsequent: Vec<SlackAllocatedUpload>,
}

impl SlackFileStagingAllocatedAttempts {
    pub fn new(first: SlackAllocatedUpload) -> Self {
        Self {
            first,
            subsequent: Vec::new(),
        }
    }

    pub fn push(&mut self, allocated: SlackAllocatedUpload) {
        self.subsequent.push(allocated);
    }

    pub fn iter(&self) -> impl Iterator<Item = &SlackAllocatedUpload> {
        std::iter::once(&self.first).chain(self.subsequent.iter())
    }

    pub fn latest(&self) -> &SlackAllocatedUpload {
        self.subsequent.last().unwrap_or(&self.first)
    }
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingOperationFailure {
    Unavailable {
        diagnostic: SlackFileStagingDiagnostic,
    },
    DuplicateOperation {
        operation_id: SlackFileStagingOperationId,
    },
    UnknownOperation {
        operation_id: SlackFileStagingOperationId,
    },
    InvalidState {
        operation_id: SlackFileStagingOperationId,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Internal {
        diagnostic: SlackFileStagingDiagnostic,
    },
}

impl std::fmt::Display for SlackFileStagingOperationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable { diagnostic } | Self::Internal { diagnostic } => {
                diagnostic.fmt(formatter)
            }
            Self::DuplicateOperation { operation_id } => {
                write!(
                    formatter,
                    "Slack file-staging operation {operation_id} already exists"
                )
            }
            Self::UnknownOperation { operation_id } => {
                write!(
                    formatter,
                    "Slack file-staging operation {operation_id} does not exist"
                )
            }
            Self::InvalidState {
                operation_id,
                diagnostic,
            } => write!(formatter, "{diagnostic}: {operation_id}"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingFailure {
    InvalidUpload {
        diagnostic: SlackFileStagingDiagnostic,
    },
    RetryExhaustedBeforeAllocation {
        diagnostic: SlackFileStagingDiagnostic,
    },
    RetryExhaustedBeforeCompletion {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CompletionNotSent {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CompletionRejected {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    CompletionUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Operation(SlackFileStagingOperationFailure),
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingOutcome {
    Staged(SlackStagedFile),
    Failed(SlackFileStagingFailure),
    Cancelled(SlackFileStagingCancellationOutcome),
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingCancellationOutcome {
    Pending,
    NoPersistentFile,
    ConfirmedDeleted,
    Shared,
    DraftOwned,
    RetainedUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Failed(SlackFileStagingOperationFailure),
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingReconcileOutcome {
    Staged(SlackStagedFile),
    StillUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Cancelled(SlackFileStagingCancellationOutcome),
    Failed(SlackFileStagingOperationFailure),
}

#[derive(Clone, Debug)]
pub enum SlackFileStagingCleanupOutcome {
    Pending,
    NoPersistentFile,
    ConfirmedDeleted,
    Shared,
    DraftOwned,
    RetainedUnknown {
        allocated_attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Failed(SlackFileStagingOperationFailure),
}

pub(crate) fn unavailable_file_staging_operation(
    operation: &'static str,
) -> SlackFileStagingOperationFailure {
    SlackFileStagingOperationFailure::Unavailable {
        diagnostic: SlackFileStagingDiagnostic::new(format!(
            "Slack file-staging {operation} is unavailable for this workspace runtime"
        )),
    }
}
