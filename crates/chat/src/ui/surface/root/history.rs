use super::{
    alpha, div, px, relative, rgb, slack_icon, AnyElement, Context, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState, SLACK_HISTORY_HEIGHT,
    SLACK_TOP_NAV_LEFT_INSET, SLACK_TOP_NAV_RIGHT_INSET,
};
use crate::ui::{surface::SLACK_TOP_NAV_LEFT_FLEX_BASIS, SLACK_TOP_NAV_RAIL_WIDTH};
use gpui::{Role, Toggled};

mod controls;
mod menu;
mod search;

impl SurfaceState {
    pub(super) fn render_slack_history_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .relative()
            .h(px(SLACK_HISTORY_HEIGHT))
            .w_full()
            .pl(px(SLACK_TOP_NAV_RAIL_WIDTH + SLACK_TOP_NAV_LEFT_INSET))
            .pr(px(SLACK_TOP_NAV_RIGHT_INSET))
            .flex()
            .items_center()
            .min_w(px(0.0))
            .bg(rgb(0x0d0d0d))
            .child(self.render_slack_workspace_switcher_toggle(cx))
            .child(
                div()
                    .w(relative(SLACK_TOP_NAV_LEFT_FLEX_BASIS))
                    .flex_none()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(
                        div()
                            .pr(px(9.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(self.render_slack_history_nav_buttons(cx))
                            .child(self.render_slack_history_menu_button(cx)),
                    ),
            )
            .child(self.render_slack_history_search(cx))
            .child(
                div()
                    .flex_auto()
                    .flex_shrink_0()
                    .w(px(78.0))
                    .min_w(px(0.0))
                    .pl(px(8.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(self.render_slack_history_slackbot_button(cx))
                    .child(self.render_slack_history_help_button()),
            )
            .into_any_element()
    }

    fn render_slack_workspace_switcher_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.slack_workspace_switcher_expanded;
        let focus_handle = self.slack_workspace_switcher_focus_handle.clone();
        div()
            .id("slack-workspace-switcher-toggle")
            .role(Role::Button)
            .aria_label(if expanded {
                "Hide Workspace Switcher"
            } else {
                "Show Workspace Switcher"
            })
            .aria_toggled(if expanded {
                Toggled::True
            } else {
                Toggled::False
            })
            .absolute()
            .left(px(77.0))
            .top(px(7.0))
            .size(px(26.0))
            .rounded(px(4.0))
            .focusable()
            .track_focus(&self.slack_workspace_switcher_focus_handle)
            .tab_stop(true)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(alpha(0xffffff, 0.10)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.14)))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_handle, cx);
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_workspace_switcher(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if slack_history_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_workspace_switcher(cx);
                }
            }))
            .child(slack_icon(
                SlackShellIcon::WorkspaceSwitcher,
                0xb9babd,
                20.0,
                cx,
            ))
    }
}

pub(super) fn slack_history_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
