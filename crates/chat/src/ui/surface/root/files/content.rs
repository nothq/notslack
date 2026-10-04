use super::{slack_files_action_key, SLACK_FILES_ROW_HEIGHT};
use crate::ui::surface::{SlackFileRow, SurfaceState};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, uniform_list, AnyElement, Context, Div, FontWeight, InteractiveElement,
    IntoElement, ListSizingBehavior, ParentElement, Role, StatefulInteractiveElement, Styled,
};

#[derive(Clone, Copy)]
struct SlackFileRowPosition {
    index: usize,
    count: usize,
}

impl SurfaceState {
    pub(super) fn render_slack_files_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_files_rows.is_empty() {
            if self.slack_files_loading {
                return self.render_slack_files_skeleton().into_any_element();
            }
            if let Some(error) = self.slack_files_error.as_deref() {
                return self.render_slack_files_error(error, cx).into_any_element();
            }
            return self.render_slack_files_empty().into_any_element();
        }
        self.render_slack_files_list(cx)
    }

    fn render_slack_files_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let row_count = self.slack_files_rows.len();
        let scroll_handle = self.slack_files_scroll_handle.clone();
        div()
            .id("slack-files-list")
            .role(Role::ListBox)
            .aria_label("Slack files")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(0x34363a))
            .overflow_hidden()
            .child(
                uniform_list("slack-files-rows", row_count, move |range, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.handle_slack_files_list_scroll(range.end, row_count, cx);
                        range
                            .map(|index| {
                                let row = &this.slack_files_rows[index];
                                this.render_slack_file_row(row, index, row_count, cx)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .track_scroll(&scroll_handle)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_file_row(
        &self,
        row: &SlackFileRow,
        index: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.slack_files_selected_id.as_ref() == Some(&row.id);
        self.slack_file_row_shell(row, SlackFileRowPosition { index, count }, selected, cx)
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(self.render_slack_file_tile(row))
            .child(slack_file_row_text(row))
            .into_any_element()
    }

    fn slack_file_row_shell(
        &self,
        row: &SlackFileRow,
        position: SlackFileRowPosition,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let id = row.id.clone();
        let open_id = row.id.clone();
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(position.index + 1)
            .aria_size_of_set(position.count)
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_FILES_ROW_HEIGHT))
            .w_full()
            .px(px(12.0))
            .border_b_1()
            .border_color(rgb(0x34363a))
            .when(selected, |this| this.bg(rgb(0x25282d)))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x222529)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_file(open_id.as_ref(), cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "enter" | "space" if !event.keystroke.modifiers.modified() => {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.select_slack_file_row(id.clone(), cx);
                            this.open_selected_slack_file(cx);
                        }
                        "up" if !event.keystroke.modifiers.modified() => {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.move_slack_file_selection(-1, cx);
                        }
                        "down" if !event.keystroke.modifiers.modified() => {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.move_slack_file_selection(1, cx);
                        }
                        _ => {}
                    }
                }),
            )
    }

    fn render_slack_file_tile(&self, row: &SlackFileRow) -> Div {
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(px(8.0))
            .bg(rgb(row.visual.tile_fill()))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(if row.visual.glyph().len() > 1 {
                8.0
            } else {
                17.0
            }))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.visual.glyph())
    }

    fn render_slack_files_skeleton(&self) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(0x34363a))
            .children((0..7).map(|index| {
                div()
                    .h(px(SLACK_FILES_ROW_HEIGHT))
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(rgb(0x34363a))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(div().size(px(36.0)).rounded(px(8.0)).bg(rgb(0x303238)))
                    .child(
                        div()
                            .flex_grow(1.0)
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(
                                div()
                                    .w(px(180.0 + index as f32 * 13.0))
                                    .h(px(12.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x303238)),
                            )
                            .child(
                                div()
                                    .w(px(260.0))
                                    .h(px(9.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x292b2f)),
                            ),
                    )
            }))
    }

    fn render_slack_files_error(&self, message: &str, cx: &mut Context<Self>) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(0x6e3c39))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(14.0))
            .text_color(rgb(0xf2d8d6))
            .child(message.to_string())
            .child(self.render_slack_files_retry(cx))
    }

    fn render_slack_files_retry(&self, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        div()
            .id("slack-files-retry")
            .role(Role::Button)
            .aria_label("Retry loading Slack files")
            .focusable()
            .tab_stop(true)
            .h(px(30.0))
            .px(px(14.0))
            .rounded(px(5.0))
            .bg(rgb(0xf8f8f8))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_files(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_files(cx);
                }
            }))
            .flex()
            .items_center()
            .text_color(rgb(0x1d1c1d))
            .font_weight(FontWeight::BOLD)
            .child("Retry")
    }

    fn render_slack_files_empty(&self) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(0x34363a))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .text_color(rgb(0xf8f8f8))
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::BOLD)
                    .child("No files found"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0xb9babd))
                    .child("Try a different search or file type."),
            )
    }
}

fn slack_file_row_text(row: &SlackFileRow) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xd1d2d3))
                .child(row.title.clone()),
        )
        .child(
            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(13.0))
                .text_color(rgb(0xb9babd))
                .child(row.metadata.clone()),
        )
}
