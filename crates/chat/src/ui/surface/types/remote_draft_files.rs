use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use crate::model::{SlackRemoteDraftFileCleanupOutcome, SlackRemoteDraftFileReference};

use super::{
    SlackComposerDraftId, SlackComposerFileId, SlackFileStagingDraftOwner,
    SlackMainComposerDraftHandle, SlackThreadDraftHandle,
};
use crate::ui::SlackWorkspaceApi;

pub(crate) const SLACK_REMOTE_DRAFT_FILE_LOAD_CONCURRENCY: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackRemoteDraftFileLocator {
    pub(crate) owner: SlackFileStagingDraftOwner,
    pub(crate) draft_id: SlackComposerDraftId,
    pub(crate) file_id: SlackComposerFileId,
    pub(crate) reference: SlackRemoteDraftFileReference,
}

impl SlackRemoteDraftFileLocator {
    pub(crate) fn main(
        handle: &SlackMainComposerDraftHandle,
        file_id: SlackComposerFileId,
        reference: SlackRemoteDraftFileReference,
    ) -> Self {
        Self {
            owner: SlackFileStagingDraftOwner::Main(handle.owner.clone()),
            draft_id: handle.draft_id,
            file_id,
            reference,
        }
    }

    pub(crate) fn thread(
        handle: &SlackThreadDraftHandle,
        file_id: SlackComposerFileId,
        reference: SlackRemoteDraftFileReference,
    ) -> Self {
        Self {
            owner: SlackFileStagingDraftOwner::Thread(handle.key().clone()),
            draft_id: handle.draft_id(),
            file_id,
            reference,
        }
    }
}

pub(crate) struct SlackQueuedRemoteDraftFileLoad {
    pub(crate) locator: SlackRemoteDraftFileLocator,
    pub(crate) workspace_api: Arc<dyn SlackWorkspaceApi>,
}

pub(crate) struct SlackActiveRemoteDraftFileLoad {
    pub(crate) locator: SlackRemoteDraftFileLocator,
}

#[derive(Clone, Debug)]
pub(crate) enum SlackRemoteDraftFileCleanupState {
    Cleaning,
    Finished(SlackRemoteDraftFileCleanupOutcome),
}

#[derive(Clone, Debug)]
pub(crate) struct SlackRemoteDraftFileCleanup {
    pub(crate) locator: SlackRemoteDraftFileLocator,
    pub(crate) state: SlackRemoteDraftFileCleanupState,
}

#[derive(Default)]
pub(crate) struct SlackRemoteDraftFileCoordinator {
    pub(crate) queued: VecDeque<SlackQueuedRemoteDraftFileLoad>,
    pub(crate) active: HashMap<SlackComposerFileId, SlackActiveRemoteDraftFileLoad>,
    pub(crate) cleanups: HashMap<SlackComposerFileId, SlackRemoteDraftFileCleanup>,
}

impl SlackRemoteDraftFileCoordinator {
    pub(crate) fn enqueue(
        &mut self,
        locator: SlackRemoteDraftFileLocator,
        workspace_api: Arc<dyn SlackWorkspaceApi>,
    ) {
        if self.queued.iter().any(|queued| queued.locator == locator)
            || self.active.values().any(|active| active.locator == locator)
        {
            return;
        }
        self.queued.push_back(SlackQueuedRemoteDraftFileLoad {
            locator,
            workspace_api,
        });
    }

    pub(crate) fn cancel_load(&mut self, locator: &SlackRemoteDraftFileLocator) {
        if let Some(index) = self
            .queued
            .iter()
            .position(|queued| queued.locator == *locator)
        {
            self.queued.remove(index);
        }
    }

    pub(crate) fn has_load_capacity(&self) -> bool {
        self.active.len() < SLACK_REMOTE_DRAFT_FILE_LOAD_CONCURRENCY
    }

    pub(crate) fn cleanup_summaries(&self) -> Vec<crate::model::ChatRemoteDraftFileCleanupSummary> {
        let mut summaries = self
            .cleanups
            .values()
            .map(remote_draft_file_cleanup_summary)
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            (
                &left.team_id,
                &left.owner_user_id,
                &left.origin_draft_id,
                &left.composer_file_id,
            )
                .cmp(&(
                    &right.team_id,
                    &right.owner_user_id,
                    &right.origin_draft_id,
                    &right.composer_file_id,
                ))
        });
        summaries
    }
}

fn remote_draft_file_cleanup_summary(
    cleanup: &SlackRemoteDraftFileCleanup,
) -> crate::model::ChatRemoteDraftFileCleanupSummary {
    let (status, diagnostic) = remote_draft_file_cleanup_result(&cleanup.state);
    crate::model::ChatRemoteDraftFileCleanupSummary {
        team_id: cleanup.locator.reference.team_id().to_string(),
        owner_user_id: cleanup.locator.reference.owner_user_id().to_string(),
        origin_draft_id: cleanup.locator.reference.origin_draft_id().to_string(),
        composer_file_id: cleanup.locator.file_id.to_string(),
        status,
        diagnostic,
    }
}

fn remote_draft_file_cleanup_result(
    state: &SlackRemoteDraftFileCleanupState,
) -> (
    crate::model::ChatRemoteDraftFileCleanupStatus,
    Option<String>,
) {
    match state {
        SlackRemoteDraftFileCleanupState::Cleaning => (
            crate::model::ChatRemoteDraftFileCleanupStatus::Cleaning,
            None,
        ),
        SlackRemoteDraftFileCleanupState::Finished(
            SlackRemoteDraftFileCleanupOutcome::PreservedShared,
        ) => (
            crate::model::ChatRemoteDraftFileCleanupStatus::PreservedShared,
            Some("Slack reports that this remote draft file is shared; it was preserved.".into()),
        ),
        SlackRemoteDraftFileCleanupState::Finished(
            SlackRemoteDraftFileCleanupOutcome::ConfirmedDeleted,
        ) => (
            crate::model::ChatRemoteDraftFileCleanupStatus::ConfirmedDeleted,
            None,
        ),
        SlackRemoteDraftFileCleanupState::Finished(
            SlackRemoteDraftFileCleanupOutcome::RetainedUnknown { diagnostic },
        ) => (
            crate::model::ChatRemoteDraftFileCleanupStatus::RetainedUnknown,
            Some(diagnostic.clone()),
        ),
        SlackRemoteDraftFileCleanupState::Finished(
            SlackRemoteDraftFileCleanupOutcome::Failed { diagnostic },
        ) => (
            crate::model::ChatRemoteDraftFileCleanupStatus::Failed,
            Some(diagnostic.clone()),
        ),
    }
}
