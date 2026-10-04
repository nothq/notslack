use super::{
    Arc, Entity, SharedString, SlackAttachment, SlackAttachmentMediaKind, SlackMainRoute,
    SlackMainTab, SlackMessageActionTarget, SlackMessageBody, SlackMessageElementIds,
    SlackMessageRenderContext, SlackMessageTimestamp, SlackReaction, SlackReactionMutation,
    SlackReactionName, SlackSavedMessageMutation, SlackShellIcon,
};
use crate::ui::{AudioPreviewPlayer, SlackRailView, VideoPlayer, WorkspaceApi};
use gpui::Subscription;

mod message_actions;

pub(crate) use message_actions::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackAttachmentRow {
    pub(crate) attachment: SlackAttachment,
    pub(crate) attachment_id: SharedString,
    pub(crate) title: SharedString,
    pub(crate) shows_title: bool,
    pub(crate) kind: SlackAttachmentRenderKind,
    pub(crate) recording_duration_label: Option<SharedString>,
    pub(crate) preview_cache_key: Option<SharedString>,
    pub(crate) preview_size: Option<crate::ui::SlackAttachmentPreviewSize>,
    pub(crate) preview_max_width: u32,
    pub(crate) preview_max_height: u32,
    pub(crate) collapsible_file_preview: bool,
    pub(crate) shared_message: Option<SlackSharedMessageAttachmentRow>,
    pub(crate) legacy_title_body: Option<SlackMessageBody>,
    pub(crate) legacy_description_body: Option<SlackMessageBody>,
}

