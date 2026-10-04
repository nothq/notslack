use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, AnyElement, BoxShadow, Context, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Role,
    StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::{
    alpha,
    surface::{slack_icon, SlackShellIcon, SurfaceState},
};

const SLACK_MESSAGE_DELETE_DIALOG_WIDTH: f32 = 520.0;
const SLACK_MESSAGE_DELETE_DIALOG_HEIGHT: f32 = 252.0;
const SLACK_MESSAGE_DELETE_CONTENT_LEFT: f32 = 28.0;
const SLACK_MESSAGE_DELETE_CONTENT_WIDTH: f32 = 464.0;

impl SurfaceState {
    pub(super) fn render_slack_message_delete_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_message_delete(cx);
            }),
            cx,
        );
        div()
            .id("slack-message-delete-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.render_slack_message_delete_dialog(cx))
            .into_any_element()
    }

    fn render_slack_message_delete_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let modal = self
            .slack_message_delete_modal
            .as_ref()
            .expect("Slack message delete layer requires modal state");
        div()
            .id("slack-message-delete-dialog")
            .role(Role::Dialog)
            .aria_label("Delete message")
            .relative()
            .w(px(SLACK_MESSAGE_DELETE_DIALOG_WIDTH))
            .h(px(SLACK_MESSAGE_DELETE_DIALOG_HEIGHT))
            .max_w(gpui::relative(0.92))
            .rounded(px(8.0))
            .bg(rgb(0x1a1d21))
            .shadow(slack_message_delete_shadow())
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_message_delete_header(modal.deleting, cx))
            .child(slack_message_delete_prompt())
            .child(self.render_slack_message_delete_preview())
            .when_some(modal.error.as_deref(), |this, error| {
                this.child(slack_message_delete_error(error))
            })
            .child(self.render_slack_message_delete_footer(modal.deleting, cx))
    }

    fn render_slack_message_delete_header(
        &self,
        deleting: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let close = slack_message_delete_close(cx);
        div()
            .absolute()
            .top(px(20.0))
            .left(px(SLACK_MESSAGE_DELETE_CONTENT_LEFT))
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
                    .child("Delete message"),
            )
            .child(if deleting {
                close
            } else {
                close
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(0xf8f8f8, 0.08)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_message_delete(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if slack_message_delete_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.close_slack_message_delete(cx);
                        }
                    }))
            })
    }

    fn render_slack_message_delete_preview(&self) -> impl IntoElement {
        let source = &self
            .slack_message_delete_modal
            .as_ref()
            .expect("Slack delete preview requires modal state")
            .source;
        div()
            .absolute()
            .top(px(120.0))
            .left(px(SLACK_MESSAGE_DELETE_CONTENT_LEFT))
            .w(px(SLACK_MESSAGE_DELETE_CONTENT_WIDTH))
            .h(px(50.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(0x35373b))
            .bg(rgb(0x222529))
            .px(px(12.0))
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(slack_message_delete_author(source.author.clone()))
            .child(slack_message_delete_body(source.preview.clone()))
            .child(slack_message_delete_timestamp(
                source.timestamp_label.clone(),
            ))
    }

    fn render_slack_message_delete_footer(
        &self,
        deleting: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .absolute()
            .right(px(SLACK_MESSAGE_DELETE_CONTENT_LEFT))
            .bottom(px(20.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.render_slack_message_delete_cancel_button(deleting, cx))
            .child(self.render_slack_message_delete_submit_button(deleting, cx))
    }

    fn render_slack_message_delete_cancel_button(
        &self,
        deleting: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = div()
            .id("slack-message-delete-cancel")
            .role(Role::Button)
            .aria_label("Cancel deleting message")
            .w(px(76.0))
            .h(px(36.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x8b8d8f))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if deleting { 0x9a9b9e } else { 0xf8f8f8 }))
            .child("Cancel");
        if deleting {
            button.into_any_element()
        } else {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(alpha(0xf8f8f8, 0.06)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_slack_message_delete(cx);
                }))
                .into_any_element()
        }
    }

    fn render_slack_message_delete_submit_button(
        &self,
        deleting: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = div()
            .id("slack-message-delete-submit")
            .role(Role::Button)
            .aria_label(if deleting {
                "Deleting message"
            } else {
                "Delete message"
            })
            .w(px(76.0))
            .h(px(36.0))
            .rounded(px(4.0))
            .bg(rgb(if deleting { 0x7d1a3b } else { 0xe01e5a }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if deleting { 0xd1d2d3 } else { 0xffffff }))
            .child(if deleting { "Deleting…" } else { "Delete" });
        if deleting {
            button.into_any_element()
        } else {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0xc9184f)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.submit_slack_message_delete(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if slack_message_delete_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.submit_slack_message_delete(cx);
                    }
                }))
                .into_any_element()
        }
    }
}

fn slack_message_delete_close(cx: &mut Context<SurfaceState>) -> gpui::Stateful<gpui::Div> {
    div()
        .id("slack-message-delete-close")
        .role(Role::Button)
        .aria_label("Close delete message dialog")
        .size(px(36.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(SlackShellIcon::Close, 0xb9babd, 24.0, cx))
}

fn slack_message_delete_author(author: gpui::SharedString) -> gpui::Div {
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
        .child(author)
}

fn slack_message_delete_body(body: gpui::SharedString) -> gpui::Div {
    div()
        .min_w(px(0.0))
        .flex_grow(1.0)
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(14.0))
        .line_height(px(20.0))
        .text_color(rgb(0xd1d2d3))
        .child(body)
}

fn slack_message_delete_timestamp(timestamp: gpui::SharedString) -> gpui::Div {
    div()
        .flex_none()
        .text_size(px(12.0))
        .line_height(px(18.0))
        .text_color(rgb(0x9a9b9e))
        .child(timestamp)
}

fn slack_message_delete_prompt() -> gpui::Div {
    div()
        .absolute()
        .top(px(72.0))
        .left(px(SLACK_MESSAGE_DELETE_CONTENT_LEFT))
        .w(px(SLACK_MESSAGE_DELETE_CONTENT_WIDTH))
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(0xd1d2d3))
        .child("Are you sure you want to delete this message? This cannot be undone.")
}

fn slack_message_delete_error(error: &str) -> gpui::Div {
    div()
        .absolute()
        .left(px(SLACK_MESSAGE_DELETE_CONTENT_LEFT))
        .bottom(px(62.0))
        .w(px(292.0))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(12.0))
        .line_height(px(18.0))
        .text_color(rgb(0xe01e5a))
        .child(error.to_string())
}

fn slack_message_delete_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0xe8e8e8, 0.13),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.35),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(48.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}

fn slack_message_delete_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
