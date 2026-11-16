//! The menu under your avatar in the rail, laid out like Slack's: who you
//! are and your presence, a status box, presence and notification
//! controls, then your profile and account settings.

use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, BoxShadow, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Role, Stateful,
    StatefulInteractiveElement, Styled,
};

use crate::ui::surface::{
    slack_icon, SlackNotificationPausePreset, SlackShellIcon, SlackSurfaceAction, SurfaceState,
};
use crate::ui::{alpha, SlackUserPresence, SlackWorkspace};

const MENU_WIDTH: f32 = 300.0;
const SUBMENU_WIDTH: f32 = 260.0;
const SUBMENU_BOTTOM_OFFSET: f32 = 17.0 + 2.0 * 28.0 + 12.0 + 1.0;
const MENU_BG: u32 = 0x222529;
const MENU_BORDER: u32 = 0x3a3d42;
const MENU_TEXT: u32 = 0xf8f8f8;
const MENU_MUTED_TEXT: u32 = 0xababad;
const MENU_HOVER_BG: u32 = 0x1264a3;
const ACCOUNT_SETTINGS_URL: &str = "https://slack.com/account/settings";

impl SurfaceState {
    pub(super) fn render_slack_self_menu(
        &self,
        workspace: &SlackWorkspace,
        presence: Option<SlackUserPresence>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let away = presence == Some(SlackUserPresence::Away);
        let paused = self.slack_self_notifications_paused(workspace);
        slack_self_menu_container("slack-self-menu", MENU_WIDTH, cx)
            .left(px(62.0))
            .bottom(px(18.0))
            .child(self.slack_self_menu_identity(workspace, away))
            .child(self.slack_self_menu_status_box(cx))
            .child(
                self.slack_self_menu_row(
                    "slack-self-toggle-presence",
                    if away {
                        "Set yourself as active"
                    } else {
                        "Set yourself as away"
                    },
                    SurfaceState::toggle_slack_self_presence,
                    cx,
                )
                .child(
                    div().flex().child("Set yourself as\u{a0}").child(
                        div().font_weight(FontWeight::BOLD).child(if away {
                            "active"
                        } else {
                            "away"
                        }),
                    ),
                ),
            )
            .child(self.slack_self_menu_notifications_row(paused, cx))
            .child(slack_self_menu_divider())
            .child(
                self.slack_self_menu_row(
                    "slack-self-view-profile",
                    "Profile",
                    |this, cx| {
                        this.close_slack_self_menu(cx);
                        this.open_slack_self_panel(cx);
                    },
                    cx,
                )
                .child("Profile"),
            )
            .child(
                self.slack_self_menu_row(
                    "slack-self-account-settings",
                    "Account settings",
                    |this, cx| {
                        this.close_slack_self_menu(cx);
                        cx.open_url(ACCOUNT_SETTINGS_URL);
                    },
                    cx,
                )
                .justify_between()
                .child("Account settings")
                .child(slack_icon(
                    SlackShellIcon::ExternalLink,
                    MENU_MUTED_TEXT,
                    16.0,
                    cx,
                )),
            )
            .when_some(self.slack_self_settings_error.as_deref(), |this, error| {
                this.child(
                    div()
                        .px(px(16.0))
                        .pt(px(6.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
    }

    fn slack_self_menu_identity(&self, workspace: &SlackWorkspace, away: bool) -> Div {
        div()
            .px(px(16.0))
            .pt(px(4.0))
            .pb(px(10.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(self.render_slack_self_badge())
            .child(
                div()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .line_height(px(20.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MENU_TEXT))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(
                                workspace
                                    .self_display_name
                                    .clone()
                                    .unwrap_or_else(|| "You".to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(13.0))
                            .text_color(rgb(MENU_MUTED_TEXT))
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .when(away, |this| {
                                        this.border_1().border_color(rgb(MENU_MUTED_TEXT))
                                    })
                                    .when(!away, |this| this.bg(rgb(0x2bac76))),
                            )
                            .child(if away { "Away" } else { "Active" }),
                    ),
            )
    }

    /// The input-like box that opens the status dialog, showing the status
    /// when one is set.
    fn slack_self_menu_status_box(&self, cx: &mut Context<Self>) -> Div {
        let status = self.slack_self_status.as_ref();
        let emoji = status
            .map(|status| status.emoji().trim_matches(':').to_string())
            .filter(|name| !name.is_empty())
            .map(|name| {
                emojis::get_by_shortcode(&name)
                    .map_or_else(|| format!(":{name}:"), |emoji| emoji.as_str().to_string())
            });
        let text = status
            .map(|status| status.text().to_string())
            .filter(|text| !text.is_empty());
        let has_text = text.is_some();
        div().px(px(16.0)).py(px(6.0)).child(
            div()
                .id("slack-self-update-status")
                .role(Role::MenuItem)
                .aria_label("Update your status")
                .focusable()
                .tab_stop(true)
                .h(px(36.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(rgb(0x565856))
                .cursor_pointer()
                .hover(|style| style.border_color(rgb(0x8a8b8d)).bg(rgb(0x2c2f33)))
                .focus_visible(|style| style.border_color(rgb(0x1d9bd1)))
                .on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
                    this.set_slack_self_menu_notifications_open(false, cx);
                }))
                .on_click(cx.listener(|this, _, _, cx| this.open_slack_self_status_dialog(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if slack_menu_activation_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.open_slack_self_status_dialog(cx);
                    }
                }))
                .child(match emoji {
                    Some(emoji) => div().text_size(px(16.0)).child(emoji),
                    None => {
                        div().child(slack_icon(SlackShellIcon::Emoji, MENU_MUTED_TEXT, 18.0, cx))
                    }
                })
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(15.0))
                        .text_color(rgb(if has_text { MENU_TEXT } else { MENU_MUTED_TEXT }))
                        .child(text.unwrap_or_else(|| "Update your status".to_string())),
                ),
        )
    }

    fn slack_self_menu_notifications_row(&self, paused: bool, cx: &mut Context<Self>) -> Div {
        let open = self.slack_self_menu_notifications_open;
        let row = self
            .slack_self_menu_row(
                "slack-self-notifications",
                "Notifications",
                |this, cx| {
                    let open = !this.slack_self_menu_notifications_open;
                    this.set_slack_self_menu_notifications_open(open, cx);
                },
                cx,
            )
            .justify_between()
            .when(open, |this| this.bg(rgb(MENU_HOVER_BG)))
            .on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
                this.set_slack_self_menu_notifications_open(true, cx);
            }))
            .child("Notifications")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_color(rgb(if open { MENU_TEXT } else { MENU_MUTED_TEXT }))
                    .child(if paused { "Paused" } else { "On" })
                    .child(slack_icon(
                        SlackShellIcon::ChevronRight,
                        if open { MENU_TEXT } else { MENU_MUTED_TEXT },
                        14.0,
                        cx,
                    )),
            );
        div().relative().child(row).when(open, |this| {
            this.child(self.slack_self_menu_notifications_submenu(paused, cx))
        })
    }

    fn slack_self_menu_notifications_submenu(
        &self,
        paused: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        slack_self_menu_container("slack-self-notifications-menu", SUBMENU_WIDTH, cx)
            .left(px(MENU_WIDTH - 6.0))
            // Down to the menu's bottom edge, below the divider and the
            // last two rows, so it stays inside the window.
            .bottom(px(-SUBMENU_BOTTOM_OFFSET))
            .when(paused, |this| {
                this.child(
                    self.slack_self_menu_row(
                        "slack-self-resume-notifications",
                        "Resume notifications",
                        |this, cx| this.pause_slack_notifications(None, cx),
                        cx,
                    )
                    .child("Resume notifications"),
                )
                .child(slack_self_menu_divider())
            })
            .child(
                div()
                    .h(px(28.0))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .text_size(px(15.0))
                    .text_color(rgb(MENU_MUTED_TEXT))
                    .child("Pause notifications for…"),
            )
            .children(
                SlackNotificationPausePreset::ALL
                    .into_iter()
                    .map(|preset| self.slack_self_menu_pause_row(preset, cx)),
            )
    }

    fn slack_self_menu_pause_row(
        &self,
        preset: SlackNotificationPausePreset,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = preset.label();
        slack_self_menu_item(
            format!("slack-self-pause-{label}"),
            label,
            cx.listener(move |this, _, _, cx| this.pause_slack_notifications(Some(preset), cx)),
            cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_menu_activation_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.pause_slack_notifications(Some(preset), cx);
                }
            }),
        )
        .child(label)
    }

    /// A menu row that runs `action`; hovering it closes the submenu.
    fn slack_self_menu_row(
        &self,
        id: &'static str,
        label: &'static str,
        action: SlackSurfaceAction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let row = slack_self_menu_item(
            id.to_string(),
            label,
            cx.listener(move |this, _, _, cx| action(this, cx)),
            cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_menu_activation_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    action(this, cx);
                }
            }),
        );
        if id == "slack-self-notifications" {
            return row;
        }
        row.on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
            this.set_slack_self_menu_notifications_open(false, cx);
        }))
    }
}

