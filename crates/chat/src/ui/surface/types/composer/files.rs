use super::{
    Arc, ScrollHandle, SlackAttachment, SlackComposerFile, SlackComposerFileId,
    SlackComposerFileState, SlackPreparedUploadFile, SLACK_COMPOSER_FILE_LIMIT,
    SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct SlackComposerFiles {
    ordered: Vec<SlackComposerFile>,
    scroll_handle: ScrollHandle,
}

impl SlackComposerFiles {
    pub(crate) fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.ordered.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SlackComposerFile> {
        self.ordered.iter()
    }

    pub(crate) fn scroll_handle(&self) -> ScrollHandle {
        self.scroll_handle.clone()
    }

    pub(crate) fn same_identity_and_order(&self, other: &Self) -> bool {
        self.ordered.len() == other.ordered.len()
            && self
                .ordered
                .iter()
                .zip(&other.ordered)
                .all(|(left, right)| left.id == right.id)
    }

    pub(crate) fn push_prepared(
        &mut self,
        id: SlackComposerFileId,
        operation_id: crate::model::SlackFileStagingOperationId,
        prepared: SlackPreparedUploadFile,
    ) {
        let (upload, preview) = prepared.into_parts();
        self.ordered
            .push(SlackComposerFile::queued(id, operation_id, upload, preview));
    }

    pub(crate) fn push_remote_loading(
        &mut self,
        id: SlackComposerFileId,
        reference: crate::model::SlackRemoteDraftFileReference,
    ) {
        self.ordered
            .push(SlackComposerFile::remote_loading(id, reference));
    }

    pub(crate) fn prepend_remote_loading(
        &mut self,
        files: Vec<(
            SlackComposerFileId,
            crate::model::SlackRemoteDraftFileReference,
        )>,
    ) {
        let mut remote = files
            .into_iter()
            .map(|(id, reference)| SlackComposerFile::remote_loading(id, reference))
            .collect::<Vec<_>>();
        remote.append(&mut self.ordered);
        self.ordered = remote;
    }

    #[cfg(test)]
    pub(crate) fn push_fixture(&mut self, id: SlackComposerFileId, attachment: SlackAttachment) {
        self.ordered
            .push(SlackComposerFile::fixture(id, attachment));
    }

    pub(crate) fn remove_file(&mut self, id: SlackComposerFileId) -> Option<SlackComposerFile> {
        let index = self.ordered.iter().position(|file| file.id == id)?;
        Some(self.ordered.remove(index))
    }

    pub(crate) fn file(&self, id: SlackComposerFileId) -> Option<&SlackComposerFile> {
        self.ordered.iter().find(|file| file.id == id)
    }

    pub(crate) fn file_mut(&mut self, id: SlackComposerFileId) -> Option<&mut SlackComposerFile> {
        self.ordered.iter_mut().find(|file| file.id == id)
    }

    pub(crate) fn has_local_pending(&self) -> bool {
        self.ordered.iter().any(|file| {
            matches!(
                file.state(),
                SlackComposerFileState::Queued { .. }
                    | SlackComposerFileState::Uploading { .. }
                    | SlackComposerFileState::Error { .. }
                    | SlackComposerFileState::Recovering { .. }
                    | SlackComposerFileState::RetainedUnknown { .. }
                    | SlackComposerFileState::RetryBlocked { .. }
                    | SlackComposerFileState::AlreadyShared { .. }
                    | SlackComposerFileState::AlreadyDraftOwned { .. }
            )
        })
    }

    pub(crate) fn validate_slack_file_limit(&self) -> Result<(), &'static str> {
        if self.ordered.len() > SLACK_COMPOSER_FILE_LIMIT {
            Err(SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC)
        } else {
            Ok(())
        }
    }

    pub(crate) fn projected_slack_file_ids(
        &self,
    ) -> Result<Arc<[crate::model::SlackFileId]>, String> {
        self.validate_slack_file_limit().map_err(str::to_string)?;
        self.ordered
            .iter()
            .map(|file| {
                file.state()
                    .projected_slack_file_id()
                    .cloned()
                    .ok_or_else(|| {
                        "Slack draft files have not all completed local staging.".to_string()
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Arc::from)
    }

    pub(crate) fn scheduled_local_files(
        &self,
    ) -> Arc<[crate::model::SlackScheduledDraftLocalFile]> {
        self.ordered
            .iter()
            .filter_map(|file| {
                let staged = file.state().staged()?;
                Some(crate::model::SlackScheduledDraftLocalFile::new(
                    staged.operation_id().clone(),
                    staged.file_id().clone(),
                ))
            })
            .collect::<Vec<_>>()
            .into()
    }

    pub(crate) fn slack_file_ids_ready(&self) -> bool {
        self.validate_slack_file_limit().is_ok()
            && self
                .ordered
                .iter()
                .all(|file| file.state().projected_slack_file_id().is_some())
    }

    pub(crate) fn staged_files(&self) -> Result<Vec<crate::ui::SlackStagedFile>, String> {
        self.ordered
            .iter()
            .map(|file| {
                file.state()
                    .staged()
                    .cloned()
                    .ok_or_else(|| "Slack draft files have not all completed staging.".to_string())
            })
            .collect()
    }

    pub(crate) fn all_staged(&self) -> bool {
        !self.ordered.is_empty()
            && self
                .ordered
                .iter()
                .all(|file| file.state().staged().is_some())
    }

    pub(crate) fn cloned_attachments(&self) -> Vec<SlackAttachment> {
        self.ordered
            .iter()
            .map(|file| file.attachment.as_ref().clone())
            .collect()
    }

    pub(crate) fn control_summaries(&self) -> Vec<crate::model::ChatComposerFileSummary> {
        self.ordered
            .iter()
            .map(SlackComposerFile::control_summary)
            .collect()
    }

    pub(crate) fn into_files(self) -> impl Iterator<Item = SlackComposerFile> {
        self.ordered.into_iter()
    }
}
