mod image_cache;
mod model_types;
mod support;
mod surface;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

use crate::model::{SlackWorkspaceApi as WorkspaceApi, *};
pub(crate) use app_model::{AppearanceMode, SurfaceTheme};
pub(crate) use gpui::{
    div, img, list, point, prelude::FluentBuilder, px, relative, rgb, AnyElement, App, AppContext,
    BoxShadow, Context, Div, Entity, FocusHandle, Focusable, FontWeight, Image, InteractiveElement,
    IntoElement, KeyDownEvent, ListAlignment, ListScrollEvent, ListSizingBehavior, ListState,
    MouseButton, MouseDownEvent, ParentElement, Pixels, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Window,
};
pub(crate) use gpui_components::{
    alpha, spawn_background_task_for_entity, spawn_timer_task_for_entity,
};
pub use media_capture::{
    AudioClipCaptureStatus, AudioClipFailure, AudioClipSessionId, CapturedAudioClip,
    CapturedVideoClip, MediaCaptureCapabilities, MediaCaptureError, MediaCaptureService,
    VideoClipCaptureStatus, VideoClipPreview, VideoClipSessionId, VideoClipSnapshot,
    MAX_AUDIO_CLIP_DURATION,
};
pub(crate) use std::sync::OnceLock;
pub(crate) use time::Date;
pub(crate) use video_playback::{
    AudioPreviewPlayer, AudioPreviewState, VideoFrameFit, VideoPlayer, VideoPlayerConfig,
    VideoPlayerSource,
};

pub fn load_slack_upload_files_from_paths(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    paths: Vec<std::path::PathBuf>,
) -> Result<Vec<SlackUploadFile>, String> {
    surface::load_slack_upload_files_from_paths(local_file_api, paths)
}

pub fn load_slack_prepared_upload_files_from_paths(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    paths: Vec<std::path::PathBuf>,
) -> Result<Vec<SlackPreparedUploadFile>, String> {
    surface::load_slack_prepared_upload_files_from_paths(local_file_api, paths)
}

pub(crate) const SLACK_TABS_HEIGHT: f32 = 38.0;
pub(crate) const SLACK_WORKSPACE_RAIL_WIDTH: f32 = 60.0;
pub(crate) const SLACK_NAV_RAIL_WIDTH: f32 = 70.0;
pub(crate) const SLACK_TOP_NAV_RAIL_WIDTH: f32 = SLACK_WORKSPACE_RAIL_WIDTH + SLACK_NAV_RAIL_WIDTH;
pub(crate) const SLACK_SIDEBAR_DEFAULT_RATIO: f32 = 0.38;
pub(crate) const SLACK_SIDEBAR_MIN_WIDTH: f32 = 320.0;
pub(crate) const SLACK_SIDEBAR_MAX_WIDTH: f32 = 720.0;
pub(crate) const SLACK_MAIN_MIN_WIDTH: f32 = 360.0;
pub(crate) const SLACK_WORKSPACE_FRAME_RIGHT_MARGIN: f32 = 4.0;
pub(crate) const SLACK_MESSAGE_GAP: f32 = 8.0;
/// Slack lays out every text line box as `font_size * SLACK_LINE_HEIGHT`.
pub(crate) const SLACK_LINE_HEIGHT: f32 = 1.466_67;
pub(crate) const SLACK_MESSAGE_REPLY_PARTICIPANT_VISIBLE_LIMIT: usize = 3;
pub(crate) const SLACK_PROFILE_PANEL_WIDTH: f32 = 332.0;

pub(crate) use image_cache::*;
pub use model_types::*;
pub(crate) use support::*;
pub use support::{
    initials, render_chat_empty_state, render_slack_empty_state, slack_avatar_fill,
    slack_channel_meta, slack_sidebar_icon, slack_workspace_title,
};
pub use surface::{
    ChatEventSource, SlackAttachmentPathSelection, SlackAuxPanelQueryBehavior, SlackAuxPanelRow,
    SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState, SlackComposerFormatAction,
    SlackFilesFilter, SlackMainComposerDraftHandle, SlackMainTab, SlackPreparedUploadFile,
    SlackRailView, SlackThreadDraftHandle, SurfaceInput, SurfaceRoot, SurfaceState,
};
#[cfg(test)]
pub use surface::{SlackMediaState, SlackPlaybackSpeed};

impl gpui::EventEmitter<crate::model::ChatSurfaceEvent> for SurfaceState {}
impl gpui::EventEmitter<crate::model::ChatSurfaceEvent> for ChatEventSource {}
