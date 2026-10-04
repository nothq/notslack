mod mutation;
mod selection;

use super::super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackAttachmentPathSelection, SlackComposerDraft, SlackComposerDraftKey, SlackComposerFileId,
    SlackPreparedUploadFile, SlackThreadDraftHandle,
};

impl SurfaceState {
    pub(crate) fn current_slack_thread_reply_draft_key(&self) -> Option<SlackComposerDraftKey> {
        self.slack_thread_panel
            .as_ref()
            .filter(|panel| self.slack_thread_panel_origin_is_current(panel))
            .map(|panel| panel.reply_draft_key.clone())
    }

    pub(crate) fn slack_thread_reply_draft_has_files(&self, key: &SlackComposerDraftKey) -> bool {
        self.with_slack_thread_reply_draft(key, |draft| !draft.files.is_empty())
            .unwrap_or(false)
    }

    pub(crate) fn slack_thread_reply_draft_has_content(&self, key: &SlackComposerDraftKey) -> bool {
        self.with_slack_thread_reply_draft(key, |draft| {
            !draft.text().trim().is_empty() || !draft.files.is_empty()
        })
        .unwrap_or(false)
    }

    pub(crate) fn slack_thread_reply_draft_is_sendable(&self, key: &SlackComposerDraftKey) -> bool {
        let share_files = self.slack_workspace_api_capabilities.share_files;
        self.with_slack_thread_reply_draft(key, |draft| {
            (!draft.text().trim().is_empty() || !draft.files.is_empty())
                && draft.files.slack_file_ids_ready()
                && (draft.files.is_empty() || share_files)
        })
        .unwrap_or(false)
    }

    pub(crate) fn with_slack_thread_reply_draft<T>(
        &self,
        key: &SlackComposerDraftKey,
        read: impl FnOnce(&SlackComposerDraft) -> T,
    ) -> Option<T> {
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            let draft = panel.reply_draft.borrow();
            return Some(read(&draft));
        }
        self.slack_composer_drafts.get(key).map(read)
    }

    pub(crate) fn slack_thread_attachment_path_selection(
        &self,
        handle: &SlackThreadDraftHandle,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<SlackAttachmentPathSelection, String> {
        let attached_file_count = self
            .with_slack_thread_reply_draft(handle.key(), |draft| {
                (draft.id == handle.draft_id()).then_some(draft.files.len())
            })
            .flatten()
            .ok_or_else(|| {
                "The Slack thread draft moved before its file count was read.".to_string()
            })?;
        SlackAttachmentPathSelection::for_attached_file_count(paths, attached_file_count)
    }

    fn slack_thread_file_summaries(
        &self,
        key: &SlackComposerDraftKey,
        file_ids: &[SlackComposerFileId],
    ) -> Result<Vec<crate::model::ChatComposerFileSummary>, String> {
        self.with_slack_thread_reply_draft(key, |draft| {
            file_ids
                .iter()
                .map(|file_id| {
                    draft
                        .files
                        .iter()
                        .find(|file| file.id() == *file_id)
                        .ok_or_else(|| {
                            format!(
                                "Slack composer file {file_id} moved before its attachment summary was read."
                            )
                        })
                        .map(|file| file.control_summary())
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .ok_or_else(|| {
            "Slack thread draft moved before its attachment summary was read.".to_string()
        })?
    }

    fn slack_thread_draft_key_has_current_identity(&self, key: &SlackComposerDraftKey) -> bool {
        self.slack_composer_draft_key(key.destination.clone())
            .as_ref()
            == Some(key)
    }

    pub(crate) fn slack_thread_draft_handle(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<SlackThreadDraftHandle> {
        self.with_slack_thread_reply_draft(key, |draft| {
            SlackThreadDraftHandle::new(key.clone(), draft.id)
        })
    }

    fn ensure_slack_thread_draft_handle(
        &mut self,
        key: SlackComposerDraftKey,
    ) -> SlackThreadDraftHandle {
        if let Some(handle) = self.slack_thread_draft_handle(&key) {
            return handle;
        }
        let draft_id = self.next_slack_composer_draft_id();
        self.slack_composer_drafts
            .insert(key.clone(), SlackComposerDraft::new(draft_id));
        SlackThreadDraftHandle::new(key, draft_id)
    }

    fn slack_thread_draft_handle_exists(&self, handle: &SlackThreadDraftHandle) -> bool {
        self.slack_thread_draft_key_has_current_identity(handle.key())
            && self
                .with_slack_thread_reply_draft(handle.key(), |draft| draft.id == handle.draft_id())
                .unwrap_or(false)
    }

    fn remove_empty_slack_thread_draft_handle(&mut self, handle: &SlackThreadDraftHandle) {
        let remove = self
            .slack_composer_drafts
            .get(handle.key())
            .is_some_and(|draft| draft.id == handle.draft_id() && draft.is_empty());
        if remove {
            self.slack_composer_drafts.remove(handle.key());
        }
    }

    fn show_slack_thread_draft_error(
        &mut self,
        key: &SlackComposerDraftKey,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self
            .slack_thread_panel
            .as_mut()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            let error_became_visible = panel.reply_error.is_none();
            panel.reply_error = Some(error);
            panel.reply_composer_focused = true;
            if error_became_visible {
                panel.list_state.remeasure();
            }
            cx.notify();
            return;
        }
        if let Some(thread_key) = self.slack_all_threads_surface_thread_key(key) {
            self.slack_all_threads_reply_errors
                .insert(thread_key, error);
            cx.notify();
        }
    }

    fn clear_slack_thread_draft_error(&mut self, key: &SlackComposerDraftKey) {
        if let Some(panel) = self
            .slack_thread_panel
            .as_mut()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            if panel.reply_error.take().is_some() {
                panel.list_state.remeasure();
            }
            return;
        }
        if let Some(thread_key) = self.slack_all_threads_surface_thread_key(key) {
            self.slack_all_threads_reply_errors.remove(&thread_key);
        }
    }

    fn remeasure_slack_all_threads_composer(&mut self, key: &SlackComposerDraftKey) {
        let Some(thread_key) = self.slack_all_threads_surface_thread_key(key) else {
            return;
        };
        let Some(index) = self
            .slack_all_threads_rows
            .iter()
            .position(|row| row.key.as_ref() == thread_key)
        else {
            return;
        };
        self.slack_all_threads_list_state
            .remeasure_items(index..index + 1);
    }
}

fn push_slack_thread_local_files(
    draft: &mut SlackComposerDraft,
    files: Vec<(
        SlackComposerFileId,
        crate::model::SlackFileStagingOperationId,
        SlackPreparedUploadFile,
    )>,
) {
    for (file_id, operation_id, prepared) in files {
        draft.files.push_prepared(file_id, operation_id, prepared);
    }
}
