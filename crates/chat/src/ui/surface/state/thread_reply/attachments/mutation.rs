mod retry;

use super::super::super::{Context, SurfaceState};
use super::push_slack_thread_local_files;
use crate::ui::surface::{
    SlackComposerDraftKey, SlackComposerFile, SlackComposerFileId, SlackFileStagingLocator,
    SlackPreparedUploadFile, SlackRemoteDraftFileLocator, SlackThreadDraftHandle,
    SLACK_COMPOSER_FILE_LIMIT, SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC,
};
use crate::ui::SlackUploadFile;

type SlackPreparedThreadLocalFile = (
    SlackComposerFileId,
    crate::model::SlackFileStagingOperationId,
    SlackPreparedUploadFile,
);

struct PreparedSlackThreadAttachments {
    files: Vec<SlackPreparedThreadLocalFile>,
    file_ids: Vec<SlackComposerFileId>,
    locators: Vec<SlackFileStagingLocator>,
    draft_token: u64,
    attachments_became_visible: bool,
    selection_truncated: bool,
}

impl SurfaceState {
    pub(crate) fn remove_slack_thread_reply_attachment(
        &mut self,
        key: &SlackComposerDraftKey,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(handle) = self.slack_thread_draft_handle(key) else {
            return false;
        };
        self.remove_slack_thread_reply_attachment_for_handle(&handle, file_id, cx)
    }

    pub(crate) fn remove_slack_thread_reply_attachment_for_handle(
        &mut self,
        handle: &SlackThreadDraftHandle,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = handle.key();
        if self.slack_thread_reply_is_pending(key)
            || !self.slack_thread_draft_key_has_current_identity(key)
            || !self.slack_thread_draft_handle_exists(handle)
        {
            return false;
        }
        let draft_token = self.next_slack_composer_draft_token();
        let active_handle = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
            .map(|panel| SlackThreadDraftHandle::new(key.clone(), panel.reply_draft.borrow().id));
        if let Some(handle) = active_handle {
            return self.remove_active_slack_thread_attachment(&handle, file_id, draft_token, cx);
        }
        self.remove_stored_slack_thread_attachment(key, file_id, draft_token, cx)
    }

    fn remove_active_slack_thread_attachment(
        &mut self,
        handle: &SlackThreadDraftHandle,
        file_id: SlackComposerFileId,
        draft_token: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = handle.key();
        let Some(panel) = self
            .slack_thread_panel
            .as_mut()
            .filter(|panel| panel.reply_draft_key == *key)
        else {
            return false;
        };
        let draft = panel.reply_draft.get_mut();
        let Some(file) = draft.files.remove_file(file_id) else {
            return false;
        };
        draft.token = draft_token;
        draft.client_message_id = None;
        let error_became_hidden = panel.reply_error.take().is_some();
        panel.reply_composer_focused = true;
        if draft.files.is_empty() || error_became_hidden {
            panel.list_state.remeasure();
        }
        self.slack_composer_draft_changed(key.clone(), draft_token, cx);
        self.cleanup_removed_slack_thread_file(handle, file_id, file, cx);
        cx.notify();
        true
    }

    fn remove_stored_slack_thread_attachment(
        &mut self,
        key: &SlackComposerDraftKey,
        file_id: SlackComposerFileId,
        draft_token: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let (handle, file, attachments_became_hidden, remove_empty) = {
            let Some(draft) = self.slack_composer_drafts.get_mut(key) else {
                return false;
            };
            let handle = SlackThreadDraftHandle::new(key.clone(), draft.id);
            let Some(file) = draft.files.remove_file(file_id) else {
                return false;
            };
            draft.token = draft_token;
            draft.client_message_id = None;
            (handle, file, draft.files.is_empty(), draft.is_empty())
        };
        if remove_empty {
            self.slack_composer_drafts.remove(key);
        }
        self.slack_composer_draft_changed(key.clone(), draft_token, cx);
        self.cleanup_removed_slack_thread_file(&handle, file_id, file, cx);
        self.clear_slack_thread_draft_error(key);
        if attachments_became_hidden {
            self.remeasure_slack_all_threads_composer(key);
        }
        cx.notify();
        true
    }

    fn cleanup_removed_slack_thread_file(
        &mut self,
        handle: &SlackThreadDraftHandle,
        file_id: SlackComposerFileId,
        file: SlackComposerFile,
        cx: &mut Context<Self>,
    ) {
        if let Some(reference) = file.state().remote_reference().cloned() {
            let locator = SlackRemoteDraftFileLocator::thread(handle, file_id, reference);
            self.begin_slack_remote_draft_file_cleanup(locator, cx);
        } else if let Some(operation_id) = file.operation_id().cloned() {
            let locator = SlackFileStagingLocator::thread(handle, file_id, operation_id);
            self.remove_slack_file_staging_ownership(locator, file, cx);
        }
    }

    pub(crate) fn attach_slack_thread_upload_files_with_selection(
        &mut self,
        handle: &SlackThreadDraftHandle,
        files: Vec<SlackUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        self.attach_slack_thread_prepared_upload_files_with_selection(
            handle,
            files
                .into_iter()
                .map(SlackPreparedUploadFile::from_upload)
                .collect(),
            path_selection_truncated,
            cx,
        )
    }

