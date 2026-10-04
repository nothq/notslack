mod paging;
mod selection;

use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{ScrollStrategy, SharedString};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::super::{
    alpha, prepare_slack_files_snapshot, px, rgb, spawn_background_task_for_entity, Context,
    Entity, PreparedSlackFilesSnapshot, SlackFilesBrowserSessionId, SlackFilesMenu,
    SlackFilesRequest, SlackFilesScope, SlackFilesSidebarSelection, SlackFilesSort,
    SlackFilesTypeFilter, SlackRailView, SurfaceState, WorkspaceApi,
};

const SLACK_FILES_PAGINATION_THRESHOLD: usize = 4;

#[derive(Clone)]
struct SlackFilesPageRequest {
    generation: u64,
    request: SlackFilesRequest,
    timezone: chrono_tz::Tz,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn refresh_slack_files_from_realtime_if_visible(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_realtime_files_stale
            || self.slack_active_rail_view != SlackRailView::Files
            || self.slack_files_loading
        {
            return;
        }
        self.slack_realtime_files_stale = false;
        self.slack_files_generation = next_slack_files_generation(self.slack_files_generation);
        self.slack_files_snapshot = None;
        self.slack_files_error = None;
        self.begin_slack_files_page(1, cx);
    }

    pub(crate) fn slack_files_search_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let props = TextInputProps::single_line(self.slack_files_search_query.clone())
            .placeholder("Search files")
            .style(TextInputStyle {
                height: px(38.0),
                min_height: px(38.0),
                padding_x: px(0.0),
                padding_y: px(0.0),
                radius: px(0.0),
                background: alpha(0x000000, 0.0),
                border: alpha(0x000000, 0.0),
                focused_border: alpha(0x000000, 0.0),
                text: rgb(0xf8f8f8).into(),
                placeholder: rgb(0x9a9b9e).into(),
                selection: alpha(0x1264a3, 0.45),
                caret: rgb(0xf8f8f8).into(),
                font_size: px(16.0),
                line_height: px(22.0),
                font_family: Some("Lato".into()),
            })
            .bordered(false)
            .accessibility(
                self.slack_files_search_accessibility_id.clone(),
                "Search Slack files",
            )
            .on_change(self.slack_files_search_on_change(cx))
            .on_submit(self.slack_files_search_on_submit(cx))
            .on_escape(self.slack_files_search_on_escape(cx));
        self.slack_files_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_files_search_input.clone()
    }

    fn slack_files_search_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                surface.slack_files_search_query = value;
                cx.notify();
            });
        })
    }

    fn slack_files_search_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.submit_slack_files_search(cx);
            });
        })
    }

    fn slack_files_search_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |window, cx| {
            surface.update(cx, |surface, cx| {
                surface.slack_files_menu = None;
                cx.focus_self(window);
                cx.notify();
            });
        })
    }

    pub(crate) fn activate_slack_files(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_files {
            return;
        }
        let Some((team_id, self_user_id)) = self.slack_workspace().and_then(|workspace| {
            workspace
                .self_user_id
                .as_ref()
                .map(|self_user_id| (workspace.team_id.clone(), self_user_id.clone()))
        }) else {
            self.slack_files_error =
                Some("Slack Files requires the current workspace user identity.".to_string());
            cx.notify();
            return;
        };
        if self.slack_files_team_id.as_deref() != Some(team_id.as_str())
            || self.slack_files_self_user_id.as_deref() != Some(self_user_id.as_str())
        {
            self.reset_slack_files_context();
            self.slack_files_team_id = Some(team_id);
            self.slack_files_self_user_id = Some(self_user_id);
            self.slack_files_browser_session_id = Some(
                SlackFilesBrowserSessionId::new(uuid::Uuid::new_v4().to_string())
                    .expect("UUID Slack Files browser session id must be valid"),
            );
        }
        self.slack_active_rail_view = SlackRailView::Files;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.slack_files_menu = None;
        cx.notify();
        if self.slack_files_snapshot.is_none() && !self.slack_files_loading {
            self.begin_slack_files_page(1, cx);
        }
    }

    pub(crate) fn leave_slack_files(&mut self) {
        if self.slack_active_rail_view != SlackRailView::Files {
            return;
        }
        self.slack_files_generation = next_slack_files_generation(self.slack_files_generation);
        self.slack_files_menu = None;
    }

    pub(crate) fn reset_slack_files_context(&mut self) {
        self.slack_files_generation = next_slack_files_generation(self.slack_files_generation);
        self.slack_files_team_id = None;
        self.slack_files_self_user_id = None;
        self.slack_files_browser_session_id = None;
        self.slack_files_snapshot = None;
        self.slack_files_rows = Arc::default();
        self.slack_files_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::All;
        self.slack_files_scope = SlackFilesScope::All;
        self.slack_files_type_filters = SlackFilesTypeFilter::DEFAULT.to_vec();
        self.slack_files_sort = SlackFilesSort::RecentlyViewed;
        self.slack_files_search_query.clear();
        self.slack_files_committed_query.clear();
        self.slack_files_error = None;
        self.slack_files_menu = None;
        self.slack_files_selected_id = None;
    }

    pub(crate) fn retry_slack_files(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Files || self.slack_files_loading {
            return;
        }
        let page = self
            .slack_files_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.pagination.next_page())
            .unwrap_or(1);
        self.begin_slack_files_page(page, cx);
    }

    pub(crate) fn submit_slack_files_search(&mut self, cx: &mut Context<Self>) {
        let query = self.slack_files_search_query.trim().to_string();
        if self.slack_files_committed_query == query && self.slack_files_snapshot.is_some() {
            return;
        }
        self.slack_files_search_query.clone_from(&query);
        self.slack_files_committed_query = query;
        self.slack_files_menu = None;
        self.reload_slack_files(cx);
    }

    pub(crate) fn clear_slack_files_search(&mut self, cx: &mut Context<Self>) {
        if self.slack_files_search_query.is_empty() && self.slack_files_committed_query.is_empty() {
            return;
        }
        self.slack_files_search_query.clear();
        self.slack_files_committed_query.clear();
        self.slack_files_menu = None;
        self.reload_slack_files(cx);
    }

    pub(crate) fn select_slack_files_scope(
        &mut self,
        scope: SlackFilesScope,
        cx: &mut Context<Self>,
    ) {
        if self.slack_files_scope == scope {
            return;
        }
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::All;
        self.slack_files_scope = scope;
        self.slack_files_menu = None;
        match scope {
            SlackFilesScope::All | SlackFilesScope::SharedWithYou => {
                self.slack_files_type_filters = SlackFilesTypeFilter::DEFAULT.to_vec();
                self.slack_files_sort = SlackFilesSort::RecentlyViewed;
            }
            SlackFilesScope::CreatedByYou => {
                self.slack_files_type_filters = vec![
                    SlackFilesTypeFilter::Lists,
                    SlackFilesTypeFilter::CanvasesAndDocuments,
                ];
                self.slack_files_sort = SlackFilesSort::LastUpdated;
            }
        }
        self.reload_slack_files(cx);
    }

    pub(crate) fn select_slack_files_sort(&mut self, sort: SlackFilesSort, cx: &mut Context<Self>) {
        self.slack_files_menu = None;
        if self.slack_files_sort == sort {
            cx.notify();
            return;
        }
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::All;
        self.slack_files_sort = sort;
        self.reload_slack_files(cx);
    }

    pub(crate) fn toggle_slack_files_type_filter(
        &mut self,
        filter: SlackFilesTypeFilter,
        cx: &mut Context<Self>,
    ) {
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::All;
        if let Some(index) = self
            .slack_files_type_filters
            .iter()
            .position(|candidate| *candidate == filter)
        {
            self.slack_files_type_filters.remove(index);
        } else {
            self.slack_files_type_filters.push(filter);
            self.slack_files_type_filters.sort_by_key(|candidate| {
                SlackFilesTypeFilter::ALL
                    .iter()
                    .position(|known| known == candidate)
                    .expect("Slack Files filter must have a stable menu order")
            });
        }
        self.reload_slack_files(cx);
        self.slack_files_menu = Some(SlackFilesMenu::Types);
    }

    pub(crate) fn select_slack_files_type_preset(
        &mut self,
        filter: SlackFilesTypeFilter,
        cx: &mut Context<Self>,
    ) {
        if self.slack_files_sidebar_selection == SlackFilesSidebarSelection::Type(filter)
            && self.slack_files_scope == SlackFilesScope::All
            && self.slack_files_type_filters.as_slice() == [filter]
            && self.slack_files_sort == SlackFilesSort::RecentlyViewed
            && self.slack_files_committed_query.is_empty()
        {
            return;
        }
        let request_changed = self.slack_files_scope != SlackFilesScope::All
            || self.slack_files_type_filters.as_slice() != [filter]
            || self.slack_files_sort != SlackFilesSort::RecentlyViewed
            || !self.slack_files_committed_query.is_empty();
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::Type(filter);
        self.slack_files_scope = SlackFilesScope::All;
        self.slack_files_type_filters = vec![filter];
        self.slack_files_sort = SlackFilesSort::RecentlyViewed;
        self.slack_files_search_query.clear();
        self.slack_files_committed_query.clear();
        if request_changed {
            self.reload_slack_files(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn select_slack_files_all(&mut self, cx: &mut Context<Self>) {
        if self.slack_files_sidebar_selection == SlackFilesSidebarSelection::All
            && self.slack_files_scope == SlackFilesScope::All
            && self.slack_files_type_filters.as_slice() == SlackFilesTypeFilter::DEFAULT
            && self.slack_files_sort == SlackFilesSort::RecentlyViewed
            && self.slack_files_committed_query.is_empty()
        {
            return;
        }
        let request_changed = self.slack_files_scope != SlackFilesScope::All
            || self.slack_files_type_filters.as_slice() != SlackFilesTypeFilter::DEFAULT
            || self.slack_files_sort != SlackFilesSort::RecentlyViewed
            || !self.slack_files_committed_query.is_empty();
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::All;
        self.slack_files_scope = SlackFilesScope::All;
        self.slack_files_type_filters = SlackFilesTypeFilter::DEFAULT.to_vec();
        self.slack_files_sort = SlackFilesSort::RecentlyViewed;
        self.slack_files_search_query.clear();
        self.slack_files_committed_query.clear();
        if request_changed {
            self.reload_slack_files(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn select_slack_files_recently_viewed(&mut self, cx: &mut Context<Self>) {
        if self.slack_files_sidebar_selection == SlackFilesSidebarSelection::RecentlyViewed
            && self.slack_files_scope == SlackFilesScope::All
            && self.slack_files_type_filters.as_slice() == SlackFilesTypeFilter::DEFAULT
            && self.slack_files_sort == SlackFilesSort::RecentlyViewed
            && self.slack_files_committed_query.is_empty()
        {
            return;
        }
        let request_changed = self.slack_files_scope != SlackFilesScope::All
            || self.slack_files_type_filters.as_slice() != SlackFilesTypeFilter::DEFAULT
            || self.slack_files_sort != SlackFilesSort::RecentlyViewed
            || !self.slack_files_committed_query.is_empty();
        self.slack_files_sidebar_selection = SlackFilesSidebarSelection::RecentlyViewed;
        self.slack_files_scope = SlackFilesScope::All;
        self.slack_files_type_filters = SlackFilesTypeFilter::DEFAULT.to_vec();
        self.slack_files_sort = SlackFilesSort::RecentlyViewed;
        self.slack_files_search_query.clear();
        self.slack_files_committed_query.clear();
        if request_changed {
            self.reload_slack_files(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn toggle_slack_files_menu(&mut self, menu: SlackFilesMenu, cx: &mut Context<Self>) {
        self.slack_files_menu = (self.slack_files_menu != Some(menu)).then_some(menu);
        cx.notify();
    }

    pub(crate) fn close_slack_files_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_files_menu.take().is_some() {
            cx.notify();
        }
    }
}

fn next_slack_files_generation(generation: u64) -> u64 {
    generation
        .checked_add(1)
        .expect("Slack Files request generation overflowed")
}
