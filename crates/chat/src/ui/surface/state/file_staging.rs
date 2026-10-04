mod lookup;
mod receipts;
mod removal;
mod retry_reconciliation;
mod upload;

use crate::model::{
    SlackFileStagingCancellationOutcome, SlackFileStagingDiagnostic, SlackFileStagingOperationId,
    SlackFileStagingOutcome,
};

use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey, SlackComposerFile,
    SlackComposerFileFailure, SlackComposerFileRecovery, SlackComposerFileState,
    SlackComposerFiles, SlackFileRemovalTombstone, SlackFileRemovalTombstoneState,
    SlackFileStagingDraftOwner, SlackFileStagingLocator, SlackFileStagingRetryRequest,
    SlackMainComposerDraftOwner,
};
use receipts::slack_file_retry_start;

enum SlackFileRetryStart {
    Queue(crate::ui::SlackUploadFile),
    Reconcile(crate::ui::SlackUploadFile),
    Cleanup(crate::ui::SlackUploadFile),
}

enum SlackFileRetryContinuation {
    None,
    Queue(SlackFileStagingLocator),
    Cleanup,
    AlreadyShared,
    AlreadyDraftOwned,
}

enum SlackVisibleFileApplication<Applied, Removed> {
    Applied(Applied),
    Removed(Removed),
}

struct SlackRemovedActiveFileOutcomes {
    staging: SlackFileStagingOutcome,
    cancellation: SlackFileStagingCancellationOutcome,
}

impl SurfaceState {
    pub(crate) fn discard_slack_composer_draft(
        &mut self,
        owner: SlackFileStagingDraftOwner,
        draft: SlackComposerDraft,
        cx: &mut Context<Self>,
    ) {
        self.discard_slack_composer_files(owner, draft.id, draft.files, cx);
    }

