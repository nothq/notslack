use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::SlackMessageMenuAction;
use gpui::{point, BoxShadow, Div, Role, Stateful};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

const SLACK_MESSAGE_MENU_WIDTH: f32 = 300.0;
const SLACK_MESSAGE_MENU_ROW_HEIGHT: f32 = 28.0;
const SLACK_MESSAGE_MENU_GROUP_GAP: f32 = 17.0;

#[derive(Clone, Copy)]
struct SlackMessageMenuColors {
    text: u32,
    secondary_text: u32,
    border: u32,
}

#[derive(Clone, Copy)]
struct SlackMessageMenuRowSpec {
    index: usize,
    action: SlackMessageMenuAction,
    selected: bool,
    colors: SlackMessageMenuColors,
}

impl SurfaceState {
    pub(super) fn render_slack_message_menu_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_message_menu.is_none() {
            return div().into_any_element();
        }
        let palette = slack_palette(self.appearance_mode);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_message_menu(cx);
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
            .child(self.render_slack_message_menu(
                SlackMessageMenuColors {
                    text: palette.main_text,
                    secondary_text: palette.main_secondary_text,
                    border: palette.main_border,
                },
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_message_menu(
        &self,
        colors: SlackMessageMenuColors,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let menu = self
            .slack_message_menu
            .as_ref()
            .expect("Slack message menu layer requires menu state");
        div()
            .id("slack-message-menu")
            .role(Role::Menu)
            .track_focus(&self.slack_message_menu_focus_handle)
            .absolute()
            .right(px(menu.right))
            .top(px(menu.top))
            .w(px(SLACK_MESSAGE_MENU_WIDTH))
            .py(px(12.0))
            .rounded(px(4.0))
            .bg(rgb(match self.appearance_mode {
                crate::ui::AppearanceMode::Dark => 0x222529,
                crate::ui::AppearanceMode::Light => 0xffffff,
            }))
            .shadow(slack_message_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .children(
                menu.actions
                    .iter()
                    .copied()
                    .enumerate()
                    .flat_map(|(index, action)| {
                        self.render_slack_message_menu_elements(index, action, colors, cx)
                    }),
            )
    }

    fn render_slack_message_menu_elements(
        &self,
        index: usize,
        action: SlackMessageMenuAction,
        colors: SlackMessageMenuColors,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let menu = self
            .slack_message_menu
            .as_ref()
            .expect("Slack message menu row requires menu state");
        let selected = menu.selected_index == Some(index);
        let group_gap =
            (index > 0 && menu.actions[index - 1].group() != action.group()).then(|| {
                div()
                    .h(px(SLACK_MESSAGE_MENU_GROUP_GAP))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .child(div().w_full().h(px(1.0)).bg(rgb(colors.border)))
                    .into_any_element()
            });
        group_gap
            .into_iter()
            .chain(std::iter::once(
                self.render_slack_message_menu_row(
                    SlackMessageMenuRowSpec {
                        index,
                        action,
                        selected,
                        colors,
                    },
                    cx,
                )
                .into_any_element(),
            ))
            .collect()
    }

    fn render_slack_message_menu_row(
        &self,
        spec: SlackMessageMenuRowSpec,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let text_color = if spec.selected {
            0xffffff
        } else if spec.action.danger() {
            0xde678a
        } else {
            spec.colors.text
        };
        let icon = spec.action.icon();
        div()
            .id(spec.action.element_id())
            .role(Role::MenuItem)
            .aria_label(spec.action.label())
            .when(spec.selected, |this| this.aria_active_descendant())
            .h(px(SLACK_MESSAGE_MENU_ROW_HEIGHT))
            .px(px(16.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(SLACK_MESSAGE_MENU_ROW_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(text_color))
            .when(spec.selected, |this| this.bg(rgb(0x1264a3)))
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_slack_message_menu_action(spec.index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_message_menu_action(spec.action, cx);
            }))
            .child(slack_message_menu_row_label(
                spec.action,
                icon,
                text_color,
                cx,
            ))
            .child(
                div()
                    .ml_auto()
                    .text_size(px(13.0))
                    .text_color(rgb(if spec.selected {
                        0xffffff
                    } else {
                        spec.colors.secondary_text
                    }))
                    .child(spec.action.shortcut()),
            )
    }
}

fn slack_message_menu_row_label(
    action: SlackMessageMenuAction,
    icon: Option<super::SlackShellIcon>,
    text_color: u32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .min_w(px(0.0))
        .flex()
        .items_center()
        .when_some(icon, |this, icon| {
            this.child(
                div()
                    .size(px(20.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(icon, text_color, 20.0, cx)),
            )
            .child(div().w(px(12.0)).flex_none())
        })
        .when(icon.is_none(), |this| this.pl(px(8.0)))
        .child(action.label())
}

fn slack_message_menu_shadow() -> Vec<BoxShadow> {
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
