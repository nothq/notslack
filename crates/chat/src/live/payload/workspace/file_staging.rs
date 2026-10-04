mod cleanup;
mod core;
mod draft;
mod lifecycle;
mod loader;
mod share;
mod transport;
mod upload;

use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use crate::model::{
    SlackAllocatedUpload, SlackFileId, SlackFileStagingOperationId, SlackStagedFile,
};
use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingDiagnostic, SlackFileStagingOperationFailure,
};
use core::{
    diagnostic, ensure_draft_reservation_targets_latest, ensure_receipt_is_latest,
    internal_failure, invalid_state, operation_mut, operation_outcome,
};
use transport::{
    allocate_upload, complete_upload, observe_file_info, SlackFileInfoObservation,
    SlackUploadAllocation, SlackUploadCompletion,
};

#[derive(Clone, Default)]
pub(super) struct SlackFileStagingLedger {
    operations: Arc<Mutex<SlackFileStagingOperations>>,
}

#[derive(Default)]
struct SlackFileStagingOperations {
    by_id: HashMap<SlackFileStagingOperationId, SlackFileStagingOperation>,
}

struct SlackFileStagingOperation {
    cancellation: Arc<AtomicBool>,
    state: SlackFileStagingState,
}

#[derive(Clone)]
enum SlackFileStagingState {
    Preparing,
    AllocatingFirst,
    RetryingWithoutAllocation,
    AllocatingRetryWithoutAllocation,
    RetryingWithAllocations {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    AllocatingRetryWithAllocations {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    Transferring {
        attempt: SlackFileStagingAttempt,
        attempts: SlackFileStagingAllocatedAttempts,
    },
    Completing {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    FailedBeforeAllocation,
    FailedBeforeCompletion {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    CancelledBeforeAllocation,
    CancelledBeforeCompletion {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    CompletionNotSent {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    CompletionRejected {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    CompletionUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    Staged {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
    },
    Reconciling {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    Cleaning {
        attempts: SlackFileStagingAllocatedAttempts,
        targets: SlackOwnedCleanupTargets,
    },
    CleanupUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        unresolved: SlackOwnedCleanupTargets,
    },
    Sharing {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
    },
    ShareUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
    },
    ShareUnknownCleaning {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        targets: SlackOwnedCleanupTargets,
    },
    ShareUnknownCleanupUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        unresolved: SlackOwnedCleanupTargets,
    },
    SharedCleaning {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        targets: SlackOwnedCleanupTargets,
    },
    SharedCleanupUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        unresolved: SlackOwnedCleanupTargets,
    },
    Drafting {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
    },
    DraftUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
    },
    DraftUnknownCleaning {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        targets: SlackOwnedCleanupTargets,
    },
    DraftUnknownCleanupUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        unresolved: SlackOwnedCleanupTargets,
    },
    DraftOwnedCleaning {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        targets: SlackOwnedCleanupTargets,
    },
    DraftOwnedCleanupUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        unresolved: SlackOwnedCleanupTargets,
    },
    Deleted {
        attempts: SlackFileStagingAllocatedAttempts,
    },
    Shared {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
    },
    DraftOwned {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
    },
}

impl SlackFileStagingState {
    fn allocated_attempts(&self) -> Option<&SlackFileStagingAllocatedAttempts> {
        match self {
            Self::Preparing
            | Self::AllocatingFirst
            | Self::RetryingWithoutAllocation
            | Self::AllocatingRetryWithoutAllocation
            | Self::FailedBeforeAllocation
            | Self::CancelledBeforeAllocation => None,
            Self::RetryingWithAllocations { attempts }
            | Self::AllocatingRetryWithAllocations { attempts }
            | Self::Transferring { attempts, .. }
            | Self::Completing { attempts }
            | Self::FailedBeforeCompletion { attempts }
            | Self::CancelledBeforeCompletion { attempts }
            | Self::CompletionNotSent { attempts }
            | Self::CompletionRejected { attempts }
            | Self::CompletionUnknown { attempts }
            | Self::Staged { attempts, .. }
            | Self::Reconciling { attempts }
            | Self::Cleaning { attempts, .. }
            | Self::CleanupUnknown { attempts, .. }
            | Self::Sharing { attempts, .. }
            | Self::ShareUnknown { attempts, .. }
            | Self::ShareUnknownCleaning { attempts, .. }
            | Self::ShareUnknownCleanupUnknown { attempts, .. }
            | Self::SharedCleaning { attempts, .. }
            | Self::SharedCleanupUnknown { attempts, .. }
            | Self::Drafting { attempts, .. }
            | Self::DraftUnknown { attempts, .. }
            | Self::DraftUnknownCleaning { attempts, .. }
            | Self::DraftUnknownCleanupUnknown { attempts, .. }
            | Self::DraftOwnedCleaning { attempts, .. }
            | Self::DraftOwnedCleanupUnknown { attempts, .. }
            | Self::Deleted { attempts }
            | Self::Shared { attempts, .. }
            | Self::DraftOwned { attempts, .. } => Some(attempts),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlackFileStagingAttempt {
    Initial,
    Retry,
}

#[derive(Clone, PartialEq, Eq)]
struct SlackOwnedCleanupTargets {
    first: SlackFileId,
    subsequent: Vec<SlackFileId>,
}

impl SlackOwnedCleanupTargets {
    fn from_attempts(
        attempts: &SlackFileStagingAllocatedAttempts,
        protected_file_id: Option<&SlackFileId>,
    ) -> Option<Self> {
        let mut seen = HashSet::new();
        let targets = attempts
            .iter()
            .map(SlackAllocatedUpload::file_id)
            .filter(|file_id| protected_file_id != Some(*file_id))
            .filter(|file_id| seen.insert((*file_id).clone()))
            .cloned()
            .collect::<Vec<_>>();
        Self::from_vec(targets)
    }

    fn from_vec(mut targets: Vec<SlackFileId>) -> Option<Self> {
        if targets.is_empty() {
            return None;
        }
        let first = targets.remove(0);
        Some(Self {
            first,
            subsequent: targets,
        })
    }

    fn iter(&self) -> impl Iterator<Item = &SlackFileId> {
        std::iter::once(&self.first).chain(self.subsequent.iter())
    }
}

pub(crate) struct SlackFileShareReservation {
    operation_ids: Vec<SlackFileStagingOperationId>,
}

pub(crate) struct SlackFileDraftReservation {
    operation_ids: Vec<SlackFileStagingOperationId>,
}

struct SlackSharedCleanupJob {
    operation_id: SlackFileStagingOperationId,
    attempts: SlackFileStagingAllocatedAttempts,
    receipt: SlackStagedFile,
    targets: SlackOwnedCleanupTargets,
}

struct SlackShareUnknownCleanupJob {
    operation_id: SlackFileStagingOperationId,
    attempts: SlackFileStagingAllocatedAttempts,
    receipt: SlackStagedFile,
    diagnostic: SlackFileStagingDiagnostic,
    targets: SlackOwnedCleanupTargets,
}

struct SlackDraftOwnedCleanupJob {
    operation_id: SlackFileStagingOperationId,
    attempts: SlackFileStagingAllocatedAttempts,
    receipt: SlackStagedFile,
    targets: SlackOwnedCleanupTargets,
}

struct SlackDraftUnknownCleanupJob {
    operation_id: SlackFileStagingOperationId,
    attempts: SlackFileStagingAllocatedAttempts,
    receipt: SlackStagedFile,
    diagnostic: SlackFileStagingDiagnostic,
    targets: SlackOwnedCleanupTargets,
}

impl SlackFileStagingLedger {
    fn lock(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, SlackFileStagingOperations>,
        SlackFileStagingOperationFailure,
    > {
        self.operations
            .lock()
            .map_err(|_| internal_failure("Slack file-staging operation ledger mutex poisoned"))
    }
}