fn slack_self_menu_item(
    id: String,
    label: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_key_down: impl Fn(&KeyDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::MenuItem)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .h(px(28.0))
        .px(px(16.0))
        .cursor_pointer()
        .flex()
        .items_center()
        .text_size(px(15.0))
        .text_color(rgb(MENU_TEXT))
        .hover(|style| style.bg(rgb(MENU_HOVER_BG)))
        .focus_visible(|style| style.bg(rgb(MENU_HOVER_BG)))
        .on_click(on_click)
        .on_key_down(on_key_down)
}

fn slack_self_menu_container(
    id: &'static str,
    width: f32,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Menu)
        .aria_label("Your account")
        .absolute()
        .w(px(width))
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(MENU_BORDER))
        .bg(rgb(MENU_BG))
        .shadow(vec![BoxShadow {
            color: alpha(0x000000, 0.42),
            offset: point(px(0.0), px(6.0)),
            blur_radius: px(22.0),
            spread_radius: px(0.0),
            inset: false,
        }])
        .py(px(12.0))
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_: &mut SurfaceState, _: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
            }),
        )
}

fn slack_self_menu_divider() -> Div {
    div().my(px(8.0)).h(px(1.0)).bg(rgb(MENU_BORDER))
}

fn slack_menu_activation_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
