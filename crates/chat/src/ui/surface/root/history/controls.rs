use super::super::super::{
    alpha, div, px, rgb, slack_icon, AnyElement, Context, Div, FluentBuilder, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use super::slack_history_action_key;
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_history_slackbot_button(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.slack_slackbot_control_available() {
            return div().size(px(28.0)).into_any_element();
        }
        div()
            .id("slack-history-slackbot")
            .role(Role::Button)
            .aria_label("Open Slackbot")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(0x3f4143))
            .bg(rgb(0x161616))
            .cursor_pointer()
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            .hover(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_history_slackbot_icon(cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.open_slack_slackbot(cx);
                }),
            )
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_history_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_slackbot(cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn render_slack_history_help_button(&self) -> Div {
        div().size(px(26.0))
    }

    pub(super) fn render_slack_history_nav_buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(self.render_slack_history_back_button(cx))
            .child(self.render_slack_history_forward_button(cx))
    }

    fn render_slack_history_back_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.can_navigate_slack_back();
        self.render_slack_top_nav_button(SlackShellIcon::Back, enabled, cx)
            .id("slack-history-back")
            .role(Role::Button)
            .aria_label("Back in history")
            .when(enabled, |this| {
                this.focusable()
                    .tab_stop(true)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigate_slack_history_back(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_history_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.navigate_slack_history_back(cx);
                        }
                    }))
            })
    }

    fn render_slack_history_forward_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.can_navigate_slack_forward();
        self.render_slack_top_nav_button(SlackShellIcon::Forward, enabled, cx)
            .id("slack-history-forward")
            .role(Role::Button)
            .aria_label("Forward in history")
            .when(enabled, |this| {
                this.focusable()
                    .tab_stop(true)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigate_slack_history_forward(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_history_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.navigate_slack_history_forward(cx);
                        }
                    }))
            })
    }

    pub(super) fn render_slack_history_menu_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = !self.slack_conversation_history.is_empty();
        self.render_slack_top_nav_button(SlackShellIcon::History, enabled, cx)
            .id("slack-show-history")
            .role(Role::Button)
            .aria_label("Show history")
            .aria_expanded(self.slack_history_menu_open)
            .when(enabled, |this| {
                this.focusable()
                    .tab_stop(true)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_slack_history_menu(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_history_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle_slack_history_menu(cx);
                        }
                    }))
            })
    }

    fn toggle_slack_history_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_history_menu_open {
            self.close_slack_history_menu(cx);
        } else {
            self.open_slack_history_menu(cx);
        }
    }

    pub(super) fn render_slack_top_nav_button(
        &self,
        icon: SlackShellIcon,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .size(px(26.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .when(enabled, |this| this.cursor_pointer())
            .when(enabled, |this| {
                this.focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            })
            .child(slack_icon(
                icon,
                if enabled { 0xf8f8f8 } else { 0x777777 },
                20.0,
                cx,
            ))
    }
}

fn slack_history_slackbot_icon(cx: &mut Context<SurfaceState>) -> Div {
    div()
        .size(px(20.0))
        .rounded(px(4.0))
        .overflow_hidden()
        .child(slack_icon(SlackShellIcon::Slackbot, 0, 20.0, cx))
}
