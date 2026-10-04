use super::SlackDateJumpAppearance;
use crate::ui::surface::SlackDateJumpPickerState;
use gpui::{
    point, px, AnyElement, BoxShadow, Context, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Role, StatefulInteractiveElement,
    Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::super::{alpha, div, slack_icon, SlackShellIcon, SurfaceState};

const SLACK_DATE_JUMP_DIALOG_WIDTH: f32 = 374.0;
const SLACK_DATE_JUMP_DIALOG_HEIGHT: f32 = 434.0;

impl SurfaceState {
    pub(super) fn render_slack_date_jump_picker_layer(
        &self,
        picker: &SlackDateJumpPickerState,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.7))
                .cursor_pointer(),
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
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.slack_date_jump_dialog(picker, appearance, cx))
            .into_any_element()
    }

    fn slack_date_jump_dialog(
        &self,
        picker: &SlackDateJumpPickerState,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-date-jump-dialog")
            .role(Role::Dialog)
            .aria_label("Jump to a specific date")
            .relative()
            .w(px(SLACK_DATE_JUMP_DIALOG_WIDTH))
            .h(px(SLACK_DATE_JUMP_DIALOG_HEIGHT))
            .rounded(px(8.0))
            .bg(appearance.dialog_background)
            .text_color(appearance.dialog_text)
            .shadow(slack_date_jump_dialog_shadow(appearance))
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_date_jump_picker_header(appearance, cx))
            .child(self.render_slack_date_jump_calendar(picker, appearance, cx))
    }

    fn render_slack_date_jump_picker_header(
        &self,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .relative()
            .h(px(70.0))
            .px(px(28.0))
            .flex()
            .items_center()
            .text_size(px(22.0))
            .line_height(px(30.0))
            .font_weight(FontWeight::BLACK)
            .text_color(appearance.dialog_text)
            .child("Jump to a specific date")
            .child(
                div()
                    .id("slack-date-jump-close")
                    .role(Role::Button)
                    .aria_label("Close date picker")
                    .focusable()
                    .tab_stop(true)
                    .absolute()
                    .right(px(20.0))
                    .top(px(17.0))
                    .size(px(36.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|style| style.bg(appearance.control_hover_background))
                    .focus_visible(|style| style.bg(appearance.control_focus_background))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_date_jump_overlay(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                            || event.keystroke.modifiers.modified()
                        {
                            return;
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        this.close_slack_date_jump_overlay(cx);
                    }))
                    .child(slack_icon(
                        SlackShellIcon::Close,
                        appearance.control_icon,
                        20.0,
                        cx,
                    )),
            )
    }
}

fn slack_date_jump_dialog_shadow(appearance: SlackDateJumpAppearance) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: appearance.dialog_outline,
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.35),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(48.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}
