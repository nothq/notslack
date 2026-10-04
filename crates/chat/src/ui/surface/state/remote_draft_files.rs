use std::sync::Arc;

use crate::model::{
    SlackFileMetadataEntry, SlackFileMetadataRequest, SlackRemoteDraftFileCleanupOutcome,
};

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    SlackActiveRemoteDraftFileLoad, SlackComposerFileState, SlackRemoteDraftFileCleanup,
    SlackRemoteDraftFileCleanupState, SlackRemoteDraftFileLocator, SlackScheduleDraftOwner,
};

impl SurfaceState {
    pub(crate) fn cancel_slack_remote_draft_file_load(
        &mut self,
        locator: &SlackRemoteDraftFileLocator,
        cx: &mut Context<Self>,
    ) {
        self.slack_remote_draft_files.cancel_load(locator);
        self.pump_slack_remote_draft_file_loads(cx);
    }

    pub(crate) fn enqueue_slack_remote_draft_file_loads(
        &mut self,
        locators: impl IntoIterator<Item = SlackRemoteDraftFileLocator>,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        for locator in locators {
            if !self.slack_file_staging_owner_is_current(&locator.owner) {
                self.apply_slack_remote_draft_file_load_failure(
                    &locator,
                    "Slack remote draft file loading cannot cross workspace identities."
                        .to_string(),
                );
                continue;
            }
            self.slack_remote_draft_files
                .enqueue(locator, workspace_api.clone());
        }
        self.pump_slack_remote_draft_file_loads(cx);
    }

    pub(crate) fn enqueue_slack_remote_draft_file_loads_for_owned_schedule_draft(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        locators: impl IntoIterator<Item = SlackRemoteDraftFileLocator>,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        for locator in locators {
            assert!(
                owner.matches_owned_draft(&locator.owner, locator.draft_id),
                "retained Slack remote-file loading requires the exact schedule draft owner"
            );
            self.slack_remote_draft_files
                .enqueue(locator, workspace_api.clone());
        }
        self.pump_slack_remote_draft_file_loads(cx);
    }

