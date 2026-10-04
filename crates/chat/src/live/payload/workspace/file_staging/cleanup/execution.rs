use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingDiagnostic, SlackStagedFile,
};

use crate::live::api::{SlackApiClient, SlackObservedApiPost};

use super::super::SlackOwnedCleanupTargets;

pub(super) enum SlackCleanupPreparation {
    Unshared {
        attempts: SlackFileStagingAllocatedAttempts,
        targets: SlackOwnedCleanupTargets,
    },
    Shared {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        targets: SlackOwnedCleanupTargets,
    },
    ShareUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        targets: SlackOwnedCleanupTargets,
    },
    RetainedShareUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    DraftOwned {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        targets: SlackOwnedCleanupTargets,
    },
    DraftUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        receipt: SlackStagedFile,
        diagnostic: SlackFileStagingDiagnostic,
        targets: SlackOwnedCleanupTargets,
    },
    RetainedDraftUnknown {
        attempts: SlackFileStagingAllocatedAttempts,
        diagnostic: SlackFileStagingDiagnostic,
    },
    Pending,
    NoPersistentFile,
    ConfirmedDeleted,
    AlreadyShared,
    AlreadyDraftOwned,
}

pub(super) enum SlackCleanupExecution {
    Confirmed,
    Unresolved {
        targets: SlackOwnedCleanupTargets,
        diagnostic: String,
    },
}

pub(super) fn delete_owned_files(
    api: &SlackApiClient,
    targets: &SlackOwnedCleanupTargets,
) -> SlackCleanupExecution {
    let mut unresolved = Vec::new();
    let mut first_diagnostic = None;
    for file_id in targets.iter() {
        let result = api.post_observed(
            "files.delete",
            &[
                ("file", file_id.as_str().to_string()),
                ("is_draft", true.to_string()),
            ],
        );
        match result {
            SlackObservedApiPost::Accepted(_) => {}
            SlackObservedApiPost::NotSent { diagnostic }
            | SlackObservedApiPost::Rejected { diagnostic }
            | SlackObservedApiPost::Unknown { diagnostic } => {
                unresolved.push(file_id.clone());
                if first_diagnostic.is_none() {
                    first_diagnostic = Some(diagnostic);
                }
            }
        }
    }
    match SlackOwnedCleanupTargets::from_vec(unresolved) {
        Some(targets) => SlackCleanupExecution::Unresolved {
            targets,
            diagnostic: first_diagnostic.unwrap_or_else(|| {
                "Slack staged-file cleanup retained unresolved operation-owned files".to_string()
            }),
        },
        None => SlackCleanupExecution::Confirmed,
    }
}
