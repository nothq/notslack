use crate::ui::surface::{slack_icon, slack_palette, SlackShellIcon, SurfaceState};
use gpui::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement, Role, Stateful, StatefulInteractiveElement, Styled,
};

use super::alpha;

impl SurfaceState {
    pub(super) fn render_slack_upgrade_card(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let (background, border, title, subtitle) = match self.appearance_mode {
            crate::ui::AppearanceMode::Dark => (0x414347, 0x5c5e62, 0xffffff, 0xd1d2d3),
            crate::ui::AppearanceMode::Light => {
                (0xffffff, palette.sidebar_border, 0x1d1c1d, 0x454245)
            }
        };
        div().h(px(70.0)).flex_none().px(px(8.0)).pt(px(8.0)).child(
            div()
                .h(px(62.0))
                .relative()
                .rounded(px(12.0))
                .border_1()
                .border_color(rgb(border))
                .bg(rgb(background))
                .flex()
                .items_center()
                .child(self.render_slack_upgrade_link(title, subtitle, cx))
                .child(self.render_slack_upgrade_dismiss(subtitle, cx)),
        )
    }

    fn render_slack_upgrade_link(
        &self,
        title: u32,
        subtitle: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-upgrade-card")
            .role(Role::Button)
            .aria_label("View Slack plans")
            .focusable()
            .tab_stop(true)
            .h_full()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .pl(px(12.0))
            .pr(px(44.0))
            .rounded(px(12.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.06)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .flex()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.open_slack_link("https://slack.com/pricing", cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if upgrade_card_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_link("https://slack.com/pricing", cx);
                }
            }))
            .child(slack_icon(SlackShellIcon::Upgrade, title, 20.0, cx))
            .child(slack_upgrade_card_copy(title, subtitle))
    }

    fn render_slack_upgrade_dismiss(&self, subtitle: u32, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("slack-upgrade-card-close")
            .role(Role::Button)
            .aria_label("Dismiss upgrade message")
            .focusable()
            .tab_stop(true)
            .absolute()
            .right(px(8.0))
            .top(px(5.0))
            .size(px(28.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.12)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.16)))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.slack_upgrade_card_dismissed = true;
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if upgrade_card_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.slack_upgrade_card_dismissed = true;
                    cx.notify();
                }
            }))
            .child(slack_icon(SlackShellIcon::Close, subtitle, 16.0, cx))
    }
}

fn upgrade_card_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}

fn slack_upgrade_card_copy(title: u32, subtitle: u32) -> Div {
    div()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(15.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(title))
                .child("Upgrade to Business+"),
        )
        .child(
            div()
                .text_size(px(13.0))
                .line_height(px(18.0))
                .text_color(rgb(subtitle))
                .child("Faster, smarter results"),
        )
}
