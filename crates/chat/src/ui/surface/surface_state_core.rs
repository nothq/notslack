use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use gpui::ListOffset;

use super::{
    build_slack_message_chunks, build_slack_message_rows_with_local_today,
    build_slack_remote_images, build_slack_sidebar_rows, div, px, rgb,
    spawn_background_task_for_entity, spawn_timer_task_for_entity, AnyElement, App, AppearanceMode,
    ChatStartup, Context, FocusHandle, Focusable, Image, IntoElement, KeyDownEvent, ListAlignment,
    ListScrollEvent, ListState, ParentElement, Render, SlackCachedSurfaceRegion,
    SlackCachedSurfaceView, SlackComposerFormatAction, SlackConversationLoadRoute, SlackDmRow,
    SlackFilesFilter, SlackFilesSort, SlackFilesTypeFilter, SlackHeaderControl,
    SlackHistoryDestination, SlackHistoryEntry, SlackMainRoute, SlackMainTab, SlackMessageRow,
    SlackRailView, SlackSearchOverlayView, SlackSearchResultsView, SlackShellIcon,
    SlackSidebarRevealState, SlackWorkspaceApiCapabilities, Styled, SurfaceInput, SurfaceState,
    SurfaceStateData, SurfaceTheme, Window, SLACK_MESSAGE_LIST_OVERDRAW,
    SLACK_SIDEBAR_LIST_OVERDRAW,
};

mod initialization;
mod input;
mod listeners;
mod render;
mod runtime;

#[derive(Clone, Copy)]
pub(crate) struct SurfaceStateConfig {
    pub(crate) theme: SurfaceTheme,
    pub(crate) appearance_mode: AppearanceMode,
    pub(crate) active: bool,
    pub(crate) preview_width: f32,
    pub(crate) viewport_height: f32,
}

struct SlackSurfaceSeed {
    sidebar_rows: Arc<[crate::ui::surface::SlackSidebarRow]>,
    dm_rows: Arc<[SlackDmRow]>,
    message_chunks: Arc<[crate::ui::surface::SlackMessageChunk]>,
    message_rows: Arc<[crate::ui::surface::SlackMessageRow]>,
    message_rows_local_today: Option<crate::ui::Date>,
    collapsed_sections: HashSet<String>,
    remote_images: HashMap<String, Arc<Image>>,
    conversation_history: Vec<SlackHistoryEntry>,
}

impl SlackSurfaceSeed {
    fn from_workspace(workspace: Option<&crate::ui::SlackWorkspace>) -> Self {
        let collapsed_sections = HashSet::new();
        let (message_rows, message_rows_local_today) =
            build_slack_message_rows_with_local_today(workspace);
        let message_chunks = build_slack_message_chunks(&message_rows);
        let sidebar_rows = build_slack_sidebar_rows(workspace, &collapsed_sections);
        let dm_rows = Arc::default();
        let remote_images = build_slack_remote_images(workspace);
        let conversation_history = workspace
            .and_then(slack_initial_history_entry)
            .map(|entry| vec![entry])
            .unwrap_or_default();
        Self {
            sidebar_rows,
            dm_rows,
            message_chunks,
            message_rows,
            message_rows_local_today,
            collapsed_sections,
            remote_images,
            conversation_history,
        }
    }

    fn sidebar_list_state(&self) -> ListState {
        SurfaceState::build_slack_sidebar_list_state(self.sidebar_rows.len())
    }

    fn message_list_state(&self) -> ListState {
        SurfaceState::build_slack_message_list_state(&self.message_rows)
    }
}

impl SurfaceStateData {
    fn new(input: SurfaceInput, startup: ChatStartup, config: SurfaceStateConfig) -> Self {
        let seed = SlackSurfaceSeed::from_workspace(input.workspace.as_ref());
        let mut data = Self::default();
        data.slack_sidebar_preferred_ratio = crate::ui::SLACK_SIDEBAR_DEFAULT_RATIO;
        data.apply_config(config);
        data.apply_input(input, startup, &seed);
        data.initialize_list_states(&seed);
        data.apply_seed(seed);
        data.initialize_navigation_defaults();
        data
    }

    fn apply_config(&mut self, config: SurfaceStateConfig) {
        self.theme = config.theme;
        self.active = config.active;
        self.appearance_mode = config.appearance_mode;
        self.preview_width = config.preview_width;
        self.viewport_height = config.viewport_height;
    }

    fn apply_input(&mut self, input: SurfaceInput, startup: ChatStartup, seed: &SlackSurfaceSeed) {
        let workspace = input.workspace;
        self.embedded_shell = input.embedded_shell;
        self.slack_shell_snapshot = workspace
            .as_ref()
            .map(crate::ui::SlackWorkspace::shell_snapshot);
        self.slack_sidebar_snapshot = workspace
            .as_ref()
            .map(crate::ui::SlackWorkspace::sidebar_snapshot);
        self.slack_conversation_snapshot = workspace
            .as_ref()
            .map(crate::ui::SlackWorkspace::conversation_snapshot);
        self.slack_sidebar_reveal = workspace.as_ref().and_then(|workspace| {
            (!workspace.conversation_id.is_empty())
                .then(|| SlackSidebarRevealState::pending(workspace.conversation_id.clone(), true))
        });
        self.slack_remote_image_queue_dirty = workspace.is_some();
        self.slack_workspace = workspace.map(Arc::new);
        self.slack_workspace_api_capabilities = input
            .workspace_api
            .as_ref()
            .map_or(SlackWorkspaceApiCapabilities::NONE, |api| {
                api.capabilities()
            });
        self.workspace_api = input.workspace_api;
        self.local_file_api = input.local_file_api;
        self.chat_startup = startup;
        self.slack_conversation_history_index = seed.conversation_history.len().saturating_sub(1);
    }

