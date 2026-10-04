use super::{
    Arc, Image, ListState, Range, RefCell, ScrollHandle, SharedString, SlackAttachment,
    SlackComposerDocument, SlackComposerFileState, SlackComposerLinkUrl,
    SlackFileStagingDraftOwner, SlackLaterItemKey, SlackMainTab, SlackMessageClientId,
    SlackMessageRow, SlackMessageTimestamp, SLACK_COMPOSER_FILE_LIMIT,
    SLACK_COMPOSER_FILE_LIMIT_DIAGNOSTIC,
};
use crate::ui::SlackUploadFile;

mod attachment_types;
mod drafts;
mod files;
mod foundation;

pub(crate) use attachment_types::{
    SlackComposerAttachmentKind, SlackComposerAttachmentPresentation,
    SlackComposerAttachmentPreview, SlackComposerAttachmentStatus,
};
pub use drafts::SlackMainComposerDraftHandle;
pub(crate) use drafts::{
    SlackActiveMainComposerContext, SlackComposerDraft, SlackComposerLinkDialog,
    SlackComposerLinkTarget, SlackMainComposerDraftOwner, SlackMainComposerNotice,
    SlackMainComposerPresentation, SlackMainComposerTarget, SlackScheduleButtonPresentation,
    SlackScheduleDraftOwner, SlackSendDraftSource, SlackThreadListRow, SlackThreadPanelOrigin,
    SlackThreadPanelState,
};
pub(crate) use files::SlackComposerFiles;
pub use foundation::{
    SlackAuxPanelQueryBehavior, SlackAuxPanelRow, SlackAuxPanelRowAction, SlackAuxPanelSection,
    SlackAuxPanelState, SlackComposerFormatAction, SlackPreparedUploadFile, SlackThreadDraftHandle,
};
pub(crate) use foundation::{
    SlackComposerDestination, SlackComposerDraftId, SlackComposerDraftKey, SlackComposerFileId,
    SlackComposerIdAllocator, SlackComposerTarget, SlackMentionInsertionMode,
    SlackMentionPickerState, SlackNewMessageDraftKey, SlackPendingThreadReplySend,
    SlackReplyComposerTarget,
};

#[derive(Clone, Debug)]
pub(crate) struct SlackComposerFile {
    id: SlackComposerFileId,
    attachment: Arc<SlackAttachment>,
    title: SharedString,
    kind: SlackComposerAttachmentKind,
    preview: Option<SlackComposerAttachmentPreview>,
    state: SlackComposerFileState,
}

impl SlackComposerFile {
    fn queued(
        id: SlackComposerFileId,
        operation_id: crate::model::SlackFileStagingOperationId,
        upload: SlackUploadFile,
        preview: Option<Arc<Image>>,
    ) -> Self {
        let title = SharedString::from(upload.name().to_string());
        let mimetype = upload.mimetype().to_string();
        let kind = SlackComposerAttachmentKind::from_mimetype(&mimetype);
        let attachment = SlackAttachment {
            title: upload.name().to_string(),
            source: Default::default(),
            media: None,
            mimetype,
            duration_millis: None,
            description: String::new(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        };
        Self {
            id,
            attachment: Arc::new(attachment),
            title,
            kind,
            preview: preview.map(SlackComposerAttachmentPreview::Local),
            state: SlackComposerFileState::Queued {
                operation_id,
                upload,
            },
        }
    }

    fn remote_loading(
        id: SlackComposerFileId,
        reference: crate::model::SlackRemoteDraftFileReference,
    ) -> Self {
        let title = SharedString::from("Loading file…");
        let attachment = SlackAttachment {
            title: title.to_string(),
            source: Default::default(),
            media: None,
            mimetype: String::new(),
            duration_millis: None,
            description: String::new(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        };
        Self {
            id,
            attachment: Arc::new(attachment),
            title,
            kind: SlackComposerAttachmentKind::File,
            preview: None,
            state: SlackComposerFileState::RemoteLoading { reference },
        }
    }

    #[cfg(test)]
    fn fixture(id: SlackComposerFileId, attachment: SlackAttachment) -> Self {
        let title = SharedString::from(attachment.title.clone());
        let kind = SlackComposerAttachmentKind::from_mimetype(&attachment.mimetype);
        let preview = attachment
            .preview_image_url
            .as_ref()
            .map(|url| SlackComposerAttachmentPreview::Remote(SharedString::from(url.clone())));
        Self {
            id,
            attachment: Arc::new(attachment),
            title,
            kind,
            preview,
            state: SlackComposerFileState::Fixture,
        }
    }

    pub(crate) fn id(&self) -> SlackComposerFileId {
        self.id
    }

    pub(crate) fn attachment(&self) -> &SlackAttachment {
        &self.attachment
    }

    pub(crate) fn control_summary(&self) -> crate::model::ChatComposerFileSummary {
        crate::model::ChatComposerFileSummary {
            file_id: self.id.to_string(),
            title: self.attachment.title.clone(),
            mimetype: self.attachment.mimetype.clone(),
            status: self.state.summary_status(),
            operation_id: self.state.operation_id().map(ToString::to_string),
            pending_retry_operation_id: self
                .state
                .pending_retry_operation_id()
                .map(ToString::to_string),
            diagnostic: self.state.diagnostic(),
        }
    }

    pub(crate) fn state(&self) -> &SlackComposerFileState {
        &self.state
    }

    pub(crate) fn state_mut(&mut self) -> &mut SlackComposerFileState {
        &mut self.state
    }

    pub(crate) fn replace_attachment(&mut self, attachment: Arc<SlackAttachment>) {
        self.title = SharedString::from(attachment.title.clone());
        self.kind = SlackComposerAttachmentKind::from_mimetype(&attachment.mimetype);
        self.preview = attachment
            .preview_image_url
            .as_ref()
            .map(|url| SlackComposerAttachmentPreview::Remote(SharedString::from(url.clone())));
        self.attachment = attachment;
    }

    pub(crate) fn presentation(&self) -> SlackComposerAttachmentPresentation {
        let status = match self.state {
            SlackComposerFileState::Queued { .. }
            | SlackComposerFileState::Uploading { .. }
            | SlackComposerFileState::Recovering { .. }
            | SlackComposerFileState::RemoteLoading { .. } => {
                SlackComposerAttachmentStatus::Loading
            }
            SlackComposerFileState::Staged { .. } | SlackComposerFileState::RemoteReady { .. } => {
                SlackComposerAttachmentStatus::Ready
            }
            SlackComposerFileState::Error { .. }
            | SlackComposerFileState::RetainedUnknown { .. }
            | SlackComposerFileState::RetryBlocked { .. }
            | SlackComposerFileState::AlreadyShared { .. }
            | SlackComposerFileState::AlreadyDraftOwned { .. }
            | SlackComposerFileState::RemoteError { .. } => SlackComposerAttachmentStatus::Error,
            #[cfg(test)]
            SlackComposerFileState::Fixture => SlackComposerAttachmentStatus::Ready,
        };
        SlackComposerAttachmentPresentation {
            file_id: self.id,
            title: self.title.clone(),
            kind: self.kind,
            preview: self.preview.clone(),
            status,
        }
    }

    pub(crate) fn into_state(self) -> SlackComposerFileState {
        self.state
    }

    pub(crate) fn operation_id(&self) -> Option<&crate::model::SlackFileStagingOperationId> {
        self.state.operation_id()
    }
}
