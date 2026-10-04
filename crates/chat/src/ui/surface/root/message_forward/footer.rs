use gpui::{
    div, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

use super::{slack_message_forward_action_key, SLACK_MESSAGE_FORWARD_CONTENT_LEFT};
use crate::ui::alpha;
use crate::ui::surface::SurfaceState;

impl SurfaceState {
    pub(super) fn render_slack_message_forward_footer(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward footer requires modal state");
        let forward_enabled = modal
            .destination
            .as_ref()
            .and_then(|destination| destination.conversation_id.as_ref())
            .is_some()
            && !modal.forwarding;
        div()
            .absolute()
            .left(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .right(px(SLACK_MESSAGE_FORWARD_CONTENT_LEFT))
            .bottom(px(20.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                if self.slack_workspace_api_capabilities.load_message_permalink {
                    self.render_slack_message_forward_copy_link_button(
                        modal.copying_link,
                        modal.link_copied,
                        cx,
                    )
                } else {
                    div().into_any_element()
                },
            )
            .child(self.render_slack_message_forward_submit_button(
                forward_enabled,
                modal.forwarding,
                cx,
            ))
    }

    fn render_slack_message_forward_copy_link_button(
        &self,
        copying: bool,
        copied: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = if copying {
            "Copying…"
        } else if copied {
            "Copied"
        } else {
            "Copy Link"
        };
        let button = slack_message_forward_footer_button(
            "slack-message-forward-copy-link",
            label,
            92.0,
            copying,
        );
        if copying {
            button.into_any_element()
        } else {
            button
                .cursor_pointer()
                .hover(|style| style.bg(alpha(0xf8f8f8, 0.06)))
                .focus_visible(|style| style.bg(alpha(0xf8f8f8, 0.06)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.copy_slack_message_forward_link(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if slack_message_forward_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.copy_slack_message_forward_link(cx);
                    }
                }))
                .into_any_element()
        }
    }

    fn render_slack_message_forward_submit_button(
        &self,
        enabled: bool,
        forwarding: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = if forwarding { "Sending…" } else { "Forward" };
        let button = div()
            .id("slack-message-forward-submit")
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(enabled)
            .w(px(80.0))
            .h(px(36.0))
            .rounded(px(4.0))
            .bg(rgb(if enabled { 0x007a5a } else { 0x2f3237 }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if enabled { 0xffffff } else { 0x9a9b9e }))
            .child(label);
        if enabled {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x148567)))
                .focus_visible(|style| style.bg(rgb(0x148567)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.submit_slack_message_forward(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if slack_message_forward_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.submit_slack_message_forward(cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }
}

fn slack_message_forward_footer_button(
    id: &'static str,
    label: &'static str,
    width: f32,
    disabled: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .w(px(width))
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
        .text_color(rgb(if disabled { 0x9a9b9e } else { 0xf8f8f8 }))
        .child(label)
}
