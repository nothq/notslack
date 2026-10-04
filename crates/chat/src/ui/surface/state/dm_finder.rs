use std::{rc::Rc, sync::Arc};

use gpui::{Entity, ScrollStrategy, UniformListScrollHandle, Window};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SlackRailView, SurfaceState};
use crate::ui::surface::{normalize_slack_dm_finder_text, slack_palette};
use crate::ui::{alpha, px, rgb};

impl SurfaceState {
    pub(crate) fn slack_dm_finder_active(&self) -> bool {
        self.slack_dm_finder_focused || !self.slack_dm_finder_normalized_query.is_empty()
    }

    pub(crate) fn slack_dm_finder_input_entity(&self, cx: &mut Context<Self>) -> Entity<TextInput> {
        let props = self.slack_dm_finder_input_props(cx);
        self.slack_dm_finder_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_dm_finder_input.clone()
    }

    fn slack_dm_finder_input_props(&self, cx: &mut Context<Self>) -> TextInputProps {
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_dm_finder_query(value, cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_submit: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.activate_selected_slack_dm_finder_result(cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_escape: TextInputAction = Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.reset_slack_dm_finder();
                    cx.notify();
                    surface.focus_handle.clone()
                })
                .ok();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        });
        let on_up = Self::slack_dm_finder_move_action(cx, -1);
        let on_down = Self::slack_dm_finder_move_action(cx, 1);
        let surface = cx.entity().downgrade();
        let on_focus: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.focus_slack_dm_finder(cx);
                })
                .ok();
        });

        TextInputProps::single_line(self.slack_dm_finder_query.clone())
            .placeholder("Find a DM…")
            .style(self.slack_dm_finder_input_style())
            .bordered(false)
            .request_focus(self.slack_dm_finder_focused)
            .accessibility(
                self.slack_dm_finder_accessibility_id.clone(),
                "Find a direct message",
            )
            .on_change(on_change)
            .on_submit(on_submit)
            .on_escape(on_escape)
            .on_up(on_up)
            .on_down(on_down)
            .on_focus(on_focus)
    }

    fn slack_dm_finder_move_action(cx: &mut Context<Self>, direction: i32) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.move_slack_dm_finder_selection(direction, cx);
                })
                .ok();
        })
    }

    fn slack_dm_finder_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(28.0),
            min_height: px(28.0),
            padding_x: px(0.0),
            padding_y: px(4.0),
            radius: px(0.0),
            background: alpha(0x000000, 0.0),
            border: alpha(0x000000, 0.0),
            focused_border: alpha(0x000000, 0.0),
            text: rgb(palette.sidebar_header_text).into(),
            placeholder: rgb(palette.sidebar_muted_text).into(),
            selection: alpha(0x1264a3, 0.36),
            caret: rgb(palette.sidebar_header_text).into(),
            font_size: px(15.0),
            line_height: px(20.0),
            font_family: Some("Lato".into()),
        }
    }

    pub(crate) fn focus_slack_dm_finder(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Dms || self.slack_dms_peek_visible {
            return;
        }
        if !self.slack_dm_finder_focused {
            self.slack_dm_finder_focused = true;
            self.rebuild_slack_dm_finder_results();
        }
        cx.notify();
    }

    pub(crate) fn clear_slack_dm_finder_query(&mut self, cx: &mut Context<Self>) {
        if self.slack_dm_finder_query.is_empty() {
            return;
        }
        self.slack_dm_finder_query.clear();
        self.slack_dm_finder_normalized_query = Default::default();
        self.slack_dm_finder_focused = true;
        self.rebuild_slack_dm_finder_results();
        cx.notify();
    }

    pub(super) fn set_slack_dm_finder_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.slack_dm_finder_normalized_query = normalize_slack_dm_finder_text(&query).into();
        self.slack_dm_finder_query = query;
        self.slack_dm_finder_focused = true;
        self.rebuild_slack_dm_finder_results();
        cx.notify();
    }

    pub(super) fn rebuild_slack_dm_finder_results(&mut self) {
        if !self.slack_dm_finder_active() {
            self.slack_dm_finder_row_indices = Arc::default();
            self.slack_dm_finder_selected_index = None;
            self.slack_dm_finder_scroll_handle = UniformListScrollHandle::new();
            self.slack_dm_finder_prefetched_range = None;
            return;
        }
        self.slack_dm_finder_row_indices = self
            .slack_dm_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                self.slack_dm_finder_normalized_query.is_empty()
                    || row
                        .finder_search_key
                        .contains(self.slack_dm_finder_normalized_query.as_ref())
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        self.slack_dm_finder_selected_index = None;
        if !self.slack_dm_finder_row_indices.is_empty() {
            self.slack_dm_finder_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }
        self.slack_dm_finder_prefetched_range = None;
    }

    pub(crate) fn select_slack_dm_finder_result(
        &mut self,
        finder_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation_id) = self
            .slack_dm_finder_row_indices
            .get(finder_index)
            .and_then(|row_index| self.slack_dm_rows.get(*row_index))
            .map(|row| row.conversation_id.to_string())
        else {
            return;
        };
        self.reset_slack_dm_finder();
        self.select_slack_dm_conversation(&conversation_id, cx);
    }

    fn activate_selected_slack_dm_finder_result(&mut self, cx: &mut Context<Self>) {
        if self.slack_dm_finder_row_indices.is_empty() {
            return;
        }
        self.select_slack_dm_finder_result(self.slack_dm_finder_selected_index.unwrap_or(0), cx);
    }

    fn move_slack_dm_finder_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        let count = self.slack_dm_finder_row_indices.len();
        if count == 0 {
            return;
        }
        let next = match self.slack_dm_finder_selected_index {
            None if direction < 0 => count - 1,
            None => 0,
            Some(current) if direction < 0 => current.saturating_sub(1),
            Some(current) => current.saturating_add(1).min(count - 1),
        };
        self.slack_dm_finder_selected_index = Some(next);
        self.slack_dm_finder_scroll_handle
            .scroll_to_item(next, ScrollStrategy::Nearest);
        self.queue_slack_dm_finder_visible_images(next, next.saturating_add(1), cx);
        cx.notify();
    }

    pub(crate) fn reset_slack_dm_finder(&mut self) {
        self.slack_dm_finder_query.clear();
        self.slack_dm_finder_normalized_query = Default::default();
        self.slack_dm_finder_focused = false;
        self.slack_dm_finder_row_indices = Arc::default();
        self.slack_dm_finder_selected_index = None;
        self.slack_dm_finder_scroll_handle = UniformListScrollHandle::new();
        self.slack_dm_finder_prefetched_range = None;
    }

    pub(crate) fn ensure_slack_dm_finder_focus_observers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.slack_dm_finder_input.read(cx).focus_handle_clone();
        if !self.slack_dm_finder_focus_observers_registered {
            cx.on_focus(&focus, window, |surface, _, cx| {
                if surface.slack_active_rail_view == SlackRailView::Dms
                    && !surface.slack_dms_peek_visible
                    && !surface.slack_dm_finder_focused
                {
                    surface.slack_dm_finder_focused = true;
                    surface.rebuild_slack_dm_finder_results();
                    cx.notify();
                }
            })
            .detach();
            cx.on_blur(&focus, window, |surface, _, cx| {
                if surface.slack_dm_finder_focused {
                    surface.slack_dm_finder_focused = false;
                    if surface.slack_dm_finder_normalized_query.is_empty() {
                        surface.slack_dm_finder_query.clear();
                        surface.slack_dm_finder_row_indices = Arc::default();
                        surface.slack_dm_finder_selected_index = None;
                        surface.slack_dm_finder_scroll_handle = UniformListScrollHandle::new();
                        surface.slack_dm_finder_prefetched_range = None;
                    }
                    cx.notify();
                }
            })
            .detach();
            self.slack_dm_finder_focus_observers_registered = true;
        }
        if (!self.slack_dm_finder_active() || self.slack_active_rail_view != SlackRailView::Dms)
            && focus.is_focused(window)
        {
            window.focus(&self.focus_handle, cx);
        }
    }
}
