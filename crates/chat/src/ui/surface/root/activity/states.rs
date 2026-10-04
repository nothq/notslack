use super::activity_action_key;
use crate::ui::surface::{
    alpha, slack_activity_palette, slack_base_icon_radius, slack_palette, SurfaceState,
};
use gpui::{
    div, px, relative, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

impl SurfaceState {
    pub(super) fn render_slack_activity_skeleton(&self) -> AnyElement {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .size_full()
            .px(px(16.0))
            .children((0..4).map(|_| {
                div()
                    .h(px(104.0))
                    .mb(px(8.0))
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(alpha(palette.card_border, palette.card_border_alpha))
                    .bg(rgb(palette.card_bg))
                    .px(px(12.0))
                    .py(px(12.0))
                    .flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(slack_base_icon_radius(36.0))
                            .bg(alpha(palette.secondary_text, 0.15)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .pt(px(3.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .w(px(116.0))
                                    .h(px(11.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.secondary_text, 0.15)),
                            )
                            .child(
                                div()
                                    .w(relative(0.72))
                                    .h(px(10.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.secondary_text, 0.10)),
                            )
                            .child(
                                div()
                                    .w(relative(0.9))
                                    .h(px(10.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.secondary_text, 0.10)),
                            ),
                    )
            }))
            .into_any_element()
    }

    pub(super) fn render_slack_activity_empty(&self) -> AnyElement {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .text_color(rgb(palette.tertiary_text))
            .child(if self.slack_activity_unread_only {
                "No unread activity"
            } else {
                "No activity in this view"
            })
            .into_any_element()
    }

    pub(super) fn render_slack_activity_error(
        &self,
        message: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let activity_palette = slack_activity_palette(self.appearance_mode);
        div()
            .size_full()
            .px(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(13.0))
            .text_color(rgb(activity_palette.primary_text))
            .child(message.to_string())
            .child(
                div()
                    .id("slack-activity-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading Slack activity")
                    .focusable()
                    .tab_stop(true)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(8.0))
                    .bg(rgb(palette.link))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_activity(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if activity_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.retry_slack_activity(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Retry"),
            )
            .into_any_element()
    }
}
