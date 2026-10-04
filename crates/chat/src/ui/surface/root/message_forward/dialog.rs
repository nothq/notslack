use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, BoxShadow, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled,
};

use super::{
    slack_message_forward_action_key, SLACK_MESSAGE_FORWARD_CONTENT_LEFT,
    SLACK_MESSAGE_FORWARD_CONTENT_WIDTH,
};
use crate::ui::alpha;
use crate::ui::surface::{
    slack_icon, SlackMessageForwardDestination, SlackShellIcon, SurfaceState,
};

impl SurfaceState {
    pub(super) fn render_slack_message_forward_header(
        &self,
        title: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .absolute()
            .top(px(20.0))
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .right(px(8.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .line_height(px(30.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(title),
            )
            .child(
                div()
                    .id("slack-message-forward-close")
                    .role(Role::Button)
                    .aria_label("Close forward dialog")
                    .focusable()
                    .tab_stop(true)
                    .size(px(36.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(0xf8f8f8, 0.08)))
                    .focus_visible(|style| style.bg(alpha(0xf8f8f8, 0.08)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_message_forward(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if slack_message_forward_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.close_slack_message_forward(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(SlackShellIcon::Close, 0xb9babd, 24.0, cx)),
            )
    }

    pub(super) fn render_slack_message_forward_destination_control(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward destination control requires modal state");
        div()
            .id("slack-message-forward-destination-control")
            .absolute()
            .top(px(78.0))
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .w(px(SLACK_MESSAGE_FORWARD_CONTENT_WIDTH))
            .h(px(36.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(if modal.destination.is_none() {
                0x1d9bd1
            } else {
                0x565856
            }))
            .when(modal.destination.is_none(), |this| {
                this.shadow(vec![BoxShadow {
                    color: alpha(0x1d9bd1, 0.30),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(4.0),
                    inset: false,
                }])
                .child(self.slack_message_forward_destination_input_entity(cx))
            })
            .when_some(modal.destination.as_ref(), |this, destination| {
                this.child(self.render_slack_message_forward_destination_chip(destination, cx))
            })
    }

    fn render_slack_message_forward_destination_chip(
        &self,
        destination: &SlackMessageForwardDestination,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let opening = destination.opening;
        div()
            .id("slack-message-forward-selected-destination")
            .role(Role::Button)
            .aria_label(format!(
                "Forward to {}. Activate to change destination.",
                destination.label
            ))
            .focusable()
            .tab_stop(true)
            .size_full()
            .px(px(12.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(8.0))
            .hover(|style| style.bg(alpha(0xf8f8f8, 0.05)))
            .focus_visible(|style| style.bg(alpha(0xf8f8f8, 0.05)))
            .child(self.render_slack_message_forward_destination_icon(
                destination.kind,
                0xb9babd,
                cx,
            ))
            .child(slack_message_forward_destination_label(destination))
            .when(opening, |this| {
                this.child(
                    div()
                        .flex_none()
                        .text_size(px(13.0))
                        .text_color(rgb(0x9a9b9e))
                        .child("Opening…"),
                )
            })
            .child(slack_icon(SlackShellIcon::Close, 0xb9babd, 16.0, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clear_slack_message_forward_destination(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if slack_message_forward_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.clear_slack_message_forward_destination(cx);
                }
            }))
    }

    pub(super) fn render_slack_message_forward_note(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .absolute()
            .top(px(130.0))
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .w(px(SLACK_MESSAGE_FORWARD_CONTENT_WIDTH))
            .h(px(124.0))
            .child(self.slack_message_forward_note_input_entity(cx))
    }

    pub(super) fn render_slack_message_forward_preview(&self) -> impl IntoElement {
        let source = &self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward preview requires modal state")
            .source;
        div()
            .absolute()
            .top(px(270.0))
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .w(px(SLACK_MESSAGE_FORWARD_CONTENT_WIDTH))
            .h(px(44.7))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(0x35373b))
            .bg(rgb(0x222529))
            .px(px(12.0))
            .overflow_hidden()
            .child(slack_message_forward_preview_content(source))
    }

    pub(super) fn render_slack_message_forward_error(&self, error: &str) -> impl IntoElement {
        div()
            .absolute()
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .right(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .top(px(319.0))
            .h(px(18.0))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_size(px(12.0))
            .line_height(px(18.0))
            .text_color(rgb(0xe01e5a))
            .child(error.to_string())
    }
}

fn slack_message_forward_destination_label(destination: &SlackMessageForwardDestination) -> Div {
    div()
        .min_w(px(0.0))
        .flex_grow(1.0)
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(15.0))
        .line_height(px(22.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xf8f8f8))
        .child(destination.label.clone())
}

fn slack_message_forward_preview_content(
    source: &crate::ui::surface::SlackMessageForwardSource,
) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .flex_none()
                .max_w(px(128.0))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(15.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xf8f8f8))
                .child(source.author.clone()),
        )
        .child(
            div()
                .min_w(px(0.0))
                .flex_grow(1.0)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .text_color(rgb(0xd1d2d3))
                .child(source.preview.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(rgb(0x9a9b9e))
                .child(source.timestamp_label.clone()),
        )
}
