use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, AnyElement, BoxShadow, Context, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Role,
    StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::surface::{
    slack_palette, SlackSelfStatusDialog, SlackSurfaceAction, SlackSurfaceActionButton,
    SurfaceState,
};
use crate::ui::{alpha, SlackUserPresence, SlackWorkspace};

impl SurfaceState {
    pub(super) fn render_slack_self_menu_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presence = self
            .slack_presence_authority
            .resolve_self_presence(workspace);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_self_menu(cx);
            }),
            cx,
        );
        div()
            .id("slack-self-menu-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .child(backdrop)
            .child(self.render_slack_self_menu(workspace, presence, cx))
            .into_any_element()
    }

    fn render_slack_self_menu(
        &self,
        workspace: &SlackWorkspace,
        presence: Option<SlackUserPresence>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.slack_self_menu_container(cx)
            .child(self.slack_self_menu_identity(workspace))
            .when_some(self.slack_self_status.as_ref(), |this, status| {
                this.child(
                    div()
                        .px(px(14.0))
                        .pb(px(6.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0xb9babd))
                        .child(format!("{} {}", status.emoji(), status.text())),
                )
            })
            .child(self.slack_self_menu_action(
                "slack-self-update-status",
                "Update your status",
                SurfaceState::open_slack_self_status_dialog,
                cx,
            ))
            .child(self.slack_self_menu_action(
                "slack-self-toggle-presence",
                match presence {
                    Some(SlackUserPresence::Away) => "Set yourself as active",
                    _ => "Set yourself as away",
                },
                SurfaceState::toggle_slack_self_presence,
                cx,
            ))
            .child(self.slack_self_menu_action(
                "slack-self-view-profile",
                "View profile",
                |this, cx| {
                    this.close_slack_self_menu(cx);
                    this.open_slack_self_panel(cx);
                },
                cx,
            ))
            .when_some(self.slack_self_settings_error.as_deref(), |this, error| {
                this.child(
                    div()
                        .px(px(14.0))
                        .pt(px(6.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
    }

    fn slack_self_menu_container(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-self-menu")
            .role(Role::Menu)
            .aria_label("Your account")
            .absolute()
            .left(px(62.0))
            .bottom(px(18.0))
            .w(px(274.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(0x3a3d42))
            .bg(rgb(0x222529))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.42),
                offset: point(px(0.0), px(6.0)),
                blur_radius: px(22.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .py(px(8.0))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
    }

    fn slack_self_menu_identity(&self, workspace: &SlackWorkspace) -> gpui::Div {
        div()
            .px(px(14.0))
            .pb(px(8.0))
            .text_size(px(15.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xf8f8f8))
            .child(
                workspace
                    .self_display_name
                    .clone()
                    .unwrap_or_else(|| "You".to_string()),
            )
    }

    fn slack_self_menu_action(
        &self,
        id: &'static str,
        label: &'static str,
        action: SlackSurfaceAction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .role(Role::MenuItem)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .h(px(36.0))
            .px(px(14.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(14.0))
            .text_color(rgb(0xf8f8f8))
            .hover(|style| style.bg(rgb(0x1264a3)))
            .focus_visible(|style| style.bg(rgb(0x1264a3)))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    action(this, cx);
                }
            }))
            .child(label)
    }

    pub(super) fn render_slack_self_status_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self
            .slack_self_status_dialog
            .as_ref()
            .expect("Slack status layer requires dialog state");
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_self_status_dialog(cx);
            }),
            cx,
        );
        div()
            .id("slack-self-status-layer")
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
            .child(self.render_slack_self_status_dialog(dialog, cx))
            .into_any_element()
    }

    fn render_slack_self_status_dialog(
        &self,
        dialog: &SlackSelfStatusDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        self.slack_self_status_dialog_container(&palette, cx)
            .child(self.render_slack_self_status_dialog_header(&palette, cx))
            .child(self.slack_self_status_text_input_entity(cx))
            .child(self.render_slack_self_status_fields(dialog, cx))
            .when_some(dialog.error.as_deref(), |this, error| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
            .child(self.render_slack_self_status_dialog_footer(cx))
    }

    fn slack_self_status_dialog_container(
        &self,
        palette: &crate::ui::surface::SlackPalette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-self-status-dialog")
            .role(Role::Dialog)
            .aria_label("Update your status")
            .relative()
            .w(px(500.0))
            .max_w(gpui::relative(0.92))
            .rounded(px(10.0))
            .bg(rgb(palette.main_bg))
            .border_1()
            .border_color(rgb(palette.main_border))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(28.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
    }

    fn render_slack_self_status_dialog_header(
        &self,
        palette: &crate::ui::surface::SlackPalette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Update your status"),
            )
            .child(self.slack_self_status_button(
                SlackSurfaceActionButton {
                    id: "slack-self-status-close",
                    label: "Close",
                    action: SurfaceState::close_slack_self_status_dialog,
                },
                false,
                cx,
            ))
    }

    fn render_slack_self_status_fields(
        &self,
        dialog: &SlackSelfStatusDialog,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .gap(px(10.0))
            .child(
                div()
                    .w(px(180.0))
                    .child(self.slack_self_status_emoji_input_entity(cx)),
            )
            .child(self.slack_self_status_button(
                SlackSurfaceActionButton {
                    id: "slack-self-status-expiration",
                    label: dialog.expiration.label(),
                    action: SurfaceState::cycle_slack_self_status_expiration,
                },
                false,
                cx,
            ))
    }

    fn render_slack_self_status_dialog_footer(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .pt(px(4.0))
            .flex()
            .justify_between()
            .child(self.slack_self_status_button(
                SlackSurfaceActionButton {
                    id: "slack-self-status-clear",
                    label: "Clear status",
                    action: SurfaceState::clear_slack_self_status,
                },
                false,
                cx,
            ))
            .child(self.slack_self_status_button(
                SlackSurfaceActionButton {
                    id: "slack-self-status-save",
                    label: "Save",
                    action: SurfaceState::submit_slack_self_status,
                },
                true,
                cx,
            ))
    }

    fn slack_self_status_button(
        &self,
        button: SlackSurfaceActionButton,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSurfaceActionButton { id, label, action } = button;
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .h(px(36.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(if primary {
                0x007a5a
            } else {
                palette.main_border
            }))
            .bg(rgb(if primary { 0x007a5a } else { palette.main_bg }))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(if primary { 0xffffff } else { palette.main_text }))
            .hover(move |style| style.bg(rgb(if primary { 0x148567 } else { 0x303337 })))
            .focus_visible(|style| style.border_color(rgb(0x1d9bd1)))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    action(this, cx);
                }
            }))
            .child(label)
    }
}
