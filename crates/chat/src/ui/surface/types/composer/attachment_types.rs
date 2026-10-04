use super::{Arc, Image, SharedString, SlackComposerFileId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackComposerAttachmentKind {
    Image,
    Video,
    Audio,
    Pdf,
    Text,
    File,
}

impl SlackComposerAttachmentKind {
    pub(super) fn from_mimetype(mimetype: &str) -> Self {
        if mimetype.starts_with("image/") {
            Self::Image
        } else if mimetype.starts_with("video/") {
            Self::Video
        } else if mimetype.starts_with("audio/") {
            Self::Audio
        } else if mimetype == "application/pdf" {
            Self::Pdf
        } else if mimetype.starts_with("text/")
            || matches!(mimetype, "application/json" | "text/csv")
        {
            Self::Text
        } else {
            Self::File
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SlackComposerAttachmentPreview {
    Local(Arc<Image>),
    Remote(SharedString),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackComposerAttachmentStatus {
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackComposerAttachmentPresentation {
    pub(crate) file_id: SlackComposerFileId,
    pub(crate) title: SharedString,
    pub(crate) kind: SlackComposerAttachmentKind,
    pub(crate) preview: Option<SlackComposerAttachmentPreview>,
    pub(crate) status: SlackComposerAttachmentStatus,
}
