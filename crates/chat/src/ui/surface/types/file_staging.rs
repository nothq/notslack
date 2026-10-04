use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use crate::model::{
    SlackFileStagingAllocatedAttempts, SlackFileStagingCancellationOutcome,
    SlackFileStagingDiagnostic, SlackFileStagingFailure, SlackFileStagingOperationFailure,
    SlackFileStagingOperationId, SlackFileStagingOutcome, SlackRemoteDraftFileReference,
};

use super::{
    SlackComposerDraftId, SlackComposerDraftKey, SlackComposerFileId, SlackMainComposerDraftOwner,
};
use crate::ui::{SlackStagedFile, SlackUploadFile, SlackWorkspaceApi};

mod coordinator;
mod file_state;
mod ownership;

pub(crate) use coordinator::{
    SlackActiveFileStaging, SlackFileRemovalTombstone, SlackFileRemovalTombstoneState,
    SlackFileStagingCoordinator, SlackFileStagingRetryRequest,
};
pub(crate) use file_state::SlackComposerFileState;
use file_state::{cancellation_outcome_diagnostic, staging_outcome_diagnostic};
pub(crate) use ownership::{
    SlackComposerFileFailure, SlackComposerFileRecovery, SlackFileRetryRecoveryKind,
    SlackRetainedFileOwnership,
};

pub(crate) const SLACK_FILE_STAGING_CONCURRENCY: usize = 4;
pub(crate) const SLACK_COMPOSER_FILE_LIMIT: usize = 10;
pub(crate) const SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC: &str =
    "You can only upload 10 files at a time.";

#[derive(Debug)]
pub struct SlackAttachmentPathSelection {
    paths: Vec<std::path::PathBuf>,
    selection_truncated: bool,
}

impl SlackAttachmentPathSelection {
    pub(crate) fn for_attached_file_count(
        mut paths: Vec<std::path::PathBuf>,
        attached_file_count: usize,
    ) -> Result<Self, String> {
        if paths.is_empty() {
            return Err("Select at least one file to attach.".to_string());
        }
        if attached_file_count >= SLACK_COMPOSER_FILE_LIMIT {
            return Err(SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string());
        }
        let remaining_slots = SLACK_COMPOSER_FILE_LIMIT - attached_file_count;
        let selection_truncated = paths.len() > remaining_slots;
        paths.truncate(remaining_slots);
        Ok(Self {
            paths,
            selection_truncated,
        })
    }

    pub fn into_parts(self) -> (Vec<std::path::PathBuf>, bool) {
        (self.paths, self.selection_truncated)
    }

    pub(crate) fn path_count(&self) -> usize {
        self.paths.len()
    }

    pub(crate) const fn was_truncated(&self) -> bool {
        self.selection_truncated
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackFileStagingDraftOwner {
    Main(SlackMainComposerDraftOwner),
    Thread(SlackComposerDraftKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackFileStagingLocator {
    pub(crate) owner: SlackFileStagingDraftOwner,
    pub(crate) draft_id: SlackComposerDraftId,
    pub(crate) file_id: SlackComposerFileId,
    pub(crate) operation_id: SlackFileStagingOperationId,
}

impl SlackFileStagingLocator {
    pub(crate) fn main(
        handle: &super::SlackMainComposerDraftHandle,
        file_id: SlackComposerFileId,
        operation_id: SlackFileStagingOperationId,
    ) -> Self {
        Self {
            owner: SlackFileStagingDraftOwner::Main(handle.owner.clone()),
            draft_id: handle.draft_id,
            file_id,
            operation_id,
        }
    }

    pub(crate) fn thread(
        handle: &super::SlackThreadDraftHandle,
        file_id: SlackComposerFileId,
        operation_id: SlackFileStagingOperationId,
    ) -> Self {
        Self {
            owner: SlackFileStagingDraftOwner::Thread(handle.key().clone()),
            draft_id: handle.draft_id(),
            file_id,
            operation_id,
        }
    }
}
