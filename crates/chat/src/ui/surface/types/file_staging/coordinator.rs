use super::{
    cancellation_outcome_diagnostic, staging_outcome_diagnostic, Arc, HashMap,
    SlackFileStagingCancellationOutcome, SlackFileStagingLocator, SlackFileStagingOperationId,
    SlackFileStagingOutcome, SlackRetainedFileOwnership, SlackWorkspaceApi, VecDeque,
    SLACK_FILE_STAGING_CONCURRENCY,
};

#[derive(Clone, Debug)]
pub(crate) enum SlackFileRemovalTombstoneState {
    WaitingForStageAndCancel,
    WaitingForStage {
        cancellation: SlackFileStagingCancellationOutcome,
    },
    WaitingForCancel {
        staging: SlackFileStagingOutcome,
    },
    WaitingForRecovery,
    Cleaning,
    RetainedUnknown(SlackRetainedFileOwnership),
}

#[derive(Clone, Debug)]
pub(crate) struct SlackFileRemovalTombstone {
    pub(crate) locator: SlackFileStagingLocator,
    pub(crate) state: SlackFileRemovalTombstoneState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackFileStagingRetryRequest {
    pub(crate) locator: SlackFileStagingLocator,
    pub(crate) next_operation_id: SlackFileStagingOperationId,
}

#[derive(Default)]
pub(crate) struct SlackFileStagingCoordinator {
    pub(crate) queued: VecDeque<SlackQueuedFileStaging>,
    pub(crate) active: HashMap<SlackFileStagingOperationId, SlackActiveFileStaging>,
    pub(crate) operation_apis: HashMap<SlackFileStagingOperationId, Arc<dyn SlackWorkspaceApi>>,
    pub(crate) removals: HashMap<SlackFileStagingOperationId, SlackFileRemovalTombstone>,
}

pub(crate) struct SlackActiveFileStaging {
    pub(crate) locator: SlackFileStagingLocator,
    pub(crate) workspace_api: Arc<dyn SlackWorkspaceApi>,
}

pub(crate) struct SlackQueuedFileStaging {
    pub(crate) locator: SlackFileStagingLocator,
    pub(crate) workspace_api: Arc<dyn SlackWorkspaceApi>,
}

impl SlackFileStagingCoordinator {
    pub(crate) fn enqueue(
        &mut self,
        locator: SlackFileStagingLocator,
        workspace_api: Arc<dyn SlackWorkspaceApi>,
    ) {
        self.queued.push_back(SlackQueuedFileStaging {
            locator: locator.clone(),
            workspace_api: workspace_api.clone(),
        });
        self.operation_apis
            .insert(locator.operation_id, workspace_api);
    }

    pub(crate) fn remove_queued(&mut self, locator: &SlackFileStagingLocator) -> bool {
        let Some(index) = self
            .queued
            .iter()
            .position(|queued| queued.locator == *locator)
        else {
            return false;
        };
        self.queued.remove(index);
        true
    }

    pub(crate) fn has_stage_capacity(&self) -> bool {
        self.active.len() < SLACK_FILE_STAGING_CONCURRENCY
    }

    pub(crate) fn file_cleanup_summaries(&self) -> Vec<crate::model::ChatFileCleanupSummary> {
        let mut summaries = self
            .removals
            .values()
            .map(|tombstone| {
                let (status, diagnostic) = match &tombstone.state {
                    SlackFileRemovalTombstoneState::WaitingForStageAndCancel => (
                        crate::model::ChatFileCleanupStatus::WaitingForStageAndCancel,
                        None,
                    ),
                    SlackFileRemovalTombstoneState::WaitingForStage { cancellation } => (
                        crate::model::ChatFileCleanupStatus::WaitingForStage,
                        Some(cancellation_outcome_diagnostic(cancellation)),
                    ),
                    SlackFileRemovalTombstoneState::WaitingForCancel { staging } => (
                        crate::model::ChatFileCleanupStatus::WaitingForCancel,
                        staging_outcome_diagnostic(staging),
                    ),
                    SlackFileRemovalTombstoneState::WaitingForRecovery => (
                        crate::model::ChatFileCleanupStatus::WaitingForRecovery,
                        None,
                    ),
                    SlackFileRemovalTombstoneState::Cleaning => {
                        (crate::model::ChatFileCleanupStatus::Cleaning, None)
                    }
                    SlackFileRemovalTombstoneState::RetainedUnknown(ownership) => (
                        crate::model::ChatFileCleanupStatus::RetainedUnknown,
                        Some(ownership.diagnostic()),
                    ),
                };
                crate::model::ChatFileCleanupSummary {
                    operation_id: tombstone.locator.operation_id.to_string(),
                    status,
                    diagnostic,
                }
            })
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        summaries
    }
}
