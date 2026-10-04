use crate::ui::surface::{slack_palette, SlackShellIcon, SurfaceState};
use crate::ui::{
    alpha, div, point, px, rgb, AnyElement, AppearanceMode, BoxShadow, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled,
};
use crate::ui::{SlackChannelNotificationMode, SlackChannelNotificationPreference};
use gpui::{MouseMoveEvent, Role, Stateful};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

mod row;

use row::{slack_channel_notification_hover_icon, SlackChannelNotificationRowSpec};

const SLACK_CHANNEL_NOTIFICATIONS_MENU_TOP: f32 = 77.5;
const SLACK_CHANNEL_NOTIFICATIONS_MENU_RIGHT: f32 = 16.0;
const SLACK_CHANNEL_NOTIFICATIONS_MENU_WIDTH: f32 = 300.0;
const SLACK_CHANNEL_NOTIFICATIONS_MENU_HEIGHT: f32 = 250.0;

impl SurfaceState {
    pub(crate) fn render_slack_channel_notifications_menu_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_channel_notifications_menu(cx);
            }),
            cx,
        );
        div()
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _, cx| {
                if this.slack_channel_notifications_menu_keyboard_highlighted {
                    this.slack_channel_notifications_menu_keyboard_highlighted = false;
                    cx.notify();
                }
            }))
            .child(backdrop)
            .child(self.render_slack_channel_notifications_menu(cx))
            .into_any_element()
    }

    fn render_slack_channel_notifications_menu(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let preference = self
            .slack_channel_notification_preference
            .as_ref()
            .expect("Slack notification menu requires a loaded preference");
        let items = if self.slack_channel_notifications_advanced_open {
            self.render_slack_channel_notification_advanced_items(preference, cx)
        } else {
            self.render_slack_channel_notification_standard_items(preference, cx)
        };
        self.slack_channel_notifications_menu_shell(cx)
            .children(items)
    }

    fn slack_channel_notifications_menu_shell(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let background = match self.appearance_mode {
            AppearanceMode::Dark => 0x212428,
            AppearanceMode::Light => 0xffffff,
        };
        div()
            .id("slack-channel-notifications-menu")
            .role(Role::Menu)
            .aria_label("Notify you about")
            .track_focus(&self.slack_channel_notifications_menu_focus_handle)
            .focusable()
            .tab_stop(false)
            .absolute()
            .right(px(SLACK_CHANNEL_NOTIFICATIONS_MENU_RIGHT))
            .top(px(SLACK_CHANNEL_NOTIFICATIONS_MENU_TOP))
            .w(px(SLACK_CHANNEL_NOTIFICATIONS_MENU_WIDTH))
            .h(px(SLACK_CHANNEL_NOTIFICATIONS_MENU_HEIGHT))
            .rounded(px(4.0))
            .bg(rgb(background))
            .shadow(slack_channel_notifications_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.handle_slack_channel_notifications_menu_key(event, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
    }

    fn render_slack_channel_notification_standard_items(
        &self,
        preference: &SlackChannelNotificationPreference,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.slack_channel_notifications_menu_selected_index;
        let heading_color = if self.appearance_mode == AppearanceMode::Dark {
            alpha(0xe8e8e8, 0.7)
        } else {
            alpha(slack_palette(self.appearance_mode).main_secondary_text, 1.0)
        };
        vec![
            slack_channel_notification_heading("Notify you about…", heading_color)
                .into_any_element(),
            self.render_slack_channel_notification_row(
                SlackChannelNotificationRowSpec::everything(
                    preference.desktop_mode == SlackChannelNotificationMode::Everything
                        && !preference.muted,
                    selected == 0,
                ),
                cx,
            )
            .into_any_element(),
            self.render_slack_channel_notification_row(
                SlackChannelNotificationRowSpec::mentions(
                    preference.desktop_mode == SlackChannelNotificationMode::Mentions
                        && !preference.muted,
                    selected == 1,
                ),
                cx,
            )
            .into_any_element(),
            self.render_slack_channel_notification_row(
                SlackChannelNotificationRowSpec::muted(preference.muted, selected == 2),
                cx,
            )
            .into_any_element(),
            slack_channel_notification_separator().into_any_element(),
            self.render_slack_channel_notification_more_item(selected == 3, cx)
                .into_any_element(),
        ]
    }

    fn render_slack_channel_notification_advanced_items(
        &self,
        preference: &SlackChannelNotificationPreference,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_channel_notifications_menu_selected_index;
        vec![
            slack_channel_notification_heading(
                "Advanced settings",
                rgb(palette.main_secondary_text).into(),
            )
            .into_any_element(),
            self.render_slack_channel_notification_row(
                SlackChannelNotificationRowSpec::nothing(
                    preference.desktop_mode == SlackChannelNotificationMode::Nothing
                        && !preference.muted,
                    selected == 4,
                ),
                cx,
            )
            .into_any_element(),
            self.render_slack_channel_notification_back_item(palette.main_text, cx)
                .into_any_element(),
        ]
    }

    fn render_slack_channel_notification_more_item(
        &self,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let highlighted = self.slack_channel_notifications_menu_keyboard_highlighted && selected;
        let text_color = if highlighted {
            0xf8f8f8
        } else {
            slack_palette(self.appearance_mode).main_text
        };
        div()
            .id("slack-channel-notifications-more")
            .role(Role::MenuItem)
            .aria_label("More options")
            .h(px(28.0))
            .px(px(24.0))
            .group("slack-channel-notifications-more-hover")
            .cursor_pointer()
            .when(highlighted, |style| {
                style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff))
            })
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(28.0))
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_1()
                    .ml(px(8.0))
                    .text_color(rgb(text_color))
                    .group_hover("slack-channel-notifications-more-hover", |style| {
                        style.text_color(rgb(0xf8f8f8))
                    })
                    .child("More options"),
            )
            .child(slack_channel_notification_hover_icon(
                SlackShellIcon::ChevronRight,
                if highlighted { 0xf8f8f8 } else { 0xb9babd },
                15.0,
                "slack-channel-notifications-more-hover",
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.slack_channel_notifications_menu_selected_index = 3;
                this.activate_selected_slack_channel_notification(cx);
            }))
    }

    fn render_slack_channel_notification_back_item(
        &self,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-channel-notifications-back")
            .role(Role::MenuItem)
            .aria_label("Back")
            .h(px(28.0))
            .px(px(24.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(text_color))
            .child("Back")
            .on_click(cx.listener(|this, _, _, cx| {
                this.return_to_slack_channel_notifications_menu(cx);
            }))
    }

    fn handle_slack_channel_notifications_menu_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.keystroke.key.as_str() {
            "escape" if self.slack_channel_notifications_advanced_open => {
                self.return_to_slack_channel_notifications_menu(cx);
                true
            }
            "escape" => {
                self.close_slack_channel_notifications_menu(cx);
                true
            }
            "left" if self.slack_channel_notifications_advanced_open => {
                self.return_to_slack_channel_notifications_menu(cx);
                true
            }
            "right"
                if !self.slack_channel_notifications_advanced_open
                    && self.slack_channel_notifications_menu_selected_index == 3 =>
            {
                self.activate_selected_slack_channel_notification(cx);
                true
            }
            _ => self.handle_slack_channel_notifications_menu_action_key(event, cx),
        }
    }

    fn handle_slack_channel_notifications_menu_action_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.modifiers.modified() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "up" => self.move_slack_channel_notifications_menu_selection(-1, cx),
            "down" => self.move_slack_channel_notifications_menu_selection(1, cx),
            "home" => self.select_slack_channel_notification_menu_boundary(false, cx),
            "end" => self.select_slack_channel_notification_menu_boundary(true, cx),
            "enter" | "space" => self.activate_selected_slack_channel_notification(cx),
            _ => return false,
        }
        true
    }

    fn select_slack_channel_notification_menu_boundary(
        &mut self,
        end: bool,
        cx: &mut Context<Self>,
    ) {
        self.slack_channel_notifications_menu_selected_index =
            match (self.slack_channel_notifications_advanced_open, end) {
                (true, _) => 4,
                (false, false) => 0,
                (false, true) => 3,
            };
        self.slack_channel_notifications_menu_keyboard_highlighted = true;
        cx.notify();
    }
}

fn slack_channel_notification_heading(label: &'static str, color: gpui::Hsla) -> Div {
    div()
        .h(px(38.0))
        .px(px(24.0))
        .pt(px(12.0))
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(color)
        .child(label)
}

fn slack_channel_notification_separator() -> Div {
    div()
        .h(px(17.0))
        .flex()
        .items_center()
        .child(div().w_full().h(px(1.0)).bg(alpha(0xe8e8e8, 0.13)))
}

fn slack_channel_notifications_menu_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0xe8e8e8, 0.13),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.12),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}
