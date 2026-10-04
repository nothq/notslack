use super::{
    SlackDateJumpAppearance, SLACK_DATE_JUMP_MENU_EDGE_INSET, SLACK_DATE_JUMP_MENU_HEIGHT,
    SLACK_DATE_JUMP_MENU_WIDTH,
};
use crate::ui::surface::{SlackDateJumpMenuAction, SlackDateJumpMenuState};
use gpui::{
    point, px, AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::super::SurfaceState;
use crate::ui::{alpha, div, FluentBuilder};

#[derive(Clone, Copy)]
struct SlackDateJumpMenuActionSpec {
    action: SlackDateJumpMenuAction,
    index: usize,
    selected: bool,
    appearance: SlackDateJumpAppearance,
}

impl SurfaceState {
    pub(super) fn render_slack_date_jump_menu_layer(
        &self,
        menu: &SlackDateJumpMenuState,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (left, top) = self.slack_date_jump_menu_position(menu);
        let menu_element = self
            .slack_date_jump_menu_container(left, top, appearance, cx)
            .children(
                SlackDateJumpMenuAction::ALL
                    .into_iter()
                    .enumerate()
                    .flat_map(|(index, action)| {
                        let divider = (action == SlackDateJumpMenuAction::SpecificDate)
                            .then(|| slack_date_jump_menu_divider(appearance).into_any_element());
                        divider.into_iter().chain(std::iter::once(
                            self.render_slack_date_jump_menu_action(
                                SlackDateJumpMenuActionSpec {
                                    action,
                                    index,
                                    selected: menu.selected_index == Some(index),
                                    appearance,
                                },
                                cx,
                            )
                            .into_any_element(),
                        ))
                    }),
            );
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_date_jump_overlay(cx);
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
            .child(menu_element)
            .into_any_element()
    }

    fn slack_date_jump_menu_position(&self, menu: &SlackDateJumpMenuState) -> (f32, f32) {
        let max_left =
            (self.preview_width - SLACK_DATE_JUMP_MENU_WIDTH - SLACK_DATE_JUMP_MENU_EDGE_INSET)
                .max(SLACK_DATE_JUMP_MENU_EDGE_INSET);
        let max_top =
            (self.viewport_height - SLACK_DATE_JUMP_MENU_HEIGHT - SLACK_DATE_JUMP_MENU_EDGE_INSET)
                .max(SLACK_DATE_JUMP_MENU_EDGE_INSET);
        (
            menu.anchor_left
                .clamp(SLACK_DATE_JUMP_MENU_EDGE_INSET, max_left),
            menu.anchor_top
                .clamp(SLACK_DATE_JUMP_MENU_EDGE_INSET, max_top),
        )
    }

    fn slack_date_jump_menu_container(
        &self,
        left: f32,
        top: f32,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-date-jump-menu")
            .role(Role::Menu)
            .aria_label("Jump to")
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(SLACK_DATE_JUMP_MENU_WIDTH))
            .h(px(SLACK_DATE_JUMP_MENU_HEIGHT))
            .py(px(12.0))
            .rounded(px(4.0))
            .bg(appearance.menu_background)
            .text_color(appearance.menu_text)
            .shadow(slack_date_jump_menu_shadow(appearance))
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .h(px(26.0))
                    .px(px(24.0))
                    .py(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(appearance.menu_header)
                    .child("Jump to…"),
            )
    }

    fn render_slack_date_jump_menu_action(
        &self,
        spec: SlackDateJumpMenuActionSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("slack-date-jump-action", spec.index))
            .role(Role::MenuItem)
            .aria_label(spec.action.label())
            .h(px(28.0))
            .px(px(24.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(28.0))
            .font_weight(FontWeight::NORMAL)
            .text_color(spec.appearance.menu_text)
            .when(spec.selected, |this| {
                this.bg(spec.appearance.menu_selected_background)
                    .text_color(spec.appearance.menu_selected_text)
            })
            .hover(|style| {
                style
                    .bg(spec.appearance.menu_selected_background)
                    .text_color(spec.appearance.menu_selected_text)
            })
            .focus_visible(|style| {
                style
                    .bg(spec.appearance.menu_selected_background)
                    .text_color(spec.appearance.menu_selected_text)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_date_jump_menu_action(spec.action, cx);
            }))
            .child(spec.action.label())
    }
}

fn slack_date_jump_menu_divider(appearance: SlackDateJumpAppearance) -> gpui::Div {
    div()
        .h(px(17.0))
        .py(px(8.0))
        .child(div().mx(px(0.0)).h(px(1.0)).bg(appearance.menu_outline))
}

fn slack_date_jump_menu_shadow(appearance: SlackDateJumpAppearance) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: appearance.menu_outline,
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
