use crate::ui::surface::{
    div, img, list, px, rgb, slack_base_icon_radius, AnyElement, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement, SlackLaterRow,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::SlackLaterFilter;
use gpui::{Role, Stateful};

use super::helpers::{later_action_key, slack_later_filter_label};

const SLACK_LATER_LIST_PANE_WIDTH: f32 = 536.0;
const SLACK_LATER_HEADER_HEIGHT: f32 = 52.0;
const SLACK_LATER_TABS_HEIGHT: f32 = 35.0;

#[derive(Clone, Copy)]
struct SlackLaterRowPosition {
    index: usize,
    count: usize,
}

impl SurfaceState {
    pub(super) fn render_slack_later_list_pane(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(SLACK_LATER_LIST_PANE_WIDTH))
            .h_full()
            .min_w(px(0.0))
            .flex_none()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(0x34363a))
            .child(self.render_slack_later_header(cx))
            .child(self.render_slack_later_tabs(cx))
            .child(self.render_slack_later_feed(cx))
    }

    fn render_slack_later_header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(SLACK_LATER_HEADER_HEIGHT))
            .flex_none()
            .px(px(20.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child("Later"),
            )
            .when(
                self.slack_workspace_api_capabilities.mutate_later_reminders,
                |this| this.child(self.render_slack_later_new_reminder_button(cx)),
            )
    }

    fn render_slack_later_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-later-tabs")
            .role(Role::TabList)
            .aria_label("Later filters")
            .h(px(SLACK_LATER_TABS_HEIGHT))
            .flex_none()
            .px(px(20.0))
            .border_b_1()
            .border_color(rgb(0x34363a))
            .flex()
            .items_end()
            .gap(px(22.0))
            .children(
                SlackLaterFilter::ALL
                    .into_iter()
                    .map(|filter| self.render_slack_later_tab(filter, cx)),
            )
    }

    fn render_slack_later_tab(
        &self,
        filter: SlackLaterFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.slack_later_filter == filter;
        let count = self.slack_later_filter_count(filter);
        let label = slack_later_filter_label(filter);
        div()
            .id(format!(
                "slack-later-tab-{}",
                filter.as_api_value().replace('_', "-")
            ))
            .role(Role::Tab)
            .aria_label(format!("{label}, {count} items"))
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h_full()
            .relative()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_later_filter(filter, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if later_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_later_filter(filter, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(14.0))
            .font_weight(if selected {
                FontWeight::BOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(rgb(if selected { 0xf8f8f8 } else { 0xb9babd }))
            .child(if count > 0 {
                format!("{label} {count}")
            } else {
                label.to_string()
            })
            .when(selected, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .right(px(0.0))
                        .bottom(px(0.0))
                        .h(px(2.0))
                        .bg(rgb(0xf8f8f8)),
                )
            })
    }

    fn render_slack_later_feed(&self, cx: &mut Context<Self>) -> Div {
        let content = if self.slack_later_rows.is_empty() && self.slack_later_loading {
            self.render_slack_later_skeleton()
        } else if self.slack_later_rows.is_empty() {
            if let Some(error) = self.slack_later_error.as_deref() {
                self.render_slack_later_error(error, cx)
            } else {
                self.render_slack_later_empty()
            }
        } else {
            self.render_slack_later_list(cx)
        };
        div().flex_grow(1.0).min_h(px(0.0)).child(content)
    }

    fn render_slack_later_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        div()
            .id("slack-later-list")
            .role(Role::ListBox)
            .aria_label("Saved for later items")
            .size_full()
            .min_h(px(0.0))
            .child(
                list(
                    self.slack_later_list_state.clone(),
                    move |index, _window, cx| {
                        view.update(cx, |this, cx| {
                            let row = &this.slack_later_rows[index];
                            this.render_slack_later_row(row, index, this.slack_later_rows.len(), cx)
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_later_row(
        &self,
        row: &SlackLaterRow,
        index: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.slack_later_selected_key.as_ref() == Some(&row.key);
        self.slack_later_row_container(row, SlackLaterRowPosition { index, count }, selected, cx)
            .flex()
            .flex_col()
            .gap(px(6.0))
            .when_some(row.type_label.clone(), |this, label| {
                this.child(
                    div()
                        .h(px(16.0))
                        .text_size(px(13.0))
                        .text_color(rgb(0xb9babd))
                        .child(label),
                )
            })
            .child(self.render_slack_later_row_content(row))
            .into_any_element()
    }

    fn slack_later_row_container(
        &self,
        row: &SlackLaterRow,
        position: SlackLaterRowPosition,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let key = row.key.clone();
        let keyboard_key = key.clone();
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(position.index + 1)
            .aria_size_of_set(position.count)
            .focusable()
            .tab_stop(true)
            .h(px(row.height))
            .w_full()
            .px(px(12.0))
            .py(px(10.0))
            .border_b_1()
            .border_color(rgb(0x34363a))
            .cursor_pointer()
            .when(selected, |this| this.bg(rgb(0x22252a)))
            .hover(|style| style.bg(rgb(0x24272c)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_later_row(key.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if later_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_later_row(keyboard_key.clone(), cx);
                }
            }))
    }

    fn render_slack_later_row_content(&self, row: &SlackLaterRow) -> Div {
        div()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.render_slack_later_row_avatar(row))
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_grow(1.0)
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf8f8f8))
                            .child(row.title.clone()),
                    )
                    .when_some(row.preview.clone(), |this, preview| {
                        this.child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(14.0))
                                .text_color(rgb(0xd1d2d3))
                                .child(preview),
                        )
                    }),
            )
    }

    fn render_slack_later_row_avatar(&self, row: &SlackLaterRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)));
        }
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_label.clone())
    }
}
