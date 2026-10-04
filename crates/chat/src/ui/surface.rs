use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use app_model::SurfaceFrame;
use gpui::ElementId;
use gpui_components::text_input::TextInput;

use crate::ui::{
    alpha, build_slack_conversation_remote_images, build_slack_message_remote_images,
    build_slack_remote_image_from_parts, build_slack_remote_images,
    build_slack_shell_remote_images, build_slack_sidebar_remote_images, div, img,
    keystroke_input_text, list, point, px, relative, rgb, slack_remote_image_cache_key,
    spawn_background_task_for_entity, spawn_timer_task_for_entity, svg_from_body, svg_with_paths,
    AnyElement, App, AppContext, AppearanceMode, BoxShadow, Context, Date, Div, Entity,
    FluentBuilder, FocusHandle, Focusable, FontWeight, IconAsset, Image, InteractiveElement,
    IntoElement, KeyDownEvent, ListAlignment, ListScrollEvent, ListSizingBehavior, ListState,
    MouseButton, MouseDownEvent, ParentElement, Render, ScrollHandle, SlackConnectionApi,
    SlackMessageClientId, SlackProfilePanelState, SlackRemoteImageCache,
    SlackWorkspaceApiCapabilities, SlackWorkspaceConnection, StatefulInteractiveElement, Styled,
    SurfaceTheme, Window, WorkspaceApi, SLACK_LINE_HEIGHT, SLACK_MESSAGE_GAP,
    SLACK_PROFILE_PANEL_WIDTH, SLACK_TABS_HEIGHT,
};

mod activity_types;
mod all_threads_types;
mod bookmark_folder_types;
mod channel;
mod channel_details_types;
mod channel_menu_types;
mod composer_attachments;
mod composer_document;
mod constants;
mod conversation_files_types;
mod conversation_tabs_types;
mod drafts_sent_types;
mod files_types;
mod helpers;
mod icons;
mod later_types;
mod members_types;
mod message;
mod message_edit_view;
mod message_forward_types;
mod message_mutation_types;
mod new_message_types;
mod notification_types;
mod palette;
mod pins_types;
mod profile;
mod root;
mod self_settings_types;
mod sidebar;
mod sidebar_section_types;
mod state;
mod surface_event_source;
mod surface_root_actions;
mod surface_root_contract;
mod surface_root_core;
mod surface_root_queries;
mod surface_root_render;
mod surface_state_core;
mod thread_broadcast;
mod types;
mod video_clip_modal;
mod view;
mod workspace_host;
mod workspace_rail;

