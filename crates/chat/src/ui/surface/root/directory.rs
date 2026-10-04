mod list;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled,
};

use crate::ui::surface::{slack_icon, slack_palette, SlackShellIcon, SurfaceState};

const SLACK_DIRECTORY_ROW_HEIGHT: f32 = 76.0;

impl SurfaceState {
    pub(super) fn render_slack_directory_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-people-surface")
            .role(Role::Main)
            .aria_label("People")
            .flex_grow(1.0)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .bg(rgb(palette.main_bg))
            .flex()
            .flex_col()
            .child(self.render_slack_directory_header(cx))
            .child(self.render_slack_directory_content(cx))
            .into_any_element()
    }

    fn render_slack_directory_header(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_none()
            .px(px(28.0))
            .pt(px(22.0))
            .pb(px(18.0))
            .border_b_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .flex_col()
            .gap(px(17.0))
            .child(self.render_slack_directory_title())
            .child(self.render_slack_directory_search(cx))
    }

    fn render_slack_directory_title(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex()
            .items_baseline()
            .gap(px(9.0))
            .child(
                div()
                    .text_size(px(22.0))
                    .line_height(px(28.0))
                    .font_weight(FontWeight::BLACK)
                    .text_color(rgb(palette.main_text))
                    .child("People"),
            )
            .when(self.slack_directory_snapshot.is_some(), |this| {
                this.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child(self.slack_directory_rows.len().to_string()),
                )
            })
    }

    fn render_slack_directory_search(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(42.0))
            .w_full()
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.composer_chip_border))
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(9.0))
            .child(slack_icon(
                SlackShellIcon::Search,
                palette.main_secondary_text,
                19.0,
                cx,
            ))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .h_full()
                    .child(self.slack_directory_search_input_entity(cx)),
            )
    }

    fn render_slack_directory_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_directory_rows.is_empty() && self.slack_directory_loading {
            return self.render_slack_directory_skeleton().into_any_element();
        }
        if let Some(error) = self
            .slack_directory_error
            .as_deref()
            .filter(|_| self.slack_directory_rows.is_empty())
        {
            return self
                .render_slack_directory_error(error, cx)
                .into_any_element();
        }
        if self.slack_directory_visible_row_indices.is_empty() {
            return self.render_slack_directory_empty().into_any_element();
        }
        self.render_slack_directory_list(cx)
    }

    fn render_slack_directory_skeleton(&self) -> impl IntoElement {
        let fill = match self.appearance_mode {
            crate::ui::AppearanceMode::Dark => 0x34363b,
            crate::ui::AppearanceMode::Light => 0xe8e8e8,
        };
        div()
            .id("slack-people-loading")
            .role(Role::Status)
            .aria_label("Loading People")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .children((0..7).map(|index| slack_directory_skeleton_row(index, fill)))
    }

    fn render_slack_directory_empty(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(if self.slack_directory_normalized_query.is_empty() {
                "No people are available in this workspace."
            } else {
                "No people match your search."
            })
    }

    fn render_slack_directory_error(
        &self,
        error: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text_color = match self.appearance_mode {
            crate::ui::AppearanceMode::Dark => 0xf2d8d6,
            crate::ui::AppearanceMode::Light => 0x8e1f0b,
        };
        div()
            .id("slack-people-error")
            .role(Role::Alert)
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(32.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(14.0))
            .text_color(rgb(text_color))
            .child(error.to_string())
            .child(self.render_slack_directory_retry_button(cx))
    }

    fn render_slack_directory_retry_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-people-retry")
            .role(Role::Button)
            .aria_label("Retry loading People")
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .bg(rgb(0x1264a3))
            .text_color(rgb(0xffffff))
            .font_weight(FontWeight::BOLD)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .child("Retry")
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_directory(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if slack_directory_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_directory(cx);
                }
            }))
    }
}

fn slack_directory_skeleton_row(index: usize, fill: u32) -> Div {
    div()
        .h(px(SLACK_DIRECTORY_ROW_HEIGHT))
        .px(px(28.0))
        .py(px(5.0))
        .child(
            div()
                .h(px(66.0))
                .w_full()
                .rounded(px(9.0))
                .border_1()
                .border_color(rgb(fill))
                .px(px(14.0))
                .flex()
                .items_center()
                .gap(px(13.0))
                .child(div().size(px(42.0)).rounded_full().bg(rgb(fill)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(7.0))
                        .child(
                            div()
                                .w(px(116.0 + (index % 3) as f32 * 28.0))
                                .h(px(11.0))
                                .rounded(px(5.0))
                                .bg(rgb(fill)),
                        )
                        .child(
                            div()
                                .w(px(78.0 + (index % 4) as f32 * 20.0))
                                .h(px(8.0))
                                .rounded(px(4.0))
                                .bg(rgb(fill)),
                        ),
                ),
        )
}

fn slack_directory_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
