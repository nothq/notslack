use std::collections::HashMap;

use gpui::{AppContext, Context, Entity};

use super::{
    ChatStartup, SlackCachedSurfaceRegion, SlackCachedSurfaceView, SlackSearchOverlayView,
    SlackSearchResultsView, SurfaceInput, SurfaceState, SurfaceStateConfig, SurfaceStateData,
};

mod inputs;

use inputs::SlackSurfaceInputs;

struct SlackSurfaceViews {
    cached_history: Entity<SlackCachedSurfaceView>,
    cached_rail: Entity<SlackCachedSurfaceView>,
    cached_sidebar: Entity<SlackCachedSurfaceView>,
    cached_conversation: Entity<SlackCachedSurfaceView>,
    search_overlay: Entity<SlackSearchOverlayView>,
    search_results: Entity<SlackSearchResultsView>,
}

impl SlackSurfaceViews {
    fn new(cx: &mut Context<SurfaceState>) -> Self {
        let surface = cx.entity();
        let cached_history = cx.new({
            let surface = surface.clone();
            move |cx| SlackCachedSurfaceView::new(surface, SlackCachedSurfaceRegion::History, cx)
        });
        let cached_rail = cx.new({
            let surface = surface.clone();
            move |cx| SlackCachedSurfaceView::new(surface, SlackCachedSurfaceRegion::Rail, cx)
        });
        let cached_sidebar = cx.new({
            let surface = surface.clone();
            move |cx| SlackCachedSurfaceView::new(surface, SlackCachedSurfaceRegion::Sidebar, cx)
        });
        let search_overlay = cx.new({
            let surface = surface.clone();
            move |cx| SlackSearchOverlayView::new(surface, cx)
        });
        let search_results = cx.new({
            let surface = surface.clone();
            move |cx| SlackSearchResultsView::new(surface, cx)
        });
        let cached_conversation = cx.new(move |cx| {
            SlackCachedSurfaceView::new(surface, SlackCachedSurfaceRegion::Conversation, cx)
        });
        Self {
            cached_history,
            cached_rail,
            cached_sidebar,
            cached_conversation,
            search_overlay,
            search_results,
        }
    }
}