pub(super) use crate::ui::{
    SlackFilesBrowserSessionId, SlackFilesRequest, SlackFilesScope, SlackFilesSort,
    SlackFilesTypeFilter,
};
pub(crate) use activity_types::*;
pub(crate) use all_threads_types::*;
pub(crate) use bookmark_folder_types::*;
pub(crate) use channel_details_types::*;
pub(crate) use channel_menu_types::*;
pub(crate) use composer_attachments::*;
pub(crate) use composer_document::{
    SlackComposerDocument, SlackComposerLinkEdit, SlackComposerLinkUrl,
};
pub(crate) use constants::*;
pub(crate) use conversation_files_types::*;
pub(crate) use conversation_tabs_types::*;
pub(crate) use drafts_sent_types::*;
pub(crate) use files_types::*;
#[cfg(test)]
pub(crate) use helpers::slack_recording_duration_millis;
pub(crate) use helpers::{
    slack_attachment_date_label, slack_base_icon_radius, slack_date_label, SlackSurfaceAction,
    SlackSurfaceActionButton,
};
pub(crate) use icons::*;
pub(crate) use later_types::*;
pub(crate) use members_types::*;
#[cfg(test)]
pub(crate) use message::build_slack_message_rows;
pub(crate) use message::{
    build_slack_appended_message_row_with_local_today,
    build_slack_conversation_message_rows_with_local_today, build_slack_local_delivery_row,
    build_slack_message_chunks, build_slack_message_rows_with_local_today,
    build_slack_pinned_message_row, build_slack_thread_page_reply_rows_in_timezone,
    build_slack_thread_parent_row, build_slack_thread_parent_row_in_timezone,
    prepare_slack_message_body, prepare_slack_message_body_from_message,
    slack_all_threads_timestamp_label, slack_attachment_row_with_identity, slack_local_today,
    slack_message_timezone, slack_reaction_rows, SlackAppendedMessageRowInput,
    SlackInlineVideoFrame, SlackLocalDeliveryRowInput, SlackMessageDocumentPosition,
    SlackReactionBarInput,
};
pub(in crate::ui::surface) use message::{
    slack_message_body_block_in_document, SlackMessageSelectionContext,
};
pub(crate) use message_edit_view::*;
pub(crate) use message_forward_types::*;
pub(crate) use message_mutation_types::*;
pub(crate) use new_message_types::*;
pub(crate) use notification_types::*;
pub(crate) use palette::{
    slack_activity_palette, slack_palette, SlackActivityPalette, SlackPalette,
};
pub(crate) use pins_types::*;
pub(crate) use self_settings_types::*;
pub(crate) use sidebar::{build_slack_sidebar_rows, build_slack_sidebar_snapshot_rows};
pub(crate) use sidebar_section_types::*;
pub(crate) use state::{
    load_slack_prepared_upload_files_from_paths, load_slack_upload_files_from_paths,
    SlackDateJumpCalendarCell, SlackDateJumpCalendarCellStatus, SlackDateJumpMenuAction,
    SlackDateJumpMenuState, SlackDateJumpOverlay, SlackDateJumpPickerState, SlackDateJumpRequest,
    SlackMessageNavigationHighlight, SlackMessageNavigationRequest, SlackScheduleAnchor,
    SlackScheduleCalendarCell, SlackScheduleCalendarCellStatus, SlackScheduleCustomState,
    SlackScheduleDatePickerState, SlackScheduleMenuState, SlackScheduleNestedPicker,
    SlackScheduleOverlay, SlackScheduleOverlayState, SlackScheduleTimeOption,
    SlackScheduleTimePickerState,
};
pub use surface_event_source::ChatEventSource;
pub(crate) use surface_state_core::SurfaceStateConfig;
pub(crate) use thread_broadcast::slack_thread_broadcast_label;
pub(crate) use types::*;
pub use types::{
    SlackAttachmentPathSelection, SlackAuxPanelQueryBehavior, SlackAuxPanelRow,
    SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState, SlackComposerFormatAction,
    SlackMainComposerDraftHandle, SlackPreparedUploadFile, SlackThreadDraftHandle,
};
pub(crate) use video_clip_modal::*;
pub(crate) use view::*;
pub use view::{SlackFilesFilter, SlackMainTab, SlackRailView};
#[cfg(test)]
pub use view::{SlackMediaState, SlackPlaybackSpeed};

