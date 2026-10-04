use super::{alpha, div, AnyElement, Context, IntoElement, SurfaceState};
use crate::ui::surface::{AppearanceMode, SlackDateJumpOverlay};
use gpui::Hsla;

mod calendar;
mod menu;
mod picker;

const SLACK_DATE_JUMP_MENU_WIDTH: f32 = 300.0;
const SLACK_DATE_JUMP_MENU_HEIGHT: f32 = 207.0;
const SLACK_DATE_JUMP_MENU_EDGE_INSET: f32 = 4.0;

#[derive(Clone, Copy)]
struct SlackDateJumpAppearance {
    menu_background: Hsla,
    menu_text: Hsla,
    menu_header: Hsla,
    menu_outline: Hsla,
    menu_selected_background: Hsla,
    menu_selected_text: Hsla,
    dialog_background: Hsla,
    dialog_text: Hsla,
    dialog_outline: Hsla,
    control_icon: u32,
    disabled_control_icon: u32,
    disabled_control_opacity: f32,
    control_hover_background: Hsla,
    control_focus_background: Hsla,
    weekday_text: Hsla,
    calendar_border: Hsla,
    calendar_text: Hsla,
    unavailable_background: Option<Hsla>,
    unavailable_text: Hsla,
    today_background: Hsla,
    today_border: Hsla,
    today_text: Hsla,
    calendar_hover_background: Hsla,
}

impl SlackDateJumpAppearance {
    fn for_mode(appearance_mode: AppearanceMode) -> Self {
        match appearance_mode {
            AppearanceMode::Dark => Self {
                menu_background: alpha(0x212428, 1.0),
                menu_text: alpha(0xf8f8f8, 1.0),
                menu_header: alpha(0xe8e8e8, 0.7),
                menu_outline: alpha(0xe8e8e8, 0.13),
                menu_selected_background: alpha(0x1264a3, 1.0),
                menu_selected_text: alpha(0xffffff, 1.0),
                dialog_background: alpha(0x1a1d21, 1.0),
                dialog_text: alpha(0xf8f8f8, 1.0),
                dialog_outline: alpha(0xe8e8e8, 0.13),
                control_icon: 0xd1d2d3,
                disabled_control_icon: 0x565856,
                disabled_control_opacity: 0.34,
                control_hover_background: alpha(0x2b2d31, 1.0),
                control_focus_background: alpha(0x1264a3, 1.0),
                weekday_text: alpha(0xababad, 1.0),
                calendar_border: alpha(0x35373b, 1.0),
                calendar_text: alpha(0xe8e8e8, 0.9),
                unavailable_background: None,
                unavailable_text: alpha(0xe8e8e8, 0.5),
                today_background: alpha(0x1d9bd1, 0.2),
                today_border: alpha(0x1d9bd1, 0.3),
                today_text: alpha(0x1d9bd1, 1.0),
                calendar_hover_background: alpha(0x2b2d31, 1.0),
            },
            AppearanceMode::Light => Self {
                menu_background: alpha(0xffffff, 1.0),
                menu_text: alpha(0x1d1c1d, 1.0),
                menu_header: alpha(0x1d1c1d, 0.7),
                menu_outline: alpha(0x1d1c1d, 0.13),
                menu_selected_background: alpha(0x1264a3, 1.0),
                menu_selected_text: alpha(0xffffff, 1.0),
                dialog_background: alpha(0xffffff, 1.0),
                dialog_text: alpha(0x1d1c1d, 1.0),
                dialog_outline: alpha(0x1d1c1d, 0.13),
                control_icon: 0x1d1c1d,
                disabled_control_icon: 0x1d1c1d,
                disabled_control_opacity: 0.13,
                control_hover_background: alpha(0x1d1c1d, 0.06),
                control_focus_background: alpha(0x1264a3, 1.0),
                weekday_text: alpha(0x1d1c1d, 0.7),
                calendar_border: alpha(0xdddddd, 1.0),
                calendar_text: alpha(0x1d1c1d, 1.0),
                unavailable_background: Some(alpha(0xf8f8f8, 1.0)),
                unavailable_text: alpha(0x1d1c1d, 0.5),
                today_background: alpha(0x1d9bd1, 0.2),
                today_border: alpha(0x1d9bd1, 0.3),
                today_text: alpha(0x1264a3, 1.0),
                calendar_hover_background: alpha(0x1d1c1d, 0.06),
            },
        }
    }
}

impl SurfaceState {
    pub(super) fn render_slack_date_jump_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let appearance = SlackDateJumpAppearance::for_mode(self.appearance_mode);
        match self.slack_date_jump_overlay.as_ref() {
            Some(SlackDateJumpOverlay::Menu(menu)) => {
                self.render_slack_date_jump_menu_layer(menu, appearance, cx)
            }
            Some(SlackDateJumpOverlay::Picker(picker)) => {
                self.render_slack_date_jump_picker_layer(picker, appearance, cx)
            }
            None => div().into_any_element(),
        }
    }
}
