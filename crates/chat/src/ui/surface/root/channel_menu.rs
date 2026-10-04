use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled,
};
use crate::ui::surface::{
    SlackChannelMenuAction, SlackChannelMenuSubmenu, SlackChannelSubmenuAction, SurfaceState,
};
use crate::ui::AppearanceMode;
use gpui::{point, BoxShadow, Role};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

const SLACK_CHANNEL_MENU_TOP: f32 = 73.5;
const SLACK_CHANNEL_MENU_RIGHT: f32 = 16.0;
const SLACK_CHANNEL_MENU_WIDTH: f32 = 300.0;
const SLACK_CHANNEL_MENU_ROW_HEIGHT: f32 = 28.0;
const SLACK_CHANNEL_SUBMENU_OVERLAP: f32 = 2.0;

impl SurfaceState {
    pub(super) fn render_slack_channel_menu_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let menu_background = slack_channel_menu_background(self.appearance_mode);
        let actions = self.slack_channel_menu_actions();
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_channel_menu(cx);
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
            .child(
                slack_channel_menu_box(
                    menu_background,
                    SLACK_CHANNEL_MENU_RIGHT,
                    SLACK_CHANNEL_MENU_TOP,
                    SLACK_CHANNEL_MENU_WIDTH,
                )
                .id("slack-channel-menu")
                .role(Role::Menu)
                .track_focus(&self.slack_channel_menu_focus_handle)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                    }),
                )
                .children(
                    actions.iter().enumerate().map(|(index, action)| {
                        self.render_slack_channel_menu_item(index, action, cx)
                    }),
                ),
            )
            .when_some(self.slack_channel_menu_submenu, |this, submenu| {
                this.child(self.render_slack_channel_submenu(submenu, menu_background, cx))
            })
            .into_any_element()
    }

    fn render_slack_channel_menu_item(
        &self,
        index: usize,
        action: SlackChannelMenuAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_channel_menu_selected_index == Some(index);
        let text_color = if selected {
            0xffffff
        } else {
            palette.main_text
        };
        div()
            .id(action.element_id())
            .role(Role::MenuItem)
            .aria_label(action.label())
            .when(selected, |this| this.aria_active_descendant())
            .h(px(SLACK_CHANNEL_MENU_ROW_HEIGHT))
            .px(px(24.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(SLACK_CHANNEL_MENU_ROW_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(text_color))
            .when(selected, |this| this.bg(rgb(0x1264a3)))
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_slack_channel_menu_action(index, action, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_channel_menu_action(action, cx);
            }))
            .child(action.label())
            .when(action.submenu().is_some(), |this| {
                this.child(div().ml_auto().flex_none().child(slack_icon(
                    SlackShellIcon::ChevronRight,
                    text_color,
                    20.0,
                    cx,
                )))
            })
            .into_any_element()
    }

    fn render_slack_channel_submenu(
        &self,
        submenu: SlackChannelMenuSubmenu,
        menu_background: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let actions = self.slack_channel_submenu_actions(submenu);
        let submenu_right =
            SLACK_CHANNEL_MENU_RIGHT + SLACK_CHANNEL_MENU_WIDTH - SLACK_CHANNEL_SUBMENU_OVERLAP;
        let submenu_top =
            SLACK_CHANNEL_MENU_TOP + submenu.root_index() as f32 * SLACK_CHANNEL_MENU_ROW_HEIGHT;
        slack_channel_menu_box(menu_background, submenu_right, submenu_top, submenu.width())
            .id("slack-channel-submenu")
            .role(Role::Menu)
            .track_focus(&self.slack_channel_submenu_focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .children(
                actions.iter().enumerate().map(|(index, action)| {
                    self.render_slack_channel_submenu_item(index, action, cx)
                }),
            )
            .into_any_element()
    }

    fn render_slack_channel_submenu_item(
        &self,
        index: usize,
        action: SlackChannelSubmenuAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_channel_submenu_selected_index == Some(index);
        div()
            .id(action.element_id())
            .role(Role::MenuItem)
            .aria_label(action.label())
            .when(selected, |this| this.aria_active_descendant())
            .h(px(SLACK_CHANNEL_MENU_ROW_HEIGHT))
            .px(px(24.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(SLACK_CHANNEL_MENU_ROW_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(palette.main_text))
            .when(selected, |this| {
                this.bg(rgb(0x1264a3)).text_color(rgb(0xffffff))
            })
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_slack_channel_submenu_action(index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_channel_submenu_action(action, cx);
            }))
            .child(action.label())
            .into_any_element()
    }
}

fn slack_channel_menu_box(background: u32, right: f32, top: f32, width: f32) -> gpui::Div {
    div()
        .absolute()
        .right(px(right))
        .top(px(top))
        .w(px(width))
        .py(px(12.0))
        .rounded(px(4.0))
        .bg(rgb(background))
        .shadow(vec![
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
        ])
        .overflow_hidden()
}

fn slack_channel_menu_background(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Dark => 0x222529,
        AppearanceMode::Light => 0xffffff,
    }
}