#[derive(Clone, Default)]
pub struct SurfaceInput {
    pub workspace: Option<crate::ui::SlackWorkspace>,
    pub workspace_api: Option<Arc<dyn WorkspaceApi>>,
    pub local_file_api: Option<Arc<dyn crate::model::SlackLocalFileApi>>,
    pub embedded_shell: bool,
    pub initial_thread_message_id: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) enum ChatStartup {
    #[default]
    Archive,
    #[cfg(any(test, feature = "test-support"))]
    Fixture,
    ConnectionRequired {
        connection_api: Arc<dyn SlackConnectionApi>,
        request: crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
    },
    Loading {
        connection_api: Arc<dyn SlackConnectionApi>,
        request: crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
        request_started: bool,
        previous_connection: Option<SlackWorkspaceConnection>,
    },
    Ready {
        connection_api: Arc<dyn SlackConnectionApi>,
        request: crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
        connection: SlackWorkspaceConnection,
    },
    Error {
        connection_api: Arc<dyn SlackConnectionApi>,
        request: crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
        previous_connection: Option<SlackWorkspaceConnection>,
        error: String,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum SlackCachedSurfaceRegion {
    History,
    Rail,
    Sidebar,
    Conversation,
}

pub(crate) struct SlackCachedSurfaceView {
    surface: gpui::WeakEntity<SurfaceState>,
    region: SlackCachedSurfaceRegion,
}

pub(crate) struct SlackSearchOverlayView {
    surface: gpui::WeakEntity<SurfaceState>,
}

pub(crate) struct SlackSearchResultsView {
    surface: gpui::WeakEntity<SurfaceState>,
}

#[derive(Clone, Copy)]
pub(crate) struct SlackSurfaceActivationTiming {
    activated_at: Instant,
    surface_construction_started_at: Instant,
    surface_ready_at: Instant,
    surface_created: bool,
}

pub struct SurfaceRoot {
    input: SurfaceInput,
    startup: ChatStartup,
    media_capture_api: Option<Arc<crate::ui::MediaCaptureApi>>,
    input_dirty: bool,
    theme: SurfaceTheme,
    active: bool,
    preview_width: f32,
    viewport_height: f32,
    slack_sidebar_preferred_ratio: f32,
    legacy_slack_sidebar_width: Option<f32>,
    slack_workspace_switcher_expanded: bool,
    surface: Option<Entity<SurfaceState>>,
    workspace_host: Option<workspace_host::SlackWorkspaceHost>,
    workspace_rail: Option<Entity<workspace_rail::SlackWorkspaceRail>>,
    event_source: Option<Entity<ChatEventSource>>,
    activation_started_at: Option<Instant>,
}

#[cfg(any(test, feature = "test-support"))]
impl Default for SurfaceRoot {
    fn default() -> Self {
        Self::from_input(SurfaceInput::default())
    }
}

pub struct SurfaceState {
    slack_spinner_frame_cache: SlackSpinnerFrameCache,
    focus_handle: FocusHandle,
    slack_workspace_switcher_focus_handle: FocusHandle,
    slack_sidebar_resize_focus_handle: FocusHandle,
    slack_header_move_focus_handle: FocusHandle,
    slack_header_details_focus_handle: FocusHandle,
    slack_header_members_focus_handle: FocusHandle,
    slack_header_notifications_focus_handle: FocusHandle,
    slack_header_search_focus_handle: FocusHandle,
    slack_header_more_focus_handle: FocusHandle,
    slack_header_dms_close_focus_handle: FocusHandle,
    slack_channel_move_menu_focus_handle: FocusHandle,
    slack_channel_notifications_menu_focus_handle: FocusHandle,
    slack_channel_menu_focus_handle: FocusHandle,
    slack_channel_submenu_focus_handle: FocusHandle,
    slack_message_menu_focus_handle: FocusHandle,
    slack_reaction_picker_focus_handle: FocusHandle,
    slack_skin_tone_menu_focus_handle: FocusHandle,
    slack_skin_tone_toggle_focus_handle: FocusHandle,
    slack_skin_tone_menu_focus_observer_registered: bool,
    slack_reaction_picker_search_input: Entity<TextInput>,
    slack_reaction_picker_search_accessibility_id: ElementId,
    slack_conversation_tab_focus_handles: HashMap<gpui::SharedString, FocusHandle>,
    slack_conversation_tabs_more_focus_handle: FocusHandle,
    slack_conversation_tabs_overflow_focus_handle: FocusHandle,
    slack_search_input: Entity<TextInput>,
    slack_search_accessibility_id: ElementId,
    slack_files_search_input: Entity<TextInput>,
    slack_files_search_accessibility_id: ElementId,
    slack_conversation_files_search_input: Entity<TextInput>,
    slack_conversation_files_search_accessibility_id: ElementId,
    slack_members_search_input: Entity<TextInput>,
    slack_members_search_accessibility_id: ElementId,
    slack_directory_search_input: Entity<TextInput>,
    slack_directory_search_accessibility_id: ElementId,
    slack_home_finder_input: Entity<TextInput>,
    slack_home_finder_accessibility_id: ElementId,
    slack_home_finder_focus_observers_registered: bool,
    slack_dm_finder_input: Entity<TextInput>,
    slack_dm_finder_accessibility_id: ElementId,
    slack_dm_finder_focus_observers_registered: bool,
    slack_new_message_to_input: Entity<TextInput>,
    slack_new_message_to_accessibility_id: ElementId,
    slack_new_message_to_focus_observers_registered: bool,
    slack_message_forward_destination_input: Entity<TextInput>,
    slack_message_forward_destination_accessibility_id: ElementId,
    slack_message_forward_note_input: Entity<TextInput>,
    slack_message_forward_note_accessibility_id: ElementId,
    slack_self_status_text_input: Entity<TextInput>,
    slack_self_status_text_accessibility_id: ElementId,
    slack_self_status_emoji_input: Entity<TextInput>,
    slack_self_status_emoji_accessibility_id: ElementId,
    slack_sidebar_section_name_input: Entity<TextInput>,
    slack_sidebar_section_name_accessibility_id: ElementId,
    slack_later_reminder_input: Entity<TextInput>,
    slack_later_reminder_accessibility_id: ElementId,
    slack_later_reminder_layer_focus_handle: FocusHandle,
    slack_later_reminder_close_focus_handle: FocusHandle,
    slack_later_reminder_save_focus_handle: FocusHandle,
    slack_later_reminder_focus_guard_start: FocusHandle,
    slack_later_reminder_focus_guard_end: FocusHandle,
    slack_later_reminder_focus_observers_registered: bool,
    slack_message_edit: Option<Entity<SlackMessageEditView>>,
    slack_composer_input: Entity<TextInput>,
    slack_composer_accessibility_id: ElementId,
    slack_composer_blur_observer_registered: bool,
    slack_composer_format_focus_handles: [FocusHandle; 10],
    slack_thread_composer_input: Entity<TextInput>,
    slack_thread_composer_accessibility_id: ElementId,
    slack_thread_composer_blur_observer_registered: bool,
    slack_thread_format_focus_handles: [FocusHandle; 10],
    slack_link_text_input: Entity<TextInput>,
    slack_link_text_accessibility_id: ElementId,
    slack_link_url_input: Entity<TextInput>,
    slack_link_url_accessibility_id: ElementId,
    slack_all_threads_composers: HashMap<gpui::SharedString, SlackAllThreadsComposerState>,
    slack_schedule_date_input: Entity<TextInput>,
    slack_schedule_date_accessibility_id: ElementId,
    slack_schedule_time_input: Entity<TextInput>,
    slack_schedule_time_accessibility_id: ElementId,
    slack_schedule_input_blur_observers_registered: bool,
    slack_cached_history: Entity<SlackCachedSurfaceView>,
    slack_cached_rail: Entity<SlackCachedSurfaceView>,
    slack_cached_sidebar: Entity<SlackCachedSurfaceView>,
    slack_cached_conversation: Entity<SlackCachedSurfaceView>,
    slack_search_overlay: Entity<SlackSearchOverlayView>,
    slack_search_results: Entity<SlackSearchResultsView>,
    slack_presence_authority: state::SlackPresenceAuthority,
    slack_realtime_team_id: Option<String>,
    slack_realtime_task: Option<gpui::Task<()>>,
    slack_realtime_sidebar_refresh_lifecycle_generation: u64,
    slack_realtime_sidebar_refresh_requested_generation: u64,
    slack_realtime_sidebar_refresh_completed_generation: u64,
    slack_realtime_sidebar_refresh_in_flight: Option<(u64, u64)>,
    slack_realtime_sidebar_refresh_retry_timer: Option<(u64, u64)>,
    slack_realtime_sidebar_refresh_failures: u8,
    slack_realtime_conversation_refresh_pending: bool,
    slack_realtime_thread_refresh_pending: bool,
    slack_realtime_activity_stale: bool,
    slack_realtime_later_stale: bool,
    slack_realtime_files_stale: bool,
    slack_realtime_all_threads_stale: bool,
    data: SurfaceStateData,
}

pub use state::SurfaceStateData;

#[cfg(test)]
mod tests;