    pub(crate) fn discard_slack_stored_composer_draft(
        &mut self,
        key: SlackComposerDraftKey,
        draft: SlackComposerDraft,
        cx: &mut Context<Self>,
    ) {
        let owner = match &key.destination {
            SlackComposerDestination::Conversation { .. } => {
                SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key))
            }
            SlackComposerDestination::Thread { .. } => SlackFileStagingDraftOwner::Thread(key),
        };
        self.discard_slack_composer_draft(owner, draft, cx);
    }

    pub(crate) fn discard_slack_composer_files(
        &mut self,
        owner: SlackFileStagingDraftOwner,
        draft_id: crate::ui::surface::SlackComposerDraftId,
        files: SlackComposerFiles,
        cx: &mut Context<Self>,
    ) {
        for file in files.into_files() {
            let file_id = file.id();
            let Some(operation_id) = file.operation_id().cloned() else {
                continue;
            };
            self.remove_slack_file_staging_ownership(
                SlackFileStagingLocator {
                    owner: owner.clone(),
                    draft_id,
                    file_id,
                    operation_id,
                },
                file,
                cx,
            );
        }
    }

    pub(crate) fn enqueue_slack_file_staging(
        &mut self,
        locators: impl IntoIterator<Item = SlackFileStagingLocator>,
        cx: &mut Context<Self>,
    ) {
        let workspace_api = self.active_slack_workspace_api();
        for locator in locators {
            if !self.slack_file_staging_owner_is_current(&locator.owner) {
                self.fail_queued_slack_file_staging(
                    &locator,
                    SlackFileStagingDiagnostic::new(
                        "Slack file staging cannot cross workspace identities.".to_string(),
                    ),
                );
                continue;
            }
            let Some(workspace_api) = workspace_api.as_ref() else {
                self.fail_queued_slack_file_staging(
                    &locator,
                    SlackFileStagingDiagnostic::new(
                        "Slack file staging requires a connected workspace.".to_string(),
                    ),
                );
                continue;
            };
            self.slack_file_staging
                .enqueue(locator, workspace_api.clone());
        }
        self.pump_slack_file_staging(cx);
    }

    pub(crate) fn remove_slack_file_staging_ownership(
        &mut self,
        locator: SlackFileStagingLocator,
        file: SlackComposerFile,
        cx: &mut Context<Self>,
    ) {
        match file.into_state() {
            SlackComposerFileState::Queued { .. } => {
                self.remove_queued_slack_file_staging_ownership(&locator);
            }
            SlackComposerFileState::Uploading { .. } => {
                self.remove_slack_uploading_file_ownership(locator, cx);
            }
            SlackComposerFileState::Error {
                failure: crate::ui::surface::SlackComposerFileFailure::NotSubmitted(_),
                ..
            } => {}
            SlackComposerFileState::Error {
                failure:
                    SlackComposerFileFailure::Cancelled(
                        SlackFileStagingCancellationOutcome::NoPersistentFile
                        | SlackFileStagingCancellationOutcome::ConfirmedDeleted
                        | SlackFileStagingCancellationOutcome::Shared
                        | SlackFileStagingCancellationOutcome::DraftOwned,
                    ),
                ..
            } => {
                self.slack_file_staging
                    .operation_apis
                    .remove(&locator.operation_id);
            }
            SlackComposerFileState::Staged { .. }
            | SlackComposerFileState::Error { .. }
            | SlackComposerFileState::RetainedUnknown { .. } => {
                self.begin_slack_file_removal_cleanup(locator, cx);
            }
            SlackComposerFileState::Recovering { .. } => {
                self.slack_file_staging.removals.insert(
                    locator.operation_id.clone(),
                    SlackFileRemovalTombstone {
                        locator,
                        state: SlackFileRemovalTombstoneState::WaitingForRecovery,
                    },
                );
            }
            SlackComposerFileState::RetryBlocked { .. } => {
                self.begin_slack_file_removal_cleanup(locator, cx);
            }
            SlackComposerFileState::AlreadyShared { .. }
            | SlackComposerFileState::AlreadyDraftOwned { .. } => {}
            SlackComposerFileState::RemoteLoading { .. }
            | SlackComposerFileState::RemoteReady { .. }
            | SlackComposerFileState::RemoteError { .. } => {}
            #[cfg(test)]
            SlackComposerFileState::Fixture => {}
        }
    }

    fn remove_queued_slack_file_staging_ownership(&mut self, locator: &SlackFileStagingLocator) {
        if self.slack_file_staging.remove_queued(locator) {
            self.slack_file_staging
                .operation_apis
                .remove(&locator.operation_id);
        }
    }

    fn remove_slack_uploading_file_ownership(
        &mut self,
        locator: SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) {
        let workspace_api = self
            .slack_file_staging
            .active
            .get(&locator.operation_id)
            .filter(|active| active.locator == locator)
            .map(|active| active.workspace_api.clone());
        let Some(workspace_api) = workspace_api else {
            self.retain_unavailable_slack_file_cleanup(locator);
            cx.notify();
            return;
        };
        self.slack_file_staging.removals.insert(
            locator.operation_id.clone(),
            SlackFileRemovalTombstone {
                locator: locator.clone(),
                state: SlackFileRemovalTombstoneState::WaitingForStageAndCancel,
            },
        );
        self.spawn_slack_file_staging_cancellation(workspace_api, locator, cx);
    }

    pub(crate) fn retry_slack_file_staging(
        &mut self,
        locator: SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.slack_file_staging_owner_is_current(&locator.owner) {
            return Err(
                "Slack file retry cannot cross workspace or account identities.".to_string(),
            );
        }
        let state = self
            .slack_file_for_staging_mut(&locator)
            .map(|file| file.state().clone())
            .ok_or_else(|| "The Slack composer file moved before retry could start.".to_string())?;
        let start = slack_file_retry_start(&state)?;
        let next_operation_id = SlackFileStagingOperationId::generate();
        match start {
            SlackFileRetryStart::Queue(upload) => {
                self.queue_slack_file_staging_retry(locator, next_operation_id, upload, cx)?;
            }
            SlackFileRetryStart::Reconcile(upload) => {
                self.reconcile_slack_file_staging_retry(locator, next_operation_id, upload, cx)?;
            }
            SlackFileRetryStart::Cleanup(upload) => {
                self.cleanup_slack_file_staging_retry(locator, next_operation_id, upload, cx)?;
            }
        }
        cx.notify();
        Ok(())
    }

    fn queue_slack_file_staging_retry(
        &mut self,
        locator: SlackFileStagingLocator,
        next_operation_id: SlackFileStagingOperationId,
        upload: crate::ui::SlackUploadFile,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let workspace_api = self
            .active_slack_workspace_api()
            .ok_or_else(|| "Slack file retry requires a connected workspace.".to_string())?;
        let next_locator = SlackFileStagingLocator {
            operation_id: next_operation_id.clone(),
            ..locator.clone()
        };
        let file = self.slack_file_for_staging_mut(&locator).ok_or_else(|| {
            "The Slack composer file moved before retry could be queued.".to_string()
        })?;
        *file.state_mut() = SlackComposerFileState::Queued {
            operation_id: next_operation_id,
            upload,
        };
        self.slack_file_staging
            .operation_apis
            .remove(&locator.operation_id);
        self.slack_file_staging.enqueue(next_locator, workspace_api);
        self.pump_slack_file_staging(cx);
        Ok(())
    }

    fn reconcile_slack_file_staging_retry(
        &mut self,
        locator: SlackFileStagingLocator,
        next_operation_id: SlackFileStagingOperationId,
        upload: crate::ui::SlackUploadFile,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let workspace_api = self.slack_file_operation_api(&locator)?;
        let request = SlackFileStagingRetryRequest {
            locator: locator.clone(),
            next_operation_id: next_operation_id.clone(),
        };
        let file = self.slack_file_for_staging_mut(&locator).ok_or_else(|| {
            "The Slack composer file moved before reconciliation could start.".to_string()
        })?;
        *file.state_mut() = SlackComposerFileState::Recovering {
            upload,
            recovery: SlackComposerFileRecovery::ReconcilingForRetry {
                prior_operation_id: locator.operation_id,
                next_operation_id,
            },
        };
        self.spawn_slack_file_retry_reconciliation(workspace_api, request, cx);
        Ok(())
    }

    fn cleanup_slack_file_staging_retry(
        &mut self,
        locator: SlackFileStagingLocator,
        next_operation_id: SlackFileStagingOperationId,
        upload: crate::ui::SlackUploadFile,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let workspace_api = self.slack_file_operation_api(&locator)?;
        let request = SlackFileStagingRetryRequest {
            locator: locator.clone(),
            next_operation_id: next_operation_id.clone(),
        };
        let file = self.slack_file_for_staging_mut(&locator).ok_or_else(|| {
            "The Slack composer file moved before cleanup could start.".to_string()
        })?;
        *file.state_mut() = SlackComposerFileState::Recovering {
            upload,
            recovery: SlackComposerFileRecovery::CleaningForRetry {
                prior_operation_id: locator.operation_id,
                next_operation_id,
            },
        };
        self.spawn_slack_file_retry_cleanup(workspace_api, request, cx);
        Ok(())
    }
}
