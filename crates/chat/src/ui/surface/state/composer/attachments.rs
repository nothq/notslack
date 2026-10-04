use super::{Context, SlackUploadFile, SurfaceState};
use crate::ui::surface::{
    SlackAttachmentPathSelection, SlackComposerFileId, SlackFileStagingLocator,
    SlackMainComposerDraftHandle, SlackMainComposerDraftOwner, SlackPreparedUploadFile,
    SlackRemoteDraftFileLocator,
};
#[cfg(test)]
use crate::ui::SlackAttachment;

mod batching;
mod picker;

impl SurfaceState {
    #[cfg(test)]
    pub(crate) fn attach_slack_draft_attachment(
        &mut self,
        attachment: SlackAttachment,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        let file_id = self.next_slack_composer_file_id();
        self.advance_slack_send_draft_revision();
        self.slack_composer_files.push_fixture(file_id, attachment);
        self.slack_main_composer_draft_changed(cx);
        self.slack_aux_panel = None;
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    pub(crate) fn attach_slack_upload_files(
        &mut self,
        files: Vec<SlackUploadFile>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return;
        }
        if !self.is_slack_workspace()
            || !self.slack_workspace_api_capabilities.send_message
            || !self.slack_workspace_api_capabilities.stage_file
        {
            return;
        }
        let Some(handle) = self.current_slack_main_composer_draft_handle() else {
            return;
        };
        let _ = self.attach_slack_upload_files_to_main_draft(&handle, files, cx);
    }

    pub(crate) fn remove_slack_draft_attachment(
        &mut self,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return false;
        }
        let Some(handle) = self.current_slack_main_composer_draft_handle() else {
            return false;
        };
        self.remove_slack_main_draft_attachment(&handle, file_id, cx)
    }

    pub(crate) fn remove_slack_main_draft_attachment(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return false;
        }
        if self.current_slack_main_composer_draft_handle().as_ref() != Some(handle) {
            return false;
        }
        let Some(file) = self.slack_composer_files.remove_file(file_id) else {
            self.slack_composer_focused = true;
            cx.notify();
            return false;
        };
        let remote_locator = file
            .state()
            .remote_reference()
            .cloned()
            .map(|reference| SlackRemoteDraftFileLocator::main(handle, file_id, reference));
        self.advance_slack_send_draft_revision();
        self.slack_main_composer_draft_changed(cx);
        if let Some(locator) = remote_locator {
            let deferred = if let Some(active_edit) = self
                .slack_active_scheduled_edit
                .as_mut()
                .filter(|active| active.edit_draft_handle == *handle)
            {
                if !active_edit.deferred_remote_removals.contains(&locator) {
                    active_edit.deferred_remote_removals.push(locator.clone());
                }
                true
            } else {
                false
            };
            if deferred {
                self.cancel_slack_remote_draft_file_load(&locator, cx);
            } else {
                self.begin_slack_remote_draft_file_cleanup(locator, cx);
            }
        } else if let Some(operation_id) = file.operation_id().cloned() {
            let locator = SlackFileStagingLocator::main(handle, file_id, operation_id);
            self.remove_slack_file_staging_ownership(locator, file, cx);
        }
        self.slack_composer_focused = true;
        cx.notify();
        true
    }

    pub(crate) fn retry_slack_draft_attachment(
        &mut self,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFileSummary, String> {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return Err("Wait for Slack scheduling to finish before retrying a file.".to_string());
        }
        let handle = self
            .current_slack_main_composer_draft_handle()
            .ok_or_else(|| "No active Slack composer draft accepts file retry.".to_string())?;
        let file = self
            .slack_composer_files
            .file(file_id)
            .ok_or_else(|| format!("Slack composer file {file_id} was not found."))?;
        if let Some(reference) = file.state().remote_reference().cloned() {
            let locator = SlackRemoteDraftFileLocator::main(&handle, file_id, reference);
            self.retry_slack_remote_draft_file_load(locator, cx)?;
            return self
                .slack_composer_files
                .file(file_id)
                .map(|file| file.control_summary())
                .ok_or_else(|| {
                    format!("Slack composer file {file_id} moved while retry was starting.")
                });
        }
        let operation_id = file
            .operation_id()
            .cloned()
            .ok_or_else(|| format!("Slack composer file {file_id} is not retryable."))?;
        let locator = SlackFileStagingLocator::main(&handle, file_id, operation_id);
        self.retry_slack_file_staging(locator, cx)?;
        self.slack_composer_files
            .file(file_id)
            .map(|file| file.control_summary())
            .ok_or_else(|| format!("Slack composer file {file_id} moved while retry was starting."))
    }

    pub(crate) fn attach_slack_upload_files_to_main_draft(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        files: Vec<SlackUploadFile>,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        self.attach_slack_upload_files_to_main_draft_with_selection(handle, files, false, cx)
    }

    pub(crate) fn attach_slack_upload_files_to_main_draft_with_selection(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        files: Vec<SlackUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        self.attach_slack_prepared_upload_files_to_main_draft_with_selection(
            handle,
            files
                .into_iter()
                .map(SlackPreparedUploadFile::from_upload)
                .collect(),
            path_selection_truncated,
            cx,
        )
    }

    pub(in crate::ui::surface::state) fn slack_main_draft_files(
        &self,
        handle: &SlackMainComposerDraftHandle,
    ) -> Option<&crate::ui::surface::SlackComposerFiles> {
        if self.current_slack_main_composer_draft_handle().as_ref() == Some(handle) {
            return Some(&self.slack_composer_files);
        }
        match &handle.owner {
            SlackMainComposerDraftOwner::Conversation(key) => self
                .slack_new_message_return_draft
                .as_ref()
                .and_then(|(return_key, draft)| {
                    (return_key == key && draft.id == handle.draft_id).then_some(&draft.files)
                })
                .or_else(|| {
                    self.slack_composer_drafts
                        .get(key)
                        .filter(|draft| draft.id == handle.draft_id)
                        .map(|draft| &draft.files)
                }),
            SlackMainComposerDraftOwner::NewMessage(key) => self
                .slack_new_message_drafts
                .get(key)
                .filter(|draft| draft.id == handle.draft_id)
                .map(|draft| &draft.files),
        }
    }

    pub(crate) fn slack_main_attachment_path_selection(
        &self,
        handle: &SlackMainComposerDraftHandle,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<SlackAttachmentPathSelection, String> {
        let attached_file_count = self
            .slack_main_draft_files(handle)
            .map(|files| files.len())
            .ok_or_else(|| {
                "The exact Slack composer draft is no longer available for attachments.".to_string()
            })?;
        SlackAttachmentPathSelection::for_attached_file_count(paths, attached_file_count)
    }

    fn slack_main_file_summaries(
        &self,
        handle: &SlackMainComposerDraftHandle,
        file_ids: &[SlackComposerFileId],
    ) -> Result<Vec<crate::model::ChatComposerFileSummary>, String> {
        let files = self.slack_main_draft_files(handle).ok_or_else(|| {
            "The exact Slack composer draft moved before attachment summaries were read."
                .to_string()
        })?;
        file_ids
            .iter()
            .map(|file_id| {
                files
                    .file(*file_id)
                    .map(|file| file.control_summary())
                    .ok_or_else(|| {
                        format!("Slack composer file {file_id} moved before its summary was read.")
                    })
            })
            .collect()
    }
}