    fn initialize_list_states(&mut self, seed: &SlackSurfaceSeed) {
        self.slack_sidebar_list_state = seed.sidebar_list_state().into();
        self.slack_home_finder_list_state = SurfaceState::build_slack_sidebar_list_state(0).into();
        self.slack_dm_list_state = SurfaceState::build_slack_sidebar_list_state(0).into();
        self.slack_all_threads_list_state = empty_slack_message_list_state().into();
        self.slack_activity_list_state = empty_slack_message_list_state().into();
        self.slack_activity_detail_list_state = empty_slack_message_list_state().into();
        self.slack_later_list_state = empty_slack_message_list_state().into();
        self.slack_conversation_files_list_state = empty_slack_message_list_state().into();
        self.slack_drafts_sent_list_state = empty_slack_message_list_state().into();
        self.slack_pins_list_state = empty_slack_message_list_state().into();
        self.slack_search_list_state = ListState::new(0, ListAlignment::Top, px(72.0)).into();
        self.slack_message_list_state = seed.message_list_state().into();
    }

    fn apply_seed(&mut self, seed: SlackSurfaceSeed) {
        self.slack_sidebar_rows = seed.sidebar_rows;
        self.slack_dm_rows = seed.dm_rows;
        self.slack_message_chunks = seed.message_chunks;
        self.slack_message_rows = seed.message_rows;
        self.slack_message_rows_local_today = seed.message_rows_local_today;
        self.slack_collapsed_sections = seed.collapsed_sections;
        self.slack_remote_images = seed.remote_images.into();
        self.slack_conversation_history = seed.conversation_history;
    }

    fn initialize_navigation_defaults(&mut self) {
        self.slack_all_threads_visible_range = (0, 4);
        self.slack_files_type_filters = SlackFilesTypeFilter::DEFAULT.to_vec();
        self.slack_files_sort = SlackFilesSort::RecentlyViewed;
        self.slack_main_route = SlackMainRoute::Conversation;
        self.slack_message_list_auto_position_active = true;
        self.slack_composer_format_roving_target = SlackComposerFormatAction::Bold;
        self.slack_pending_conversation_route = SlackConversationLoadRoute::Conversation;
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::Messages;
        self.slack_conversation_tab_roving_key = super::SLACK_CONVERSATION_TAB_MESSAGES_KEY.into();
        self.slack_files_filter = SlackFilesFilter::All;
        self.slack_header_roving_target = SlackHeaderControl::ChannelMove;
    }
}

fn empty_slack_message_list_state() -> ListState {
    ListState::new(0, ListAlignment::Top, px(SLACK_MESSAGE_LIST_OVERDRAW))
}

fn slack_initial_history_entry(workspace: &crate::ui::SlackWorkspace) -> Option<SlackHistoryEntry> {
    let conversation_id = (!workspace.conversation_id.is_empty())
        .then(|| gpui::SharedString::from(workspace.conversation_id.clone()))?;
    let label = gpui::SharedString::from(workspace.channel_name.clone());
    let avatar_image_url = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.target_id == workspace.conversation_id)
        .and_then(|item| item.avatar_image_url.clone())
        .map(gpui::SharedString::from);
    let (icon, accessibility_label) = match workspace.channel_kind {
        crate::ui::SlackConversationKind::Channel => (
            SlackShellIcon::HashSmall,
            gpui::SharedString::from(format!("#{}", workspace.channel_name)),
        ),
        crate::ui::SlackConversationKind::PrivateChannel => (
            SlackShellIcon::LockSmall,
            gpui::SharedString::from(format!("#{} (Private)", workspace.channel_name)),
        ),
        crate::ui::SlackConversationKind::DirectMessage
        | crate::ui::SlackConversationKind::GroupMessage
        | crate::ui::SlackConversationKind::Unknown => (SlackShellIcon::Dm, label.clone()),
    };
    Some(SlackHistoryEntry {
        destination: SlackHistoryDestination::Conversation {
            conversation_id,
            rail_view: SlackRailView::Home,
            tab: SlackMainTab::Messages,
            source_tab: None,
        },
        label,
        accessibility_label,
        icon,
        avatar_image_url,
    })
}

impl Deref for SurfaceState {
    type Target = SurfaceStateData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SurfaceState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

fn slack_workspace_changed(
    next: &crate::ui::SlackWorkspace,
    current: &crate::ui::SlackWorkspace,
) -> bool {
    next.team_id != current.team_id
        || next.conversation_id != current.conversation_id
        || next.messages.len() != current.messages.len()
        || next.rail_badges != current.rail_badges
        || slack_sidebar_signature(next) != slack_sidebar_signature(current)
}

type SlackSidebarItemSignature<'a> = (&'a str, bool, bool, Option<u32>);
type SlackSidebarSectionSignature<'a> = (&'a str, Vec<SlackSidebarItemSignature<'a>>);
type SlackSidebarSignature<'a> = Vec<SlackSidebarSectionSignature<'a>>;

fn slack_sidebar_signature(workspace: &crate::ui::SlackWorkspace) -> SlackSidebarSignature<'_> {
    workspace
        .sections
        .iter()
        .map(|section| {
            (
                section.label.as_str(),
                section
                    .items
                    .iter()
                    .map(|item| {
                        (
                            item.target_id.as_str(),
                            item.active,
                            item.unread,
                            item.count,
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
