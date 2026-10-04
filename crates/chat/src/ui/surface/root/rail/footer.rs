use super::super::super::{
    alpha, div, px, rgb, slack_base_icon_radius, slack_icon, Context, Div, FluentBuilder,
    InteractiveElement, ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use super::slack_rail_action_key;
use crate::model::ChatSurfaceEvent;
use crate::ui::SlackWorkspace;
use app_model::AppearanceMode;
use gpui::{Role, Stateful};

impl SurfaceState {
    pub(super) fn render_slack_rail_footer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .px(px(17.0))
            .pb(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.0))
            .child(self.render_slack_rail_compose_button(cx))
            .child(self.render_slack_rail_theme_button(cx))
            .child(self.render_slack_rail_self_button(workspace, cx))
    }

    fn render_slack_rail_theme_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let (next_mode, label, icon) = match self.appearance_mode {
            AppearanceMode::Dark => (
                AppearanceMode::Light,
                "Switch to light mode",
                SlackShellIcon::ThemeMoon,
            ),
            AppearanceMode::Light => (
                AppearanceMode::Dark,
                "Switch to dark mode",
                SlackShellIcon::ThemeSun,
            ),
        };
        div()
            .id("slack-rail-theme")
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .rounded_full()
            .bg(alpha(0xf6f6f6, 0.25))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xf6f6f6, 0.35)))
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.35)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(ChatSurfaceEvent::AppearanceModeRequested(next_mode));
            }))
            .on_key_down(cx.listener(move |_, event, window, cx| {
                if slack_rail_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.emit(ChatSurfaceEvent::AppearanceModeRequested(next_mode));
                }
            }))
            .child(slack_icon(icon, 0xe5e5e5, 20.0, cx))
    }

    fn render_slack_rail_compose_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("slack-rail-create-new")
            .role(Role::Button)
            .aria_label("Create new")
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .rounded_full()
            .bg(alpha(0xf6f6f6, 0.25))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xf6f6f6, 0.35)))
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.35)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, _, cx| {
                this.activate_slack_new_message(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_rail_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_new_message(cx);
                }
            }))
            .child(slack_icon(SlackShellIcon::Plus, 0xe5e5e5, 20.0, cx))
    }

    fn render_slack_rail_self_button(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (_, user_id, _, _) = self.slack_self_identity();
        div()
            .id("slack-rail-self")
            .role(Role::Button)
            .aria_label(format!(
                "User: {}",
                workspace.self_display_name.as_deref().unwrap_or("You")
            ))
            .when(user_id.is_some(), |this| {
                this.focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            })
            .when(user_id.is_some(), |this| {
                this.on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_self_menu(cx);
                }))
            })
            .when(user_id.is_some(), |this| {
                this.on_key_down(cx.listener(move |this, event, window, cx| {
                    if slack_rail_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_self_menu(cx);
                    }
                }))
            })
            .size(px(36.0))
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(0x2e3136))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .child(self.render_slack_self_badge())
            .when_some(
                self.slack_presence_authority
                    .resolve_self_presence(workspace),
                |this, presence| {
                    this.child(self.render_slack_rail_self_presence(
                        presence,
                        workspace.rail_badges.self_notifications_paused,
                        cx,
                    ))
                },
            )
    }
}
