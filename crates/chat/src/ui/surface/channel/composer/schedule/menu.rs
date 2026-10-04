use super::{
    alpha, div, point, px, rgb, AnyElement, BoxShadow, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, SurfaceState, SLACK_SCHEDULE_MENU_FIXED_HEIGHT,
    SLACK_SCHEDULE_MENU_PRESET_HEIGHT, SLACK_SCHEDULE_MENU_WIDTH,
};
use crate::ui::surface::{SlackScheduleAnchor, SlackScheduleMenuState};
use gpui::Role;
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

const SLACK_SCHEDULE_MENU_EDGE_INSET: f32 = 8.0;
const SLACK_SCHEDULE_MENU_ANCHOR_GAP: f32 = 6.0;

impl SurfaceState {
    pub(super) fn render_slack_schedule_menu_layer(
        &self,
        menu: &SlackScheduleMenuState,
        anchor: SlackScheduleAnchor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let preset_height = menu
            .presets
            .iter()
            .flatten()
            .fold(0.0, |height, _| height + SLACK_SCHEDULE_MENU_PRESET_HEIGHT);
        let menu_height = SLACK_SCHEDULE_MENU_FIXED_HEIGHT + preset_height;
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.dismiss_slack_schedule_layer(cx);
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
            .child(self.render_slack_schedule_menu(menu, anchor, menu_height, cx))
            .into_any_element()
    }

    fn render_slack_schedule_menu(
        &self,
        menu: &SlackScheduleMenuState,
        anchor: SlackScheduleAnchor,
        menu_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (top, left) = self.slack_schedule_menu_position(anchor, menu_height);
        div()
            .id("slack-schedule-menu")
            .role(Role::Menu)
            .aria_label("Schedule message")
            .absolute()
            .top(px(top))
            .left(px(left))
            .w(px(SLACK_SCHEDULE_MENU_WIDTH))
            .h(px(menu_height))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x3a3d41))
            .bg(rgb(0x212428))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.42),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(24.0),
                spread_radius: px(-6.0),
                inset: false,
            }])
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_schedule_menu_title())
            .when_some(menu.presets[0].as_ref(), |this, preset| {
                this.child(self.render_slack_schedule_preset_row(&preset.label, 0, cx))
            })
            .when_some(menu.presets[1].as_ref(), |this, preset| {
                this.child(self.render_slack_schedule_preset_row(&preset.label, 1, cx))
            })
            .when_some(menu.presets[2].as_ref(), |this, preset| {
                this.child(self.render_slack_schedule_preset_row(&preset.label, 2, cx))
            })
            .child(self.render_slack_schedule_custom_time_row(cx))
    }

    fn slack_schedule_menu_position(
        &self,
        anchor: SlackScheduleAnchor,
        menu_height: f32,
    ) -> (f32, f32) {
        let maximum_left =
            (self.preview_width - SLACK_SCHEDULE_MENU_WIDTH - SLACK_SCHEDULE_MENU_EDGE_INSET)
                .max(SLACK_SCHEDULE_MENU_EDGE_INSET);
        let left = (anchor.x - SLACK_SCHEDULE_MENU_WIDTH)
            .clamp(SLACK_SCHEDULE_MENU_EDGE_INSET, maximum_left);
        let below = anchor.y + SLACK_SCHEDULE_MENU_ANCHOR_GAP;
        let above = anchor.y - menu_height - SLACK_SCHEDULE_MENU_ANCHOR_GAP;
        let maximum_top = (self.viewport_height - menu_height - SLACK_SCHEDULE_MENU_EDGE_INSET)
            .max(SLACK_SCHEDULE_MENU_EDGE_INSET);
        let top = if below + menu_height <= self.viewport_height {
            below
        } else {
            above
        }
        .clamp(SLACK_SCHEDULE_MENU_EDGE_INSET, maximum_top);
        (top, left)
    }

    fn render_slack_schedule_menu_title(&self) -> Div {
        div()
            .h(px(38.0))
            .px(px(24.0))
            .flex()
            .items_center()
            .text_size(px(12.5))
            .text_color(rgb(0xababad))
            .child("Schedule message")
    }

    fn render_slack_schedule_custom_time_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-schedule-custom-time")
            .role(Role::MenuItem)
            .aria_label("Custom time")
            .focusable()
            .tab_stop(true)
            .h(px(48.187_5))
            .px(px(24.0))
            .border_t_1()
            .border_color(rgb(0x414449))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(0xf8f8f8))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_custom_schedule(cx);
            }))
            .child("Custom time")
    }

    fn render_slack_schedule_preset_row(
        &self,
        label: &str,
        preset_index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("slack-schedule-preset", preset_index))
            .role(Role::MenuItem)
            .aria_label(label.to_string())
            .focusable()
            .tab_stop(true)
            .h(px(31.0))
            .px(px(24.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(0xf8f8f8))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.schedule_slack_preset(preset_index, cx);
            }))
            .child(label.to_string())
    }
}