    pub(crate) fn attach_slack_thread_prepared_upload_files_with_selection(
        &mut self,
        handle: &SlackThreadDraftHandle,
        files: Vec<SlackPreparedUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let prepared =
            self.prepare_slack_thread_attachments(handle, files, path_selection_truncated)?;
        let targets_active_panel = self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.reply_draft_key == *handle.key()
                && panel.reply_draft.borrow().id == handle.draft_id()
        });
        if targets_active_panel {
            self.apply_active_slack_thread_attachments(handle, prepared, cx)
        } else {
            self.apply_stored_slack_thread_attachments(handle, prepared, cx)
        }
    }

    fn prepare_slack_thread_attachments(
        &mut self,
        handle: &SlackThreadDraftHandle,
        files: Vec<SlackPreparedUploadFile>,
        path_selection_truncated: bool,
    ) -> Result<PreparedSlackThreadAttachments, String> {
        let current_file_count =
            self.slack_thread_attachment_current_file_count(handle, files.len())?;
        let remaining_slots = SLACK_COMPOSER_FILE_LIMIT - current_file_count;
        let attachments_became_visible = current_file_count == 0;
        let selection_truncated = path_selection_truncated || files.len() > remaining_slots;
        let files = files
            .into_iter()
            .take(remaining_slots)
            .map(|file| {
                (
                    self.next_slack_composer_file_id(),
                    crate::model::SlackFileStagingOperationId::generate(),
                    file,
                )
            })
            .collect::<Vec<_>>();
        let file_ids = files
            .iter()
            .map(|(file_id, _, _)| *file_id)
            .collect::<Vec<_>>();
        let locators = files
            .iter()
            .map(|(file_id, operation_id, _)| {
                SlackFileStagingLocator::thread(handle, *file_id, operation_id.clone())
            })
            .collect::<Vec<_>>();
        let draft_token = self.next_slack_composer_draft_token();
        Ok(PreparedSlackThreadAttachments {
            files,
            file_ids,
            locators,
            draft_token,
            attachments_became_visible,
            selection_truncated,
        })
    }

    fn slack_thread_attachment_current_file_count(
        &self,
        handle: &SlackThreadDraftHandle,
        incoming_file_count: usize,
    ) -> Result<usize, String> {
        if incoming_file_count == 0 {
            return Err("Select at least one file to attach.".to_string());
        }
        if !self.slack_workspace_api_capabilities.upload_files
            || !self.slack_workspace_api_capabilities.stage_file
        {
            return Err("File uploads are unavailable for this Slack workspace.".to_string());
        }
        if !self.slack_thread_draft_handle_exists(handle) {
            return Err(
                "The Slack thread draft is no longer available for this attachment operation."
                    .to_string(),
            );
        }
        if self.slack_thread_reply_is_pending(handle.key()) {
            return Err(
                "Wait for the current thread reply to finish before attaching files.".to_string(),
            );
        }
        let file_count = self
            .with_slack_thread_reply_draft(handle.key(), |draft| {
                (draft.id == handle.draft_id()).then_some(draft.files.len())
            })
            .flatten()
            .ok_or_else(|| {
                "The Slack thread draft moved before its file count was read.".to_string()
            })?;
        if file_count >= SLACK_COMPOSER_FILE_LIMIT {
            return Err(SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string());
        }
        Ok(file_count)
    }

    fn apply_active_slack_thread_attachments(
        &mut self,
        handle: &SlackThreadDraftHandle,
        prepared: PreparedSlackThreadAttachments,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let Some(panel) = self
            .slack_thread_panel
            .as_mut()
            .filter(|panel| panel.reply_draft_key == *handle.key())
        else {
            return Err(
                "The Slack thread draft moved before file attachment completed.".to_string(),
            );
        };
        if panel.reply_draft.borrow().id != handle.draft_id() {
            return Err(
                "The Slack thread draft moved before file attachment completed.".to_string(),
            );
        }
        let draft = panel.reply_draft.get_mut();
        push_slack_thread_local_files(draft, prepared.files);
        draft.token = prepared.draft_token;
        draft.client_message_id = None;
        let error_became_hidden = panel.reply_error.take().is_some();
        panel.reply_composer_focused = true;
        if prepared.attachments_became_visible || error_became_hidden {
            panel.list_state.remeasure();
        }
        self.slack_composer_draft_changed(handle.key().clone(), prepared.draft_token, cx);
        self.enqueue_slack_file_staging(prepared.locators, cx);
        self.finish_slack_thread_attachments(
            handle,
            prepared.file_ids,
            prepared.selection_truncated,
            cx,
        )
    }

    fn apply_stored_slack_thread_attachments(
        &mut self,
        handle: &SlackThreadDraftHandle,
        prepared: PreparedSlackThreadAttachments,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let Some(draft) = self
            .slack_composer_drafts
            .get_mut(handle.key())
            .filter(|draft| draft.id == handle.draft_id())
        else {
            return Err(
                "The stored Slack thread draft moved before file attachment completed.".to_string(),
            );
        };
        push_slack_thread_local_files(draft, prepared.files);
        draft.token = prepared.draft_token;
        draft.client_message_id = None;
        self.slack_composer_draft_changed(handle.key().clone(), prepared.draft_token, cx);
        self.clear_slack_thread_draft_error(handle.key());
        if prepared.attachments_became_visible {
            self.remeasure_slack_all_threads_composer(handle.key());
        }
        self.enqueue_slack_file_staging(prepared.locators, cx);
        self.finish_slack_thread_attachments(
            handle,
            prepared.file_ids,
            prepared.selection_truncated,
            cx,
        )
    }

    fn finish_slack_thread_attachments(
        &mut self,
        handle: &SlackThreadDraftHandle,
        file_ids: Vec<SlackComposerFileId>,
        selection_truncated: bool,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        if selection_truncated {
            self.show_slack_thread_draft_error(
                handle.key(),
                SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string(),
                cx,
            );
        }
        cx.notify();
        Ok(crate::model::ChatComposerFilesAttached {
            files: self.slack_thread_file_summaries(handle.key(), &file_ids)?,
            diagnostic: selection_truncated
                .then(|| SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string()),
        })
    }
}
