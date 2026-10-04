mod helpers;

use helpers::{
    flush_slack_home_finder_section, slack_home_finder_conversation_kind,
    slack_home_finder_input_style,
};
use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::{Entity, ListOffset, Window};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SlackRailView, SlackSidebarRowKind, SurfaceState};
use crate::ui::surface::{normalize_slack_dm_finder_text, slack_palette};
use crate::ui::{alpha, px, rgb, SlackConversationKind};

const SLACK_HOME_FINDER_INITIAL_VISIBLE_ROWS: usize = 12;
const SLACK_HOME_FINDER_IMAGE_VISIBLE_OVERDRAW: usize = 2;

impl SurfaceState {
    pub(crate) fn initialize_slack_home_finder_input(&mut self, cx: &mut Context<Self>) {
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_home_finder_query(value, cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_submit: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.activate_selected_slack_home_finder_result(cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_escape: TextInputAction = Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.reset_slack_home_finder();
                    cx.notify();
                    surface.focus_handle.clone()
                })
                .ok();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        });
        let on_up = Self::slack_home_finder_move_action(cx, -1);
        let on_down = Self::slack_home_finder_move_action(cx, 1);
        let surface = cx.entity().downgrade();
        let on_focus: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.focus_slack_home_finder(cx);
                })
                .ok();
        });
        let props = TextInputProps::single_line("")
            .placeholder("Find a conversation…")
            .style(slack_home_finder_input_style(self.appearance_mode))
            .bordered(false)
            .accessibility(
                self.slack_home_finder_accessibility_id.clone(),
                "Channel or user name",
            )
            .on_change(on_change)
            .on_submit(on_submit)
            .on_escape(on_escape)
            .on_up(on_up)
            .on_down(on_down)
            .on_focus(on_focus);
        self.slack_home_finder_input
            .update(cx, |input, cx| input.apply_props(props, cx));
    }

    fn slack_home_finder_move_action(cx: &mut Context<Self>, direction: i32) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.move_slack_home_finder_selection(direction, cx);
                })
                .ok();
        })
    }

    pub(crate) fn slack_home_finder_active(&self) -> bool {
        self.slack_home_finder_focused || !self.slack_home_finder_normalized_query.is_empty()
    }

    pub(crate) fn slack_home_finder_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if self.slack_home_finder_input.read(cx).text() != self.slack_home_finder_query {
            let query = self.slack_home_finder_query.clone();
            self.slack_home_finder_input.update(cx, |input, cx| {
                input.set_text_and_move_cursor_to_end(query, cx);
            });
        }
        self.slack_home_finder_input.clone()
    }

    pub(crate) fn focus_slack_home_finder(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Home || self.slack_dms_peek_visible {
            return;
        }
        if !self.slack_home_finder_focused {
            self.slack_home_finder_focused = true;
            self.rebuild_slack_home_finder_results();
            self.queue_slack_home_finder_visible_images(
                0,
                SLACK_HOME_FINDER_INITIAL_VISIBLE_ROWS,
                cx,
            );
        }
        cx.notify();
    }

    pub(crate) fn clear_slack_home_finder_query(&mut self, cx: &mut Context<Self>) {
        if self.slack_home_finder_query.is_empty() {
            return;
        }
        self.slack_home_finder_query.clear();
        self.slack_home_finder_normalized_query = Default::default();
        self.slack_home_finder_focused = true;
        self.rebuild_slack_home_finder_results();
        self.queue_slack_home_finder_visible_images(0, SLACK_HOME_FINDER_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    fn set_slack_home_finder_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.slack_home_finder_query == query {
            return;
        }
        self.slack_home_finder_normalized_query = normalize_slack_dm_finder_text(&query).into();
        self.slack_home_finder_query = query;
        self.slack_home_finder_focused = true;
        self.rebuild_slack_home_finder_results();
        self.queue_slack_home_finder_visible_images(0, SLACK_HOME_FINDER_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn rebuild_slack_home_finder_results(&mut self) {
        if !self.slack_home_finder_active() {
            self.clear_slack_home_finder_results();
            return;
        }

        let query = self.slack_home_finder_normalized_query.as_ref();
        let mut row_indices = Vec::new();
        let mut selectable_indices = Vec::new();
        let mut section_header = None;
        let mut section_matches = Vec::new();
        for (source_index, row) in self.slack_sidebar_rows.iter().enumerate() {
            match &row.kind {
                SlackSidebarRowKind::SectionHeader { .. } => {
                    flush_slack_home_finder_section(
                        section_header,
                        &mut section_matches,
                        &mut row_indices,
                        &mut selectable_indices,
                    );
                    section_header = Some(source_index);
                }
                SlackSidebarRowKind::Item {
                    item,
                    finder_search_key,
                    ..
                } if !item.target_id.is_empty()
                    && slack_home_finder_conversation_kind(item.target_kind)
                    && (query.is_empty() || finder_search_key.contains(query)) =>
                {
                    section_matches.push(source_index);
                }
                _ => {}
            }
        }
        flush_slack_home_finder_section(
            section_header,
            &mut section_matches,
            &mut row_indices,
            &mut selectable_indices,
        );

        self.slack_home_finder_row_indices = row_indices.into();
        self.slack_home_finder_selectable_indices = selectable_indices.into();
        self.slack_home_finder_selected_index = None;
        self.slack_home_finder_prefetched_range = None;
        self.slack_home_finder_list_state
            .reset(self.slack_home_finder_row_indices.len());
        if !self.slack_home_finder_row_indices.is_empty() {
            self.slack_home_finder_list_state.scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: px(0.0),
            });
        }
    }

    pub(crate) fn select_slack_home_finder_result(
        &mut self,
        display_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation_id) = self
            .slack_home_finder_row_indices
            .get(display_index)
            .and_then(|source_index| self.slack_sidebar_rows.get(*source_index))
            .and_then(|row| match &row.kind {
                SlackSidebarRowKind::Item { item, .. } => Some(item.target_id.clone()),
                _ => None,
            })
        else {
            return;
        };
        self.reset_slack_home_finder();
        self.select_slack_conversation(&conversation_id, cx);
    }

    fn activate_selected_slack_home_finder_result(&mut self, cx: &mut Context<Self>) {
        let selected_index = self.slack_home_finder_selected_index.unwrap_or(0);
        let Some(display_index) = self
            .slack_home_finder_selectable_indices
            .get(selected_index)
            .copied()
        else {
            return;
        };
        self.select_slack_home_finder_result(display_index, cx);
    }

    fn move_slack_home_finder_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        let count = self.slack_home_finder_selectable_indices.len();
        if count == 0 {
            return;
        }
        let next = match self.slack_home_finder_selected_index {
            None if direction < 0 => count - 1,
            None => 0,
            Some(current) if direction < 0 => current.saturating_sub(1),
            Some(current) => current.saturating_add(1).min(count - 1),
        };
        self.slack_home_finder_selected_index = Some(next);
        let display_index = self.slack_home_finder_selectable_indices[next];
        let viewport = self.slack_home_finder_list_state.viewport_bounds();
        let selection_is_visible = self
            .slack_home_finder_list_state
            .bounds_for_item(display_index)
            .is_some_and(|bounds| {
                bounds.top() >= viewport.top() && bounds.bottom() <= viewport.bottom()
            });
        if !selection_is_visible {
            self.slack_home_finder_list_state.scroll_to(ListOffset {
                item_ix: display_index,
                offset_in_item: px(0.0),
            });
        }
        self.queue_slack_home_finder_visible_images(display_index, display_index + 1, cx);
        cx.notify();
    }

    pub(crate) fn reset_slack_home_finder(&mut self) {
        self.slack_home_finder_query.clear();
        self.slack_home_finder_normalized_query = Default::default();
        self.slack_home_finder_focused = false;
        self.clear_slack_home_finder_results();
    }

    fn clear_slack_home_finder_results(&mut self) {
        self.slack_home_finder_row_indices = Arc::default();
        self.slack_home_finder_selectable_indices = Arc::default();
        self.slack_home_finder_selected_index = None;
        self.slack_home_finder_prefetched_range = None;
        self.slack_home_finder_list_state.reset(0);
    }

    pub(crate) fn ensure_slack_home_finder_focus_observers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.slack_home_finder_input.read(cx).focus_handle_clone();
        if !self.slack_home_finder_focus_observers_registered {
            cx.on_focus(&focus, window, |surface, _, cx| {
                if surface.slack_active_rail_view == SlackRailView::Home
                    && !surface.slack_dms_peek_visible
                    && !surface.slack_home_finder_focused
                {
                    surface.slack_home_finder_focused = true;
                    surface.rebuild_slack_home_finder_results();
                    cx.notify();
                }
            })
            .detach();
            cx.on_blur(&focus, window, |surface, _, cx| {
                if surface.slack_home_finder_focused {
                    surface.slack_home_finder_focused = false;
                    if surface.slack_home_finder_normalized_query.is_empty() {
                        surface.clear_slack_home_finder_results();
                    }
                    cx.notify();
                }
            })
            .detach();
            self.slack_home_finder_focus_observers_registered = true;
        }
        if (!self.slack_home_finder_active() || self.slack_active_rail_view != SlackRailView::Home)
            && focus.is_focused(window)
        {
            window.focus(&self.focus_handle, cx);
        }
    }

    pub(crate) fn queue_slack_home_finder_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_rail_view != SlackRailView::Home || !self.slack_home_finder_active() {
            return;
        }
        let start = visible_start
            .saturating_sub(SLACK_HOME_FINDER_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_home_finder_row_indices.len());
        let end = visible_end
            .saturating_add(SLACK_HOME_FINDER_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_home_finder_row_indices.len());
        if self.slack_home_finder_prefetched_range == Some((start, end)) {
            return;
        }
        self.slack_home_finder_prefetched_range = Some((start, end));
        let mut queued = self
            .slack_pending_remote_image_urls
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let urls = self.slack_home_finder_row_indices[start..end]
            .iter()
            .filter_map(|source_index| self.slack_sidebar_rows.get(*source_index))
            .filter_map(|row| match &row.kind {
                SlackSidebarRowKind::Item { item, .. } => item.avatar_image_url.as_deref(),
                _ => None,
            })
            .filter(|url| !self.slack_remote_images.contains_key(*url))
            .filter(|url| !self.slack_active_remote_image_urls.contains(*url))
            .filter(|url| queued.insert(url))
            .map(str::to_string)
            .collect::<Vec<_>>();
        self.slack_pending_remote_image_urls
            .extend(urls.into_iter().rev());
        self.start_next_slack_remote_image_load(cx);
    }
}
