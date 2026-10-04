use super::super::super::super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackComposerDraftKey, SlackComposerFileId, SlackFileStagingLocator,
    SlackRemoteDraftFileLocator,
};

impl SurfaceState {
    pub(crate) fn retry_slack_thread_reply_attachment(
        &mut self,
        key: &SlackComposerDraftKey,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatComposerFileSummary, String> {
        if self.slack_thread_reply_is_pending(key)
            || !self.slack_thread_draft_key_has_current_identity(key)
        {
            return Err("The active Slack thread cannot retry files.".to_string());
        }
        let handle = self
            .slack_thread_draft_handle(key)
            .ok_or_else(|| "No active Slack thread draft accepts file retry.".to_string())?;
        let (remote_reference, operation_id) = self
            .with_slack_thread_reply_draft(key, |draft| {
                (draft.id == handle.draft_id())
                    .then(|| {
                        draft.files.file(file_id).map(|file| {
                            (
                                file.state().remote_reference().cloned(),
                                file.operation_id().cloned(),
                            )
                        })
                    })
                    .flatten()
            })
            .flatten()
            .ok_or_else(|| format!("Slack thread composer file {file_id} was not found."))?;
        if let Some(reference) = remote_reference {
            let locator = SlackRemoteDraftFileLocator::thread(&handle, file_id, reference);
            self.retry_slack_remote_draft_file_load(locator, cx)?;
            return self
                .with_slack_thread_reply_draft(key, |draft| {
                    draft.files.file(file_id).map(|file| file.control_summary())
                })
                .flatten()
                .ok_or_else(|| {
                    format!("Slack thread composer file {file_id} moved while retry was starting.")
                });
        }
        let operation_id = operation_id
            .ok_or_else(|| format!("Slack thread composer file {file_id} is not retryable."))?;
        let locator = SlackFileStagingLocator::thread(&handle, file_id, operation_id);
        self.retry_slack_file_staging(locator, cx)?;
        self.with_slack_thread_reply_draft(key, |draft| {
            draft.files.file(file_id).map(|file| file.control_summary())
        })
        .flatten()
        .ok_or_else(|| {
            format!("Slack thread composer file {file_id} moved while retry was starting.")
        })
    }
}