macro_rules! build_surface_state {
    ($inputs:ident, $views:ident, $input:ident, $startup:ident, $config:ident, $cx:ident) => {
        SurfaceState {
            slack_spinner_frame_cache: std::cell::RefCell::new(HashMap::new()),
            focus_handle: $cx.focus_handle(),
            slack_workspace_switcher_focus_handle: $cx.focus_handle(),
            slack_sidebar_resize_focus_handle: $cx.focus_handle(),
            slack_header_move_focus_handle: $cx.focus_handle(),
            slack_header_details_focus_handle: $cx.focus_handle(),
            slack_header_members_focus_handle: $cx.focus_handle(),
            slack_header_notifications_focus_handle: $cx.focus_handle(),
            slack_header_search_focus_handle: $cx.focus_handle(),
            slack_header_more_focus_handle: $cx.focus_handle(),
            slack_header_dms_close_focus_handle: $cx.focus_handle(),
            slack_channel_move_menu_focus_handle: $cx.focus_handle(),
            slack_channel_notifications_menu_focus_handle: $cx.focus_handle(),
            slack_channel_menu_focus_handle: $cx.focus_handle(),
            slack_channel_submenu_focus_handle: $cx.focus_handle(),
            slack_message_menu_focus_handle: $cx.focus_handle(),
            slack_reaction_picker_focus_handle: $cx.focus_handle(),
            slack_skin_tone_menu_focus_handle: $cx.focus_handle(),
            slack_skin_tone_toggle_focus_handle: $cx.focus_handle(),
            slack_skin_tone_menu_focus_observer_registered: false,
            slack_reaction_picker_search_input: $inputs.reaction_picker_search.entity,
            slack_reaction_picker_search_accessibility_id: $inputs
                .reaction_picker_search
                .accessibility_id,
            slack_conversation_tab_focus_handles: HashMap::new(),
            slack_conversation_tabs_more_focus_handle: $cx.focus_handle(),
            slack_conversation_tabs_overflow_focus_handle: $cx.focus_handle(),
            slack_search_input: $inputs.search.entity,
            slack_search_accessibility_id: $inputs.search.accessibility_id,
            slack_files_search_input: $inputs.files_search.entity,
            slack_files_search_accessibility_id: $inputs.files_search.accessibility_id,
            slack_conversation_files_search_input: $inputs.conversation_files_search.entity,
            slack_conversation_files_search_accessibility_id: $inputs
                .conversation_files_search
                .accessibility_id,
            slack_members_search_input: $inputs.members_search.entity,
            slack_members_search_accessibility_id: $inputs.members_search.accessibility_id,
            slack_directory_search_input: $inputs.directory_search.entity,
            slack_directory_search_accessibility_id: $inputs.directory_search.accessibility_id,
            slack_home_finder_input: $inputs.home_finder.entity,
            slack_home_finder_accessibility_id: $inputs.home_finder.accessibility_id,
            slack_home_finder_focus_observers_registered: false,
            slack_dm_finder_input: $inputs.dm_finder.entity,
            slack_dm_finder_accessibility_id: $inputs.dm_finder.accessibility_id,
            slack_dm_finder_focus_observers_registered: false,
            slack_new_message_to_input: $inputs.new_message_to.entity,
            slack_new_message_to_accessibility_id: $inputs.new_message_to.accessibility_id,
            slack_new_message_to_focus_observers_registered: false,
            slack_message_forward_destination_input: $inputs.forward_destination.entity,
            slack_message_forward_destination_accessibility_id: $inputs
                .forward_destination
                .accessibility_id,
            slack_message_forward_note_input: $inputs.forward_note.entity,
            slack_message_forward_note_accessibility_id: $inputs.forward_note.accessibility_id,
            slack_self_status_text_input: $inputs.self_status_text.entity,
            slack_self_status_text_accessibility_id: $inputs.self_status_text.accessibility_id,
            slack_self_status_emoji_input: $inputs.self_status_emoji.entity,
            slack_self_status_emoji_accessibility_id: $inputs.self_status_emoji.accessibility_id,
            slack_sidebar_section_name_input: $inputs.sidebar_section_name.entity,
            slack_sidebar_section_name_accessibility_id: $inputs
                .sidebar_section_name
                .accessibility_id,
            slack_later_reminder_input: $inputs.later_reminder.entity,
            slack_later_reminder_accessibility_id: $inputs.later_reminder.accessibility_id,
            slack_later_reminder_layer_focus_handle: $cx.focus_handle(),
            slack_later_reminder_close_focus_handle: $cx.focus_handle(),
            slack_later_reminder_save_focus_handle: $cx.focus_handle(),
            slack_later_reminder_focus_guard_start: $cx.focus_handle(),
            slack_later_reminder_focus_guard_end: $cx.focus_handle(),
            slack_later_reminder_focus_observers_registered: false,
            slack_message_edit: None,
            slack_composer_input: $inputs.composer.entity,
            slack_composer_accessibility_id: $inputs.composer.accessibility_id,
            slack_composer_blur_observer_registered: false,
            slack_composer_format_focus_handles: std::array::from_fn(|_| $cx.focus_handle()),
            slack_thread_composer_input: $inputs.thread_composer.entity,
            slack_thread_composer_accessibility_id: $inputs.thread_composer.accessibility_id,
            slack_thread_composer_blur_observer_registered: false,
            slack_thread_format_focus_handles: std::array::from_fn(|_| $cx.focus_handle()),
            slack_link_text_input: $inputs.link_text.entity,
            slack_link_text_accessibility_id: $inputs.link_text.accessibility_id,
            slack_link_url_input: $inputs.link_url.entity,
            slack_link_url_accessibility_id: $inputs.link_url.accessibility_id,
            slack_all_threads_composers: HashMap::new(),
            slack_schedule_date_input: $inputs.schedule_date.entity,
            slack_schedule_date_accessibility_id: $inputs.schedule_date.accessibility_id,
            slack_schedule_time_input: $inputs.schedule_time.entity,
            slack_schedule_time_accessibility_id: $inputs.schedule_time.accessibility_id,
            slack_schedule_input_blur_observers_registered: false,
            slack_cached_history: $views.cached_history,
            slack_cached_rail: $views.cached_rail,
            slack_cached_sidebar: $views.cached_sidebar,
            slack_cached_conversation: $views.cached_conversation,
            slack_search_overlay: $views.search_overlay,
            slack_search_results: $views.search_results,
            slack_presence_authority: Default::default(),
            slack_realtime_team_id: None,
            slack_realtime_task: None,
            slack_realtime_sidebar_refresh_lifecycle_generation: 0,
            slack_realtime_sidebar_refresh_requested_generation: 0,
            slack_realtime_sidebar_refresh_completed_generation: 0,
            slack_realtime_sidebar_refresh_in_flight: None,
            slack_realtime_sidebar_refresh_retry_timer: None,
            slack_realtime_sidebar_refresh_failures: 0,
            slack_realtime_conversation_refresh_pending: false,
            slack_realtime_thread_refresh_pending: false,
            slack_realtime_activity_stale: false,
            slack_realtime_later_stale: false,
            slack_realtime_files_stale: false,
            slack_realtime_all_threads_stale: false,
            data: SurfaceStateData::new($input, $startup, $config),
        }
    };
}

impl SurfaceState {
    pub(crate) fn new(
        input: SurfaceInput,
        startup: ChatStartup,
        config: SurfaceStateConfig,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_thread_message_id = input.initial_thread_message_id.clone();
        let inputs = SlackSurfaceInputs::new(cx);
        let views = SlackSurfaceViews::new(cx);
        let mut state = build_surface_state!(inputs, views, input, startup, config, cx);
        {
            let (presence, data) = (&mut state.slack_presence_authority, &mut state.data);
            presence.initialize(data);
        }
        state.initialize_slack_main_composer_context(cx);
        state.initialize_slack_home_finder_input(cx);
        state.initialize_slack_directory_input(cx);
        state.initialize_slack_new_message_input(cx);
        state.initialize_slack_inline_mention_observers(cx);
        state.register_slack_list_scroll_handlers(cx);
        state.sync_slack_members_context(cx);
        state.sync_slack_channel_notification_preference(cx);
        state.sync_slack_preferred_skin_tone(cx);
        state.sync_slack_remote_draft_hydration(cx);
        state.ensure_slack_realtime_subscription(cx);
        if let Some(parent_message_id) = initial_thread_message_id {
            state.open_slack_thread_panel(&parent_message_id, cx);
        }
        state
    }
}
