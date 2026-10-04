mod loading;
mod query;

use std::{collections::HashSet, rc::Rc, sync::Arc, time::Duration};

use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    prepare_slack_conversation_files_snapshot, slack_palette,
    PreparedSlackConversationFilesSnapshot, SlackConversationFilesCache,
    SlackConversationFilesExplorerRow, SlackConversationFilesMenu,
};
use crate::ui::{
    alpha, px, rgb, spawn_background_task_for_entity, spawn_timer_task_for_entity,
    SlackConversationFilesFilter, SlackConversationFilesRequest, SlackConversationFilesSort,
    SlackFilesBrowserSessionId,
};

const SLACK_CONVERSATION_FILES_DEBOUNCE: Duration = Duration::from_millis(250);
const SLACK_CONVERSATION_FILES_PAGINATION_THRESHOLD: usize = 4;
const SLACK_CONVERSATION_FILES_INITIAL_VISIBLE_HEIGHT: f32 = 512.0;

#[derive(Clone)]
struct SlackConversationFilesLoad {
    generation: u64,
    request: SlackConversationFilesRequest,
    timezone: chrono_tz::Tz,
    cache: SlackConversationFilesCache,
}

impl SurfaceState {
    pub(crate) fn slack_conversation_files_search_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let palette = slack_palette(self.appearance_mode);
        let props = TextInputProps::single_line(self.slack_conversation_files_search_query.clone())
            .placeholder("Search")
            .style(TextInputStyle {
                height: px(34.0),
                min_height: px(34.0),
                padding_x: px(0.0),
                padding_y: px(0.0),
                radius: px(0.0),
                background: alpha(0x000000, 0.0),
                border: alpha(0x000000, 0.0),
                focused_border: alpha(0x000000, 0.0),
                text: rgb(palette.main_text).into(),
                placeholder: rgb(palette.main_secondary_text).into(),
                selection: alpha(0x1264a3, 0.35),
                caret: rgb(palette.main_text).into(),
                font_size: px(15.0),
                line_height: px(22.0),
                font_family: Some("Lato".into()),
            })
            .bordered(false)
            .accessibility(
                self.slack_conversation_files_search_accessibility_id
                    .clone(),
                "Search conversation files and links",
            )
            .on_change(self.slack_conversation_files_search_on_change(cx))
            .on_submit(self.slack_conversation_files_search_on_submit(cx))
            .on_escape(self.slack_conversation_files_search_on_escape(cx));
        self.slack_conversation_files_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_conversation_files_search_input.clone()
    }

    fn slack_conversation_files_search_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_conversation_files_search_query(value, cx);
                })
                .ok();
        })
    }

    fn slack_conversation_files_search_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.submit_slack_conversation_files_search(cx);
                })
                .ok();
        })
    }

    fn slack_conversation_files_search_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, _cx| {
                    surface.slack_conversation_files_menu = None;
                    surface.focus_handle.clone()
                })
                .ok();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    pub(crate) fn activate_slack_conversation_files(&mut self, cx: &mut Context<Self>) {
        if !self
            .slack_workspace_api_capabilities
            .load_conversation_files
        {
            return;
        }
        let Some((team_id, conversation_id)) = self.slack_workspace().and_then(|workspace| {
            workspace
                .files_tab()
                .map(|_| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        }) else {
            return;
        };
        let context_changed = self.slack_conversation_files_team_id.as_deref()
            != Some(team_id.as_str())
            || self.slack_conversation_files_conversation_id.as_deref()
                != Some(conversation_id.as_str());
        if context_changed {
            self.reset_slack_conversation_files_context();
            self.slack_conversation_files_team_id = Some(team_id);
            self.slack_conversation_files_conversation_id = Some(conversation_id);
            self.slack_conversation_files_browser_session_id = Some(
                SlackFilesBrowserSessionId::new(uuid::Uuid::new_v4().to_string())
                    .expect("UUID conversation Files browser session id must be valid"),
            );
        }
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::FilesLinks;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.slack_conversation_files_menu = None;
        cx.notify();
        if self.slack_conversation_files_loading {
            self.slack_conversation_files_reload_pending |= context_changed;
            return;
        }
        if self.slack_conversation_files_reload_pending {
            self.start_pending_slack_conversation_files_reload(cx);
        } else {
            self.ensure_slack_conversation_files_filter(cx);
        }
        self.queue_initial_slack_conversation_files_images(cx);
    }

    pub(crate) fn leave_slack_conversation_files(&mut self) {
        self.slack_conversation_files_menu = None;
    }

    pub(crate) fn reset_slack_conversation_files_context(&mut self) {
        let request_in_flight = self.slack_conversation_files_loading;
        self.slack_conversation_files_generation = next_generation(
            self.slack_conversation_files_generation,
            "conversation Files request",
        );
        self.slack_conversation_files_debounce_generation = next_generation(
            self.slack_conversation_files_debounce_generation,
            "conversation Files debounce",
        );
        self.slack_conversation_files_team_id = None;
        self.slack_conversation_files_conversation_id = None;
        self.slack_conversation_files_browser_session_id = None;
        self.slack_conversation_files_snapshot = None;
        self.slack_conversation_files_cache = SlackConversationFilesCache::default();
        self.slack_conversation_files_filter = SlackConversationFilesFilter::All;
        self.slack_conversation_files_sort = SlackConversationFilesSort::Newest;
        self.slack_conversation_files_search_query.clear();
        self.slack_conversation_files_committed_query.clear();
        self.slack_conversation_files_explorer_rows = Arc::default();
        self.slack_conversation_files_list_state.reset(0);
        self.slack_conversation_files_loading = request_in_flight;
        self.slack_conversation_files_reload_pending = request_in_flight;
        self.slack_conversation_files_error = None;
        self.slack_conversation_files_menu = None;
        self.rebuild_slack_conversation_files_explorer(true);
    }

    pub(crate) fn select_slack_conversation_files_filter(
        &mut self,
        filter: SlackConversationFilesFilter,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_files_filter == filter {
            return;
        }
        self.slack_conversation_files_filter = filter;
        self.slack_conversation_files_menu = None;
        if !self.slack_conversation_files_loading {
            self.ensure_slack_conversation_files_filter(cx);
        }
        self.rebuild_slack_conversation_files_explorer(true);
        self.queue_initial_slack_conversation_files_images(cx);
        cx.notify();
    }

    pub(crate) fn select_slack_conversation_files_sort(
        &mut self,
        sort: SlackConversationFilesSort,
        cx: &mut Context<Self>,
    ) {
        self.slack_conversation_files_menu = None;
        if self.slack_conversation_files_sort == sort {
            cx.notify();
            return;
        }
        self.slack_conversation_files_sort = sort;
        self.queue_slack_conversation_files_reload(cx);
    }

    pub(crate) fn toggle_slack_conversation_files_sort_menu(&mut self, cx: &mut Context<Self>) {
        self.slack_conversation_files_menu = (self.slack_conversation_files_menu
            != Some(SlackConversationFilesMenu::Sort))
        .then_some(SlackConversationFilesMenu::Sort);
        cx.notify();
    }

    pub(crate) fn clear_slack_conversation_files_search(&mut self, cx: &mut Context<Self>) {
        if self.slack_conversation_files_search_query.is_empty()
            && self.slack_conversation_files_committed_query.is_empty()
        {
            return;
        }
        self.slack_conversation_files_search_query.clear();
        self.slack_conversation_files_debounce_generation = next_generation(
            self.slack_conversation_files_debounce_generation,
            "conversation Files debounce",
        );
        self.commit_slack_conversation_files_search(cx);
    }
}

fn next_generation(generation: u64, label: &str) -> u64 {
    generation
        .checked_add(1)
        .unwrap_or_else(|| panic!("Slack {label} generation overflowed"))
}
