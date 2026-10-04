use super::super::super::{
    alpha, div, px, rgb, slack_empty_state_illustration, Context, Div, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use super::drafts_sent_action_key;
use crate::ui::SlackDraftsSentTab;
use gpui::Role;

type SlackDraftsSentEmptyContent = (
    &'static str,
    &'static str,
    &'static str,
    SlackShellIcon,
    f32,
);

impl SurfaceState {
    pub(super) fn render_slack_drafts_sent_empty(&self, cx: &mut Context<Self>) -> Div {
        let Some((title, body, action, illustration, illustration_width)) =
            drafts_sent_empty_content(self.slack_drafts_sent_tab)
        else {
            return div().flex_grow(1.0).min_h(px(0.0));
        };
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .bg(rgb(0x1a1d21))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(super::SLACK_DRAFTS_SENT_EMPTY_WIDTH))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(drafts_sent_empty_illustration(
                        illustration,
                        illustration_width,
                        cx,
                    ))
                    .child(drafts_sent_empty_title(title))
                    .child(drafts_sent_empty_body(body))
                    .child(self.render_slack_drafts_sent_new_message_button(action, cx)),
            )
    }

    fn render_slack_drafts_sent_new_message_button(
        &self,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(format!(
                "slack-drafts-sent-{}-new-message",
                self.slack_drafts_sent_tab.label().to_ascii_lowercase()
            ))
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .mt(px(16.0))
            .mb(px(8.0))
            .h(px(36.0))
            .px(px(12.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(0x797c81, 0.5))
            .bg(rgb(0x1a1d21))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x222529)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.compose_new_slack_message(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if drafts_sent_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.compose_new_slack_message(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(label)
    }

    pub(super) fn render_slack_drafts_sent_loading(&self) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .text_color(rgb(0xb9babd))
            .child("Loading…")
    }

    pub(super) fn render_slack_drafts_sent_error(
        &self,
        error: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(14.0))
            .text_color(rgb(0xd1d2d3))
            .child(error.to_string())
            .child(self.render_slack_drafts_sent_retry(cx))
    }

    fn render_slack_drafts_sent_retry(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-drafts-sent-retry")
            .role(Role::Button)
            .aria_label("Retry loading Drafts and sent messages")
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(4.0))
            .bg(rgb(0x611f69))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_drafts_sent(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if drafts_sent_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_drafts_sent(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child("Retry")
    }
}

fn drafts_sent_empty_content(tab: SlackDraftsSentTab) -> Option<SlackDraftsSentEmptyContent> {
    match tab {
        SlackDraftsSentTab::Drafts => Some((
            "Draft messages to send when you’re ready",
            "Start typing a message anywhere, then find it here. Re-read, revise, and send whenever you’d like.",
            "New Message",
            SlackShellIcon::EmptyDrafts,
            147.0,
        )),
        SlackDraftsSentTab::Scheduled => Some((
            "Write now, send later",
            "Schedule messages to be sent at a later time, or another day altogether. They’ll wait here until they’re delivered.",
            "Start New Message",
            SlackShellIcon::EmptyScheduled,
            145.18,
        )),
        SlackDraftsSentTab::Sent => None,
    }
}

fn drafts_sent_empty_illustration(
    illustration: SlackShellIcon,
    illustration_width: f32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .w_full()
        .h(px(140.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(slack_empty_state_illustration(
            illustration,
            illustration_width,
            140.0,
            cx,
        ))
}

fn drafts_sent_empty_title(title: &'static str) -> Div {
    div()
        .text_size(px(18.0))
        .line_height(px(24.0))
        .font_weight(FontWeight::BLACK)
        .text_color(rgb(0xd1d2d3))
        .child(title)
}

fn drafts_sent_empty_body(body: &'static str) -> Div {
    div()
        .mt(px(8.0))
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(0xd1d2d3))
        .child(body)
}
