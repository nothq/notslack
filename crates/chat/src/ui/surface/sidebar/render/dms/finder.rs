use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, point, px, rgb, uniform_list, AnyElement, BoxShadow, Context, Div, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

use crate::ui::surface::{
    slack_base_icon_radius, slack_icon, slack_palette, SlackDmRow, SlackShellIcon, SurfaceState,
};
use crate::ui::{alpha, SlackConversationKind, SlackUserPresence};

mod row_context;

use row_context::SlackDmFinderRowContext;

const SLACK_DM_FINDER_IDLE_BAND_HEIGHT: f32 = 32.0;
const SLACK_DM_FINDER_ACTIVE_BAND_HEIGHT: f32 = 42.0;
const SLACK_DM_FINDER_FIELD_HEIGHT: f32 = 28.0;
const SLACK_DM_FINDER_SECTION_HEIGHT: f32 = 29.0;
const SLACK_DM_FINDER_ROW_HEIGHT: f32 = 28.0;

impl SurfaceState {
    pub(super) fn render_slack_dm_finder_search(
        &self,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let focused = self.slack_dm_finder_focused;
        div()
            .h(px(if expanded {
                SLACK_DM_FINDER_ACTIVE_BAND_HEIGHT
            } else {
                SLACK_DM_FINDER_IDLE_BAND_HEIGHT
            }))
            .flex_none()
            .px(px(8.0))
            .when(expanded, |this| this.pt(px(4.0)).pb(px(10.0)))
            .when(!expanded, |this| this.pb(px(4.0)))
            .child(self.render_slack_dm_finder_field(focused, cx))
    }