pub(crate) fn slack_inline_video_dimensions(attachment: &SlackAttachmentRow) -> (f32, f32) {
    const FALLBACK_WIDTH: f32 = 360.0;
    const FALLBACK_HEIGHT: f32 = 202.0;
    const MAX_WIDTH: f32 = 440.0;
    const MAX_HEIGHT: f32 = 354.0;

    attachment
        .preview_size
        .map_or((FALLBACK_WIDTH, FALLBACK_HEIGHT), |size| {
            let width = size.width() as f32;
            let height = size.height() as f32;
            let scale = (MAX_WIDTH / width).min(MAX_HEIGHT / height).min(1.0);
            (width * scale, height * scale)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackAttachmentSelection {
    pub(crate) attachment_id: SharedString,
    pub(crate) title: SharedString,
    pub(crate) attachment: SlackAttachment,
}

impl SlackAttachmentSelection {
    pub(crate) fn from_row(row: &SlackAttachmentRow) -> Self {
        Self {
            attachment_id: row.attachment_id.clone(),
            title: row.title.clone(),
            attachment: row.attachment.clone(),
        }
    }

    pub(crate) fn from_shared_file(row: &SlackSharedMessageFileRow) -> Self {
        Self {
            attachment_id: row.attachment_id.clone(),
            title: row.title.clone(),
            attachment: row.attachment.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackMediaReconciliationKey {
    pub(crate) conversation_revision: u64,
    pub(crate) main_route: SlackMainRoute,
    pub(crate) rail_view: SlackRailView,
    pub(crate) main_tab: SlackMainTab,
    pub(crate) search_results_open: bool,
    pub(crate) search_generation: u64,
    pub(crate) thread_generation: u64,
    pub(crate) thread_reply_generation: u64,
    pub(crate) all_threads_generation: u64,
    pub(crate) activity_detail_generation: u64,
    pub(crate) later_generation: u64,
    pub(crate) pins_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackAttachmentLayoutRow {
    Single { attachment_index: usize },
    FileGallery(SlackFileGalleryRow),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackFileGalleryRow {
    pub(crate) attachment_id: SharedString,
    pub(crate) count_label: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) lines: Arc<[SlackFileGalleryLineRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackFileGalleryLineRow {
    pub(crate) height: u32,
    pub(crate) cells: Arc<[SlackFileGalleryCellRow]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackFileGalleryCellRow {
    pub(crate) attachment_index: usize,
    pub(crate) width: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSharedMessageAttachmentRow {
    pub(crate) author_name: SharedString,
    pub(crate) author_avatar_text: SharedString,
    pub(crate) author_avatar_fill: u32,
    pub(crate) author_avatar_image_url: Option<SharedString>,
    pub(crate) body: SharedString,
    pub(crate) channel_label: Option<SharedString>,
    pub(crate) timestamp_label: Option<SharedString>,
    pub(crate) permalink: Option<SharedString>,
    pub(crate) files: Arc<[SlackSharedMessageFileRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSharedMessageFileRow {
    pub(crate) attachment_id: SharedString,
    pub(crate) title: SharedString,
    pub(crate) attachment: SlackAttachment,
    pub(crate) link_url: SharedString,
    pub(crate) preview_cache_key: Option<SharedString>,
    pub(crate) preview_width: u32,
    pub(crate) preview_height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackAttachmentRenderKind {
    Recording,
    SharedMessage,
    WebsitePreview,
    FileCard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMediaTarget {
    pub(crate) attachment_id: SharedString,
    pub(crate) file_id: SharedString,
    pub(crate) title: SharedString,
    pub(crate) kind: SlackAttachmentMediaKind,
    pub(crate) duration_millis: Option<u32>,
}

impl SlackMediaTarget {
    pub(crate) fn from_selection(selection: &SlackAttachmentSelection) -> Option<Self> {
        let media = selection.attachment.media.as_ref()?;
        Some(Self {
            attachment_id: selection.attachment_id.clone(),
            file_id: media.file_id().to_string().into(),
            title: selection.title.clone(),
            kind: media.kind(),
            duration_millis: selection
                .attachment
                .duration_millis
                .map(std::num::NonZeroU32::get),
        })
    }

    pub(crate) fn matches(&self, attachment_id: &str, file_id: &str) -> bool {
        self.attachment_id.as_ref() == attachment_id && self.file_id.as_ref() == file_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMediaHostContext {
    Message(SlackMessageRenderContext),
    Search { generation: u64 },
    Lightbox,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMediaHostId {
    pub(crate) context: SlackMediaHostContext,
    pub(crate) attachment_id: SharedString,
}

impl SlackMediaHostId {
    pub(crate) fn message(context: SlackMessageRenderContext, attachment_id: SharedString) -> Self {
        Self {
            context: SlackMediaHostContext::Message(context),
            attachment_id,
        }
    }

    pub(crate) fn lightbox(attachment_id: SharedString) -> Self {
        Self {
            context: SlackMediaHostContext::Lightbox,
            attachment_id,
        }
    }

    pub(crate) fn search(generation: u64, attachment_id: SharedString) -> Self {
        Self {
            context: SlackMediaHostContext::Search { generation },
            attachment_id,
        }
    }
}

pub(crate) struct SlackAudioPlayer {
    pub(crate) player: AudioPreviewPlayer,
    pub(crate) target: SlackMediaTarget,
    pub(crate) refresh_scheduled: bool,
    pub(crate) failure_emitted: bool,
}

pub(crate) enum SlackAudioPlayerEvent {
    Failed,
}

pub(crate) enum SlackMediaPlayback {
    Loading {
        generation: u64,
        target: SlackMediaTarget,
        host: SlackMediaHostId,
    },
    Video {
        target: SlackMediaTarget,
        host: SlackMediaHostId,
        player: Entity<VideoPlayer>,
        source: crate::model::SlackPreparedMediaSource,
        workspace_api: Arc<dyn WorkspaceApi>,
    },
    Audio {
        target: SlackMediaTarget,
        host: SlackMediaHostId,
        player: Entity<SlackAudioPlayer>,
        _subscription: Subscription,
        source: crate::model::SlackPreparedMediaSource,
        workspace_api: Arc<dyn WorkspaceApi>,
    },
    Failed {
        target: SlackMediaTarget,
        host: SlackMediaHostId,
        error: SharedString,
    },
}

impl SlackMediaPlayback {
    pub(crate) fn target(&self) -> &SlackMediaTarget {
        match self {
            Self::Loading { target, .. }
            | Self::Video { target, .. }
            | Self::Audio { target, .. }
            | Self::Failed { target, .. } => target,
        }
    }

    pub(crate) fn host(&self) -> &SlackMediaHostId {
        match self {
            Self::Loading { host, .. }
            | Self::Video { host, .. }
            | Self::Audio { host, .. }
            | Self::Failed { host, .. } => host,
        }
    }

    pub(crate) fn set_host(&mut self, host: SlackMediaHostId) {
        match self {
            Self::Loading {
                host: current_host, ..
            }
            | Self::Video {
                host: current_host, ..
            }
            | Self::Audio {
                host: current_host, ..
            }
            | Self::Failed {
                host: current_host, ..
            } => *current_host = host,
        }
    }

    pub(crate) fn attachment_id(&self) -> &str {
        self.target().attachment_id.as_ref()
    }

    pub(crate) fn file_id(&self) -> &str {
        self.target().file_id.as_ref()
    }
}
