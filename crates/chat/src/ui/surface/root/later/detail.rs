use crate::ui::surface::{
    div, px, rgb, slack_icon, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackLaterRow, SlackLaterRowContent,
    SlackLaterThreadTarget, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{Role, SharedString};

use super::helpers::later_action_key;

mod reminder;

const SLACK_LATER_THREAD_HEADER_HEIGHT: f32 = 49.0;

impl SurfaceState {
    pub(super) fn render_slack_later_detail_pane(&self, cx: &mut Context<Self>) -> Div {
        let content = match self.selected_slack_later_row() {
            Some(row) => self.render_slack_later_detail(row, cx),
            None => self.render_slack_later_blank_detail(cx),
        };
        div()
            .w(px(0.0))
            .min_w(px(0.0))
            .h_full()
            .flex_grow(1.0)
            .child(content)
    }

    fn render_slack_later_blank_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(SlackShellIcon::Later, 0x6f2b78, 172.0, cx))
            .into_any_element()
    }

    fn render_slack_later_detail(&self, row: &SlackLaterRow, cx: &mut Context<Self>) -> AnyElement {
        match &row.content {
            SlackLaterRowContent::Placeholder { .. } => {
                self.render_slack_later_detail_loading(row, cx)
            }
            SlackLaterRowContent::HydrationError { .. } => {
                self.render_slack_later_detail_hydration_error(row, cx)
            }
            SlackLaterRowContent::Message { detail } => {
                self.render_slack_later_message_detail(row, detail, cx)
            }
            SlackLaterRowContent::File { attachment } => {
                self.render_slack_later_file_detail(attachment, row, cx)
            }
            SlackLaterRowContent::Reminder { detail } => {
                self.render_slack_later_reminder_detail(detail, cx)
            }
            SlackLaterRowContent::Tombstone | SlackLaterRowContent::Unsupported => {
                self.render_slack_later_unsupported_detail(row, cx)
            }
        }
    }

    fn render_slack_later_message_detail(
        &self,
        row: &SlackLaterRow,
        detail: &SlackLaterThreadTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.slack_workspace_api_capabilities.load_thread {
            return self.render_slack_later_saved_message_detail(detail, cx);
        }
        let panel = self.slack_thread_panel.as_ref().filter(|panel| {
            panel.origin.later_item_key() == Some(&row.key)
                && panel.conversation_id == detail.conversation_id
                && panel.parent_message_id == detail.thread_timestamp
        });
        match panel {
            Some(panel) if panel.pagination_initialized => {
                self.render_slack_later_thread(panel, cx)
            }
            Some(panel) => self.render_slack_later_thread_loading(
                Some(panel.conversation_name.clone().into()),
                cx,
            ),
            None => {
                self.render_slack_later_thread_loading(Some(detail.conversation_name.clone()), cx)
            }
        }
    }

    fn render_slack_later_saved_message_detail(
        &self,
        detail: &SlackLaterThreadTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header(
                "Saved item",
                Some(detail.conversation_name.clone()),
                cx,
            ))
            .child(
                div()
                    .id("slack-later-saved-message-scroll")
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .overflow_scroll()
                    .pt(px(16.0))
                    .child(self.render_slack_later_saved_message(&detail.selected_message_row, cx)),
            )
            .into_any_element()
    }

    fn render_slack_later_unsupported_detail(
        &self,
        row: &SlackLaterRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("Later", None, cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(15.0))
                    .text_color(rgb(0xb9babd))
                    .child(row.title.clone()),
            )
            .into_any_element()
    }

    pub(super) fn render_slack_later_detail_header(
        &self,
        title: &'static str,
        subtitle: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h(px(SLACK_LATER_THREAD_HEADER_HEIGHT))
            .flex_none()
            .pl(px(20.0))
            .pr(px(12.0))
            .border_b_1()
            .border_color(rgb(0x34363a))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_later_detail_heading(title, subtitle, cx))
            .child(self.render_slack_later_detail_close(cx))
    }

    fn render_slack_later_detail_heading(
        &self,
        title: &'static str,
        subtitle: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .when(title == "Thread", |this| {
                this.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(1.0))
                        .child(slack_icon(SlackShellIcon::HashSmall, 0xb9babd, 20.0, cx))
                        .child(slack_icon(SlackShellIcon::Forward, 0xb9babd, 12.0, cx)),
                )
            })
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(title),
            )
            .when_some(subtitle, |this, subtitle| {
                this.child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.0))
                        .text_color(rgb(0xb9babd))
                        .child(subtitle),
                )
            })
    }

    fn render_slack_later_detail_close(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-later-detail-close")
            .role(Role::Button)
            .aria_label("Close Later detail")
            .focusable()
            .tab_stop(true)
            .size(px(32.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2b2d31)))
            .focus_visible(|style| style.bg(rgb(0x2b2d31)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.close_slack_later_detail(cx);
            }))
            .on_key_down(cx.listener(|this, event, _, cx| {
                if later_action_key(event) {
                    cx.stop_propagation();
                    this.close_slack_later_detail(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(24.0))
            .text_color(rgb(0xb9babd))
            .child("×")
    }
}
