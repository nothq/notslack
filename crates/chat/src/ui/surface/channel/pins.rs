use gpui::{Role, SharedString, Stateful};
use gpui_components::selectable_text::SelectableTextDocument;

use super::{
    div, list, px, rgb, slack_palette, AnyElement, Context, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use crate::ui::surface::{SlackMessageDocumentPosition, SlackMessageRenderContext};

impl SurfaceState {
    pub(crate) fn render_slack_pins_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let content = if self.slack_pins_rows.is_empty() && self.slack_pins_loading {
            self.render_slack_pins_skeleton()
        } else if self.slack_pins_rows.is_empty() {
            if let Some(error) = self.slack_pins_error.as_deref() {
                self.render_slack_pins_error(error, cx)
            } else {
                self.render_slack_pins_empty()
            }
        } else {
            self.render_slack_pins_list(cx)
        };
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(31.0))
            .pt(px(20.0))
            .pb(px(20.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.0))
                    .flex_none()
                    .pl(px(4.0))
                    .pb(px(6.0))
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_muted_text))
                    .child("Pinned messages"),
            )
            .child(content)
            .into_any_element()
    }

    fn render_slack_pins_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let snapshot = self
            .slack_pins_snapshot
            .as_ref()
            .expect("Slack Pins rows require a snapshot");
        let selection_document_id: SharedString = format!(
            "slack-selection:{}:{}:pins:{}:{}",
            snapshot.team_id,
            snapshot.conversation_id,
            self.slack_pins_generation,
            self.slack_pins_rows.len(),
        )
        .into();
        let row_document_id = selection_document_id.clone();
        let list_state = self.slack_pins_list_state.clone();
        let messages = list(
            self.slack_pins_list_state.clone(),
            move |index, _window, cx| {
                view.update(cx, |this, cx| {
                    let row = this
                        .slack_pins_rows
                        .get(index)
                        .expect("Slack Pins row index should exist");
                    this.render_slack_pin_row(row, index, row_document_id.clone(), cx)
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        div()
            .id("slack-pins-list")
            .role(Role::ListBox)
            .aria_label("Pinned messages")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                SelectableTextDocument::new(
                    "slack-pins-selection-document",
                    selection_document_id,
                    messages,
                )
                .on_autoscroll(move |distance, window, _cx| {
                    list_state.scroll_by(distance);
                    window.refresh();
                }),
            )
            .into_any_element()
    }

    fn render_slack_pin_row(
        &self,
        row: &crate::ui::surface::SlackPinRow,
        index: usize,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = self.slack_pins_rows.len();
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_position_in_set(index + 1)
            .aria_size_of_set(count)
            .w_full()
            .min_w(px(0.0))
            .py(px(4.0))
            .child(
                div()
                    .w_full()
                    .min_w(px(0.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.main_border))
                    .px(px(12.0))
                    .py(px(12.0))
                    .child(self.render_slack_message_in_document(
                        &row.message,
                        SlackMessageRenderContext::Pins,
                        SlackMessageDocumentPosition::row(selection_document_id, index),
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_slack_pins_skeleton(&self) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .children((0..5).map(|_| {
                div()
                    .h(px(68.0))
                    .px(px(16.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.main_border))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(px(6.0))
                            .bg(rgb(palette.attachment_bg)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .w(px(160.0))
                                    .h(px(10.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(palette.attachment_bg)),
                            )
                            .child(
                                div()
                                    .w(px(320.0))
                                    .h(px(9.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(palette.attachment_bg)),
                            ),
                    )
            }))
            .into_any_element()
    }

    fn render_slack_pins_empty(&self) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .text_color(rgb(palette.main_text))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .child("No pinned messages"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_muted_text))
                    .child("Messages pinned in this channel will appear here."),
            )
            .into_any_element()
    }

    fn render_slack_pins_error(&self, error: &str, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .child(
                div()
                    .max_w(px(520.0))
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_muted_text))
                    .child(error.to_string()),
            )
            .child(self.render_slack_pins_retry(cx))
            .into_any_element()
    }

    fn render_slack_pins_retry(&self, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-pins-retry")
            .role(Role::Button)
            .aria_label("Retry loading pinned messages")
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .bg(rgb(0x1264a3))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .hover(|style| style.bg(rgb(0x0b4c8c)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_pins(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_pins(cx);
                }
            }))
            .focus_visible(move |style| style.border_1().border_color(rgb(palette.main_text)))
    }
}