    fn render_slack_dm_finder_field(&self, focused: bool, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_DM_FINDER_FIELD_HEIGHT))
            .w_full()
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(if focused {
                0x1d9bd1
            } else {
                palette.sidebar_border
            }))
            .bg(rgb(palette.sidebar_bg))
            .flex()
            .items_center()
            .overflow_hidden()
            .when(focused, |this| {
                this.shadow(vec![BoxShadow {
                    color: alpha(0x1d9bd1, 0.48),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(4.0),
                    spread_radius: px(2.0),
                    inset: false,
                }])
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.focus_slack_dm_finder(cx);
                }),
            )
            .child(
                div()
                    .w(px(34.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(
                        SlackShellIcon::ListSearch,
                        palette.sidebar_icon,
                        18.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .h_full()
                    .child(self.slack_dm_finder_input_entity(cx)),
            )
            .when(!self.slack_dm_finder_query.is_empty(), |this| {
                this.child(self.render_slack_dm_finder_clear(cx))
            })
    }

    fn render_slack_dm_finder_clear(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("slack-dm-finder-clear")
            .role(Role::Button)
            .aria_label("Clear direct message search")
            .focusable()
            .tab_stop(true)
            .w(px(32.0))
            .h_full()
            .flex_none()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(16.0))
                    .rounded_full()
                    .bg(rgb(0xf8f8f8))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(SlackShellIcon::Close, 0x1d1c1d, 12.0, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.clear_slack_dm_finder_query(cx);
                    let focus = this.slack_dm_finder_input.read(cx).focus_handle_clone();
                    window.focus(&focus, cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    this.clear_slack_dm_finder_query(cx);
                    let focus = this.slack_dm_finder_input.read(cx).focus_handle_clone();
                    window.focus(&focus, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn render_slack_dm_finder_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let result_count = self.slack_dm_finder_row_indices.len();
        let selected_index = self.slack_dm_finder_selected_index;
        let empty_label = if self.slack_dm_inbox_snapshot.is_none() {
            "Loading direct messages…"
        } else if self.slack_dm_finder_normalized_query.is_empty() {
            "No direct messages found"
        } else {
            "No direct messages match your search"
        };

        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_slack_dm_finder_section(cx))
            .when(result_count == 0, |this| {
                this.child(self.render_slack_dm_finder_empty(empty_label))
            })
            .when(result_count > 0, |this| {
                this.child(self.render_slack_dm_finder_list(result_count, selected_index, cx))
            })
            .into_any_element()
    }

    fn render_slack_dm_finder_empty(&self, label: &'static str) -> Div {
        div()
            .h(px(56.0))
            .px(px(32.0))
            .flex_none()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(slack_palette(self.appearance_mode).sidebar_muted_text))
            .child(label)
    }

    fn render_slack_dm_finder_list(
        &self,
        result_count: usize,
        selected_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self.slack_dm_rows.clone();
        let result_indices = self.slack_dm_finder_row_indices.clone();
        let scroll_handle = self.slack_dm_finder_scroll_handle.clone();
        let view = cx.entity();
        div()
            .id("slack-dm-finder-listbox")
            .role(Role::ListBox)
            .aria_label("Direct message search results")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                uniform_list(
                    "slack-dm-finder-results",
                    result_count,
                    move |range, _window, cx| {
                        let visible_start = range.start;
                        let visible_end = range.end;
                        let rows = rows.clone();
                        let result_indices = result_indices.clone();
                        view.update(cx, move |this, cx| {
                            this.queue_slack_dm_finder_visible_images(
                                visible_start,
                                visible_end,
                                cx,
                            );
                            range
                                .map(|finder_index| {
                                    let row_index = *result_indices
                                        .get(finder_index)
                                        .expect("Slack DM finder row index should exist");
                                    let row = rows
                                        .get(row_index)
                                        .expect("prepared Slack DM row should exist");
                                    this.render_slack_dm_finder_row(
                                        row,
                                        SlackDmFinderRowContext {
                                            index: finder_index,
                                            selected: selected_index == Some(finder_index),
                                            count: result_count,
                                        },
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .track_scroll(&scroll_handle)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_dm_finder_section(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_DM_FINDER_SECTION_HEIGHT))
            .flex_none()
            .pl(px(18.0))
            .pr(px(8.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(15.0))
            .text_color(rgb(palette.sidebar_section_text))
            .child(slack_icon(
                SlackShellIcon::Dm,
                palette.sidebar_section_icon,
                18.0,
                cx,
            ))
            .child("Direct messages")
    }

    fn render_slack_dm_finder_row(
        &self,
        row: &SlackDmRow,
        context: SlackDmFinderRowContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let background = if context.selected {
            0x1264a3
        } else {
            palette.sidebar_bg
        };
        let element = div()
            .id(row.finder_element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.finder_accessibility_label.clone())
            .aria_selected(context.selected)
            .aria_position_in_set(context.index + 1)
            .aria_size_of_set(context.count)
            .w_full()
            .h(px(SLACK_DM_FINDER_ROW_HEIGHT))
            .flex_none()
            .pl(px(32.0))
            .pr(px(8.0))
            .bg(rgb(background))
            .flex()
            .items_center()
            .gap(px(8.0))
            .overflow_hidden()
            .child(self.render_slack_dm_finder_avatar(row, background))
            .child(self.render_slack_dm_finder_title(row, context.selected, palette.sidebar_text));
        if !self.slack_workspace_api_capabilities.load_conversation {
            return element.into_any_element();
        }
        element
            .cursor_pointer()
            .hover(|style| style.bg(rgb(if context.selected { 0x1264a3 } else { 0x27292d })))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.select_slack_dm_finder_result(context.index, cx);
                }),
            )
            .into_any_element()
    }

    fn render_slack_dm_finder_title(
        &self,
        row: &SlackDmRow,
        selected: bool,
        sidebar_text: u32,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(if selected { 0xffffff } else { sidebar_text }))
            .child(row.title.clone())
    }

    fn render_slack_dm_finder_avatar(&self, row: &SlackDmRow, row_background: u32) -> Div {
        if row.kind == SlackConversationKind::GroupMessage {
            return div()
                .size(px(16.0))
                .rounded(px(4.0))
                .bg(rgb(0xd1d2d3))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .line_height(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x1d1c1d))
                .child(
                    row.finder_group_count_label
                        .clone()
                        .expect("Slack group DM finder row must have a participant count"),
                );
        }
        let participant = row.participants.first();
        let avatar = if let Some(image) = participant
            .and_then(|participant| participant.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            div()
                .size(px(16.0))
                .rounded(slack_base_icon_radius(16.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(16.0)))
        } else {
            div()
                .size(px(16.0))
                .rounded(slack_base_icon_radius(16.0))
                .bg(rgb(row.finder_avatar_fill))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(7.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(row.finder_avatar_initials.clone())
        };
        avatar.relative().when_some(
            participant.and_then(|participant| participant.presence),
            |this, presence| this.child(slack_dm_finder_presence_badge(presence, row_background)),
        )
    }
}

fn slack_dm_finder_presence_badge(presence: SlackUserPresence, row_background: u32) -> Div {
    let fill = match presence {
        SlackUserPresence::Active => 0x2bac76,
        SlackUserPresence::Away => row_background,
    };
    div()
        .absolute()
        .right(px(-1.0))
        .bottom(px(-1.0))
        .size(px(5.0))
        .rounded_full()
        .bg(rgb(fill))
        .border_1()
        .border_color(rgb(if presence == SlackUserPresence::Away {
            0xbababa
        } else {
            row_background
        }))
}
