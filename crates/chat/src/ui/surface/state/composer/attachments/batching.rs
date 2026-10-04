use crate::model::SlackFileStagingOperationId;

use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackComposerDraft, SlackComposerDraftKey, SlackComposerFileId, SlackFileStagingLocator,
    SlackMainComposerDraftHandle, SlackMainComposerDraftOwner, SlackPreparedUploadFile,
    SLACK_COMPOSER_FILE_LIMIT, SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC,
};

type SlackPreparedMainAttachment = (
    SlackComposerFileId,
    SlackFileStagingOperationId,
    SlackPreparedUploadFile,
);

struct SlackPreparedMainAttachmentBatch {
    files: Vec<SlackPreparedMainAttachment>,
    file_ids: Vec<SlackComposerFileId>,
    locators: Vec<SlackFileStagingLocator>,
    selection_truncated: bool,
}

struct SlackMainAttachmentBatchRequest<'a> {
    handle: &'a SlackMainComposerDraftHandle,
    files: Vec<SlackPreparedUploadFile>,
    path_selection_truncated: bool,
    active: bool,
}

impl SurfaceState {
    pub(crate) fn attach_slack_prepared_upload_files_to_main_draft_with_selection(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        files: Vec<SlackPreparedUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let active = self.current_slack_main_composer_draft_handle().as_ref() == Some(handle);
        let batch = self.prepare_slack_main_attachment_batch(
            SlackMainAttachmentBatchRequest {
                handle,
                files,
                path_selection_truncated,
                active,
            },
            cx,
        )?;
        if active {
            return self.attach_active_slack_main_attachment_batch(handle, batch, cx);
        }
        self.attach_stored_slack_main_attachment_batch(handle, batch, cx)
    }

    fn attach_stored_slack_main_attachment_batch(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        batch: SlackPreparedMainAttachmentBatch,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let SlackPreparedMainAttachmentBatch {
            files,
            file_ids,
            locators,
            selection_truncated,
        } = batch;
        let draft_token = self.next_slack_composer_draft_token();
        if let Some(key) =
            self.attach_stored_slack_main_attachment_files(handle, files, draft_token)?
        {
            self.slack_composer_draft_changed(key, draft_token, cx);
        }
        self.enqueue_slack_file_staging(locators, cx);
        cx.notify();
        Ok(crate::model::ChatComposerFilesAttached {
            files: self.slack_main_file_summaries(handle, &file_ids)?,
            diagnostic: selection_truncated
                .then(|| SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string()),
        })
    }

    fn prepare_slack_main_attachment_batch(
        &mut self,
        request: SlackMainAttachmentBatchRequest<'_>,
        cx: &mut Context<Self>,
    ) -> Result<SlackPreparedMainAttachmentBatch, String> {
        let SlackMainAttachmentBatchRequest {
            handle,
            files,
            path_selection_truncated,
            active,
        } = request;
        self.validate_slack_main_attachment_batch(handle, &files, active, cx)?;
        let current_file_count = self
            .slack_main_draft_files(handle)
            .expect("validated Slack attachment draft must remain available")
            .len();
        let remaining_slots = SLACK_COMPOSER_FILE_LIMIT - current_file_count;
        let selection_truncated = path_selection_truncated || files.len() > remaining_slots;
        let files = files
            .into_iter()
            .take(remaining_slots)
            .map(|file| {
                (
                    self.next_slack_composer_file_id(),
                    SlackFileStagingOperationId::generate(),
                    file,
                )
            })
            .collect::<Vec<_>>();
        Ok(SlackPreparedMainAttachmentBatch {
            file_ids: files.iter().map(|(file_id, _, _)| *file_id).collect(),
            locators: files
                .iter()
                .map(|(file_id, operation_id, _)| {
                    SlackFileStagingLocator::main(handle, *file_id, operation_id.clone())
                })
                .collect(),
            files,
            selection_truncated,
        })
    }

    fn validate_slack_main_attachment_batch(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        files: &[SlackPreparedUploadFile],
        active: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return Err("Wait for Slack scheduling to finish before attaching files.".to_string());
        }
        if files.is_empty() {
            return Err("Select at least one file to attach.".to_string());
        }
        if !self.slack_workspace_api_capabilities.upload_files
            || !self.slack_workspace_api_capabilities.stage_file
        {
            return Err("File uploads are unavailable for this Slack workspace.".to_string());
        }
        let current_file_count = self
            .slack_main_draft_files(handle)
            .map(|files| files.len())
            .ok_or_else(|| {
                "The exact Slack composer draft is no longer available for attachments.".to_string()
            })?;
        if current_file_count < SLACK_COMPOSER_FILE_LIMIT {
            return Ok(());
        }
        if active {
            self.slack_error = Some(SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string());
            self.slack_composer_focused = true;
            cx.notify();
        }
        Err(SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string())
    }

    fn attach_active_slack_main_attachment_batch(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        batch: SlackPreparedMainAttachmentBatch,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let SlackPreparedMainAttachmentBatch {
            files,
            file_ids,
            locators,
            selection_truncated,
        } = batch;
        for (file_id, operation_id, prepared) in files {
            self.slack_composer_files
                .push_prepared(file_id, operation_id, prepared);
        }
        self.advance_slack_send_draft_revision();
        self.slack_main_composer_draft_changed(cx);
        self.slack_aux_panel = None;
        self.slack_composer_focused = true;
        self.slack_error =
            selection_truncated.then(|| SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string());
        self.enqueue_slack_file_staging(locators, cx);
        cx.notify();
        Ok(crate::model::ChatComposerFilesAttached {
            files: self.slack_main_file_summaries(handle, &file_ids)?,
            diagnostic: selection_truncated
                .then(|| SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC.to_string()),
        })
    }

    fn attach_stored_slack_main_attachment_files(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        files: Vec<SlackPreparedMainAttachment>,
        draft_token: u64,
    ) -> Result<Option<SlackComposerDraftKey>, String> {
        match &handle.owner {
            SlackMainComposerDraftOwner::Conversation(key) => {
                let return_matches = self.slack_new_message_return_draft.as_ref().is_some_and(
                    |(return_key, draft)| return_key == key && draft.id == handle.draft_id,
                );
                if return_matches {
                    let Some((_, draft)) = self.slack_new_message_return_draft.as_mut() else {
                        return Err(
                            "The Slack return draft moved before file insertion.".to_string()
                        );
                    };
                    push_slack_local_files(draft, files, draft_token);
                } else {
                    let Some(draft) = self
                        .slack_composer_drafts
                        .get_mut(key)
                        .filter(|draft| draft.id == handle.draft_id)
                    else {
                        return Err(
                            "The stored Slack conversation draft moved before file insertion."
                                .to_string(),
                        );
                    };
                    push_slack_local_files(draft, files, draft_token);
                }
                Ok(Some(key.clone()))
            }
            SlackMainComposerDraftOwner::NewMessage(key) => {
                let Some(draft) = self
                    .slack_new_message_drafts
                    .get_mut(key)
                    .filter(|draft| draft.id == handle.draft_id)
                else {
                    return Err(
                        "The stored Slack new-message draft moved before file insertion."
                            .to_string(),
                    );
                };
                push_slack_local_files(draft, files, draft_token);
                Ok(None)
            }
        }
    }
}

fn push_slack_local_files(
    draft: &mut SlackComposerDraft,
    files: Vec<SlackPreparedMainAttachment>,
    draft_token: u64,
) {
    for (file_id, operation_id, prepared) in files {
        draft.files.push_prepared(file_id, operation_id, prepared);
    }
    draft.token = draft_token;
    draft.client_message_id = None;
}