    pub(crate) fn retry_slack_remote_draft_file_load(
        &mut self,
        locator: SlackRemoteDraftFileLocator,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.slack_file_staging_owner_is_current(&locator.owner) {
            return Err(
                "Slack remote draft file retry cannot cross workspace identities.".to_string(),
            );
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return Err(
                "Slack remote draft file retry requires a connected workspace.".to_string(),
            );
        };
        let file = self.slack_remote_draft_file_mut(&locator).ok_or_else(|| {
            "The Slack remote draft file moved before retry could start.".to_string()
        })?;
        match file.state() {
            SlackComposerFileState::RemoteError { reference, .. }
                if reference == &locator.reference => {}
            SlackComposerFileState::RemoteLoading { .. } => {
                return Err("This Slack remote draft file is already loading.".to_string());
            }
            SlackComposerFileState::RemoteReady { .. } => {
                return Err("This Slack remote draft file metadata is already loaded.".to_string());
            }
            _ => {
                return Err(
                    "This Slack composer file is not a retryable remote draft file.".to_string(),
                );
            }
        }
        *file.state_mut() = SlackComposerFileState::RemoteLoading {
            reference: locator.reference.clone(),
        };
        self.slack_remote_draft_files
            .enqueue(locator, workspace_api);
        self.pump_slack_remote_draft_file_loads(cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn begin_slack_remote_draft_file_cleanup(
        &mut self,
        locator: SlackRemoteDraftFileLocator,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_remote_draft_files.cancel_load(&locator);
            self.pump_slack_remote_draft_file_loads(cx);
            self.slack_remote_draft_files.cleanups.insert(
                locator.file_id,
                SlackRemoteDraftFileCleanup {
                    locator,
                    state: SlackRemoteDraftFileCleanupState::Finished(
                        SlackRemoteDraftFileCleanupOutcome::Failed {
                            diagnostic:
                                "Slack remote draft file cleanup requires a connected workspace."
                                    .to_string(),
                        },
                    ),
                },
            );
            cx.notify();
            return;
        };
        self.begin_slack_remote_draft_file_cleanup_with_api(locator, workspace_api, cx);
    }

    pub(crate) fn begin_slack_remote_draft_file_cleanup_with_api(
        &mut self,
        locator: SlackRemoteDraftFileLocator,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        self.slack_remote_draft_files.cancel_load(&locator);
        self.pump_slack_remote_draft_file_loads(cx);
        self.slack_remote_draft_files.cleanups.insert(
            locator.file_id,
            SlackRemoteDraftFileCleanup {
                locator: locator.clone(),
                state: SlackRemoteDraftFileCleanupState::Cleaning,
            },
        );
        self.spawn_background_task(
            (workspace_api, locator),
            cx,
            |(workspace_api, locator): (Arc<dyn WorkspaceApi>, SlackRemoteDraftFileLocator)| {
                let outcome = workspace_api.cleanup_slack_remote_draft_file(&locator.reference);
                (locator, outcome)
            },
            |this, (locator, outcome), cx| {
                let Some(cleanup) = this
                    .slack_remote_draft_files
                    .cleanups
                    .get_mut(&locator.file_id)
                else {
                    return;
                };
                if cleanup.locator != locator
                    || !matches!(cleanup.state, SlackRemoteDraftFileCleanupState::Cleaning)
                {
                    return;
                }
                cleanup.state = SlackRemoteDraftFileCleanupState::Finished(outcome);
                cx.notify();
            },
        );
    }

    pub(in crate::ui::surface::state) fn reset_slack_remote_draft_file_loads(&mut self) {
        let pending_owner = self
            .slack_schedule_pending
            .as_ref()
            .map(|pending| pending.owner.clone())
            .or_else(|| {
                self.slack_composer_schedule_recovery
                    .as_ref()
                    .map(|recovery| recovery.owner.clone())
            });
        self.slack_remote_draft_files.queued.retain(|queued| {
            pending_owner.as_ref().is_some_and(|owner| {
                owner.matches_owned_draft(&queued.locator.owner, queued.locator.draft_id)
            })
        });
    }

    fn pump_slack_remote_draft_file_loads(&mut self, cx: &mut Context<Self>) {
        while self.slack_remote_draft_files.has_load_capacity() {
            let Some(queued) = self.slack_remote_draft_files.queued.pop_front() else {
                break;
            };
            let locator = queued.locator;
            if self.slack_remote_draft_file_mut(&locator).is_none() {
                continue;
            }
            let request = match SlackFileMetadataRequest::new(vec![locator.reference.clone()]) {
                Ok(request) => request,
                Err(diagnostic) => {
                    self.apply_slack_remote_draft_file_load_failure(&locator, diagnostic);
                    continue;
                }
            };
            let workspace_api = queued.workspace_api;
            self.slack_remote_draft_files.active.insert(
                locator.file_id,
                SlackActiveRemoteDraftFileLoad {
                    locator: locator.clone(),
                },
            );
            self.spawn_background_task(
                (workspace_api, locator, request),
                cx,
                |(workspace_api, locator, request)| {
                    let result = workspace_api.load_slack_file_metadata(&request);
                    (locator, result)
                },
                |this, (locator, result), cx| {
                    this.finish_slack_remote_draft_file_load(locator, result, cx);
                },
            );
        }
    }

    fn finish_slack_remote_draft_file_load(
        &mut self,
        locator: SlackRemoteDraftFileLocator,
        result: Result<crate::model::SlackFileMetadataBatch, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(active) = self
            .slack_remote_draft_files
            .active
            .remove(&locator.file_id)
        else {
            return;
        };
        if active.locator != locator {
            self.slack_remote_draft_files
                .active
                .insert(active.locator.file_id, active);
            return;
        }
        match result {
            Ok(batch) => {
                let mut entries = batch.into_entries();
                let entry = entries
                    .pop()
                    .expect("one-reference metadata request must return one entry");
                assert!(
                    entries.is_empty(),
                    "one-reference metadata request returned extra entries"
                );
                self.apply_slack_remote_draft_file_metadata(&locator, entry, cx);
            }
            Err(diagnostic) => {
                self.apply_slack_remote_draft_file_load_failure(&locator, diagnostic);
            }
        }
        self.pump_slack_remote_draft_file_loads(cx);
        cx.notify();
    }

    fn apply_slack_remote_draft_file_metadata(
        &mut self,
        locator: &SlackRemoteDraftFileLocator,
        entry: SlackFileMetadataEntry,
        cx: &mut Context<Self>,
    ) {
        let preview_url = {
            let file = match self.slack_remote_draft_file_mut(locator) {
                Some(file) => file,
                None => return,
            };
            match entry {
                SlackFileMetadataEntry::Loaded {
                    reference,
                    metadata,
                } if reference == locator.reference => {
                    let attachment = metadata.attachment_arc();
                    let preview_url = attachment
                        .preview_image_url
                        .as_ref()
                        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
                        .cloned();
                    file.replace_attachment(attachment);
                    *file.state_mut() = SlackComposerFileState::RemoteReady { reference };
                    preview_url
                }
                SlackFileMetadataEntry::Failed {
                    reference,
                    diagnostic,
                } if reference == locator.reference => {
                    *file.state_mut() = SlackComposerFileState::RemoteError {
                        reference,
                        diagnostic,
                    };
                    None
                }
                SlackFileMetadataEntry::Loaded { reference, .. }
                | SlackFileMetadataEntry::Failed { reference, .. } => {
                    *file.state_mut() = SlackComposerFileState::RemoteError {
                        reference: locator.reference.clone(),
                        diagnostic: format!(
                            "Slack returned metadata for {} while loading {}.",
                            reference.file_id(),
                            locator.reference.file_id()
                        ),
                    };
                    None
                }
            }
        };
        if let Some(url) = preview_url {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    fn apply_slack_remote_draft_file_load_failure(
        &mut self,
        locator: &SlackRemoteDraftFileLocator,
        diagnostic: String,
    ) {
        let Some(file) = self.slack_remote_draft_file_mut(locator) else {
            return;
        };
        *file.state_mut() = SlackComposerFileState::RemoteError {
            reference: locator.reference.clone(),
            diagnostic,
        };
    }

    fn slack_remote_draft_file_mut(
        &mut self,
        locator: &SlackRemoteDraftFileLocator,
    ) -> Option<&mut crate::ui::surface::SlackComposerFile> {
        let files =
            self.slack_files_for_owned_composer_draft_mut(&locator.owner, locator.draft_id)?;
        let file = files.file_mut(locator.file_id)?;
        (file.state().remote_reference() == Some(&locator.reference)).then_some(file)
    }
}
