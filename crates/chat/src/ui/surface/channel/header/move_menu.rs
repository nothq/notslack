use crate::ui::surface::{
    slack_icon, slack_palette, SlackHeaderControl, SlackShellIcon, SurfaceState,
};
use crate::ui::SlackWorkspace;
use crate::ui::{
    alpha, div, point, px, rgb, AnyElement, AppearanceMode, BoxShadow, Context, Div, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui::{Role, Stateful, Toggled};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::{controls::SlackHeaderSquareButtonSpec, slack_header_action_key};

const SLACK_CHANNEL_MOVE_MENU_SIDEBAR_OVERLAP: f32 = 93.0;
const SLACK_CHANNEL_MOVE_MENU_TOP: f32 = 88.0;
const SLACK_CHANNEL_MOVE_MENU_WIDTH: f32 = 280.0;
const SLACK_CHANNEL_MOVE_MENU_HEADING_HEIGHT: f32 = 38.0;
const SLACK_CHANNEL_MOVE_MENU_ROW_HEIGHT: f32 = 28.0;

impl SurfaceState {
    pub(super) fn render_slack_channel_move_button(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::ChannelMove);
        self.render_slack_header_square_button(
            SlackHeaderSquareButtonSpec {
                id: "slack-channel-move",
                label: "Move channel",
                icon: SlackShellIcon::HeaderStar,
                fill: if self.slack_active_channel_is_starred() {
                    0xe3a300
                } else {
                    palette.main_text
                },
                bordered: true,
            },
            &self.slack_header_move_focus_handle,
            tab_stop,
            cx,
        )
        .aria_expanded(self.slack_channel_move_menu_open)
        .on_click(cx.listener(|this, _, _, cx| {
            this.slack_header_roving_target = SlackHeaderControl::ChannelMove;
            this.toggle_slack_channel_move_menu(cx);
        }))
        .on_key_down(cx.listener(|this, event, window, cx| {
            if this.handle_slack_channel_header_roving_key(
                SlackHeaderControl::ChannelMove,
                event,
                window,
                cx,
            ) {
                return;
            }
            if !slack_header_action_key(event) {
                return;
            }
            window.prevent_default();
            cx.stop_propagation();
            this.toggle_slack_channel_move_menu(cx);
        }))
    }

    fn toggle_slack_channel_move_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_channel_move_menu_open {
            self.close_slack_channel_move_menu(cx);
        } else {
            self.open_slack_channel_move_menu(cx);
        }
    }

    fn open_slack_channel_move_menu(&mut self, cx: &mut Context<Self>) {
        let available = self
            .slack_workspace()
            .is_some_and(|workspace| self.slack_channel_move_menu_available(workspace));
        if !available {
            return;
        }
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.close_slack_channel_menu(cx);
        self.slack_channel_move_menu_open = true;
        self.slack_channel_move_menu_focus_pending = true;
        self.slack_header_move_focus_pending = false;
        cx.notify();
    }

    fn close_slack_channel_move_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_move_menu_open {
            return;
        }
        self.slack_channel_move_menu_open = false;
        self.slack_channel_move_menu_focus_pending = false;
        self.slack_header_move_focus_pending = true;
        cx.notify();
    }

    fn activate_slack_channel_move_star(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_move_menu_open {
            return;
        }
        self.slack_channel_move_menu_open = false;
        self.slack_channel_move_menu_focus_pending = false;
        self.slack_header_move_focus_pending = true;
        self.toggle_slack_active_channel_star(cx);
    }

    pub(crate) fn render_slack_channel_move_menu_layer(
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
                this.close_slack_channel_move_menu(cx);
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
            .child(backdrop)
            .child(self.render_slack_channel_move_menu(cx))
            .into_any_element()
    }

    fn render_slack_channel_move_menu(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let visual = slack_channel_move_menu_visual(self.appearance_mode);
        let starred = self.slack_active_channel_is_starred();
        self.slack_channel_move_menu_shell(visual.background, cx)
            .child(slack_channel_move_menu_heading(visual.heading))
            .child(self.render_slack_channel_move_starred_item(starred, visual.item_text, cx))
    }

    fn slack_channel_move_menu_shell(
        &self,
        background: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-channel-move-menu")
            .role(Role::Menu)
            .aria_label("Move to")
            .track_focus(&self.slack_channel_move_menu_focus_handle)
            .focusable()
            .tab_stop(false)
            .absolute()
            .left(px(self.slack_rail_width() + self.slack_sidebar_width()
                - SLACK_CHANNEL_MOVE_MENU_SIDEBAR_OVERLAP))
            .top(px(SLACK_CHANNEL_MOVE_MENU_TOP))
            .w(px(SLACK_CHANNEL_MOVE_MENU_WIDTH))
            .rounded(px(4.0))
            .bg(rgb(background))
            .shadow(slack_channel_move_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.handle_slack_channel_move_menu_key(event, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
    }

    fn render_slack_channel_move_starred_item(
        &self,
        starred: bool,
        item_text: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-channel-move-starred")
            .role(Role::MenuItemRadio)
            .aria_label("Starred")
            .aria_toggled(if starred {
                Toggled::True
            } else {
                Toggled::False
            })
            .aria_active_descendant()
            .h(px(SLACK_CHANNEL_MOVE_MENU_ROW_HEIGHT))
            .px(px(24.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.activate_slack_channel_move_star(cx);
            }))
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(15.0))
            .line_height(px(SLACK_CHANNEL_MOVE_MENU_ROW_HEIGHT))
            .text_color(rgb(item_text))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(slack_icon(SlackShellIcon::HeaderStar, item_text, 20.0, cx))
                    .child("Starred"),
            )
            .child(if starred { "✓" } else { "" })
    }

    fn handle_slack_channel_move_menu_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.keystroke.key.as_str() {
            "escape" => {
                self.close_slack_channel_move_menu(cx);
                true
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                self.activate_slack_channel_move_star(cx);
                true
            }
            "up" | "down" | "home" | "end" if !event.keystroke.modifiers.modified() => true,
            _ => false,
        }
    }
}

struct SlackChannelMoveMenuVisual {
    background: u32,
    heading: gpui::Hsla,
    item_text: u32,
}

fn slack_channel_move_menu_visual(appearance_mode: AppearanceMode) -> SlackChannelMoveMenuVisual {
    match appearance_mode {
        AppearanceMode::Dark => SlackChannelMoveMenuVisual {
            background: 0x222529,
            heading: alpha(0xe8e8e8, 0.7),
            item_text: 0xf8f8f8,
        },
        AppearanceMode::Light => SlackChannelMoveMenuVisual {
            background: 0xffffff,
            heading: alpha(0x1d1c1d, 0.7),
            item_text: 0x1d1c1d,
        },
    }
}

fn slack_channel_move_menu_heading(color: gpui::Hsla) -> Div {
    div()
        .h(px(SLACK_CHANNEL_MOVE_MENU_HEADING_HEIGHT))
        .pt(px(12.0))
        .child(
            div()
                .h(px(26.0))
                .px(px(24.0))
                .py(px(4.0))
                .text_size(px(13.0))
                .line_height(px(18.0))
                .font_weight(FontWeight::NORMAL)
                .text_color(color)
                .child("Move to…"),
        )
}

fn slack_channel_move_menu_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0x1d1c1d, 0.13),
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
