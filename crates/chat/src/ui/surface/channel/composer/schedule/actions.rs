use super::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, ParentElement, Styled, SurfaceState,
};
use gpui::{Role, StatefulInteractiveElement};

impl SurfaceState {
    pub(super) fn render_slack_schedule_dialog_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .right(px(28.0))
            .bottom(px(24.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .id("slack-schedule-cancel")
                    .role(Role::Button)
                    .aria_label("Cancel scheduled message")
                    .focusable()
                    .tab_stop(true)
                    .w(px(80.0))
                    .h_full()
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(rgb(0x56595e))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_schedule_overlay(cx);
                    }))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("slack-schedule-submit")
                    .role(Role::Button)
                    .aria_label("Schedule message")
                    .focusable()
                    .tab_stop(true)
                    .w(px(147.0))
                    .h_full()
                    .rounded(px(4.0))
                    .bg(rgb(0x007a5a))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.submit_slack_custom_schedule(cx);
                    }))
                    .child("Schedule Message"),
            )
    }
}

pub(super) fn slack_schedule_calendar_icon() -> Div {
    div()
        .relative()
        .w(px(16.0))
        .h(px(15.0))
        .rounded(px(2.0))
        .border_1()
        .border_color(rgb(0xb9babd))
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .left(px(0.0))
                .right(px(0.0))
                .border_t_1()
                .border_color(rgb(0xb9babd)),
        )
        .child(
            div()
                .absolute()
                .left(px(4.0))
                .bottom(px(3.0))
                .size(px(2.0))
                .bg(rgb(0xb9babd)),
        )
        .child(
            div()
                .absolute()
                .right(px(4.0))
                .bottom(px(3.0))
                .size(px(2.0))
                .bg(rgb(0xb9babd)),
        )
}

pub(super) fn slack_schedule_clock_icon() -> Div {
    div()
        .relative()
        .size(px(16.0))
        .rounded_full()
        .border_1()
        .border_color(rgb(0xb9babd))
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .left(px(7.0))
                .w(px(1.0))
                .h(px(5.0))
                .bg(rgb(0xb9babd)),
        )
        .child(
            div()
                .absolute()
                .top(px(7.0))
                .left(px(7.0))
                .w(px(4.0))
                .h(px(1.0))
                .bg(rgb(0xb9babd)),
        )
}
