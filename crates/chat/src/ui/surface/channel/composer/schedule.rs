use super::{
    alpha, div, point, px, rgb, slack_icon, AnyElement, BoxShadow, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::{
    SlackScheduleCustomState, SlackScheduleNestedPicker, SlackScheduleOverlay,
};
use gpui::{ListSizingBehavior, Role};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

const SLACK_SCHEDULE_MENU_WIDTH: f32 = 260.0;
const SLACK_SCHEDULE_MENU_FIXED_HEIGHT: f32 = 86.187_5;
const SLACK_SCHEDULE_MENU_PRESET_HEIGHT: f32 = 31.0;
const SLACK_SCHEDULE_DIALOG_WIDTH: f32 = 520.0;
const SLACK_SCHEDULE_DIALOG_HEIGHT: f32 = 216.0;
const SLACK_SCHEDULE_DATE_PICKER_LEFT: f32 = -46.6;
const SLACK_SCHEDULE_DATE_PICKER_TOP: f32 = 119.0;
const SLACK_SCHEDULE_DATE_PICKER_WIDTH: f32 = 349.0;
const SLACK_SCHEDULE_DATE_PICKER_HEIGHT: f32 = 372.0;
const SLACK_SCHEDULE_CALENDAR_CELL_WIDTH: f32 = 43.0;
const SLACK_SCHEDULE_CALENDAR_CELL_HEIGHT: f32 = 41.0;
const SLACK_SCHEDULE_TIME_PICKER_LEFT: f32 = 306.4;
const SLACK_SCHEDULE_TIME_PICKER_TOP: f32 = 119.0;
const SLACK_SCHEDULE_TIME_PICKER_WIDTH: f32 = 195.6;
const SLACK_SCHEDULE_TIME_PICKER_HEIGHT: f32 = 264.0;
const SLACK_SCHEDULE_TIME_OPTION_HEIGHT: f32 = 28.0;
const SLACK_SCHEDULE_WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

mod actions;
mod calendar;
mod menu;
mod time_picker;

use actions::{slack_schedule_calendar_icon, slack_schedule_clock_icon};

#[derive(Clone, Copy)]
enum SlackScheduleMonthButtonDirection {
    Previous,
    Next,
}

#[derive(Clone, Copy)]
struct SlackScheduleMonthButtonSpec {
    element_id: &'static str,
    label: &'static str,
    glyph: &'static str,
    enabled: bool,
    direction: SlackScheduleMonthButtonDirection,
}

#[derive(Clone, Copy)]
struct SlackScheduleCalendarCellContext {
    index: usize,
    cursor_date: chrono::NaiveDate,
    right_edge: bool,
    bottom_edge: bool,
}

#[derive(Clone, Copy)]
struct SlackScheduleTimeOptionContext {
    index: usize,
    selected_index: Option<usize>,
    cursor_index: usize,
    option_count: usize,
}

impl SurfaceState {
    pub(crate) fn render_slack_schedule_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.slack_schedule_overlay.as_ref() {
            Some(overlay) => match &overlay.phase {
                SlackScheduleOverlay::Menu(menu) => {
                    self.render_slack_schedule_menu_layer(menu, overlay.anchor, cx)
                }
                SlackScheduleOverlay::Custom(custom) => {
                    self.render_slack_schedule_custom_layer(custom, cx)
                }
            },
            None => div().into_any_element(),
        }
    }

    fn render_slack_schedule_custom_layer(
        &self,
        custom: &SlackScheduleCustomState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.72))
                .cursor_pointer(),
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
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.render_slack_schedule_dialog(custom, cx))
            .into_any_element()
    }

    fn render_slack_schedule_dialog(
        &self,
        custom: &SlackScheduleCustomState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-schedule-dialog")
            .role(Role::Dialog)
            .aria_label("Schedule message")
            .relative()
            .top(px(8.5))
            .w(px(SLACK_SCHEDULE_DIALOG_WIDTH))
            .h(px(SLACK_SCHEDULE_DIALOG_HEIGHT))
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(0x34373b))
            .bg(rgb(0x1a1d21))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(14.0)),
                blur_radius: px(38.0),
                spread_radius: px(-10.0),
                inset: false,
            }])
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_schedule_dialog_header(custom, cx))
            .child(self.render_slack_schedule_dialog_date_control(custom, cx))
            .child(self.render_slack_schedule_dialog_time_control(custom, cx))
            .when_some(custom.error.as_deref(), |this, message| {
                this.child(
                    div()
                        .absolute()
                        .top(px(129.0))
                        .left(px(28.0))
                        .right(px(28.0))
                        .h(px(18.0))
                        .text_size(px(11.0))
                        .text_color(rgb(0xf2a6a2))
                        .child(message.to_string()),
                )
            })
            .child(self.render_slack_schedule_dialog_actions(cx))
            .when_some(custom.picker.as_ref(), |this, picker| {
                this.child(self.render_slack_schedule_nested_picker(custom, picker, cx))
            })
    }

    fn render_slack_schedule_dialog_header(
        &self,
        custom: &SlackScheduleCustomState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .absolute()
            .top(px(23.0))
            .left(px(28.0))
            .right(px(24.0))
            .h(px(48.0))
            .child(
                div()
                    .text_size(px(18.0))
                    .line_height(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child("Schedule message"),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .text_color(rgb(0xababad))
                    .child(format!("Time zone: {}", custom.timezone_label)),
            )
            .child(
                div()
                    .id("slack-schedule-close")
                    .role(Role::Button)
                    .aria_label("Close schedule message dialog")
                    .focusable()
                    .tab_stop(true)
                    .absolute()
                    .top(px(0.0))
                    .right(px(0.0))
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_schedule_overlay(cx);
                    }))
                    .child(slack_icon(SlackShellIcon::Close, 0xb9babd, 20.0, cx)),
            )
    }

    fn render_slack_schedule_dialog_date_control(
        &self,
        custom: &SlackScheduleCustomState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let picker_open = matches!(
            custom.picker.as_ref(),
            Some(SlackScheduleNestedPicker::Date(_))
        );
        let active = picker_open || custom.date_input_focused;
        div()
            .id("slack-schedule-date")
            .absolute()
            .top(px(87.0))
            .left(px(28.0))
            .w(px(278.4))
            .h(px(36.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x85888c))
            .when(active, |this| {
                this.border_color(rgb(0x1d9bd1)).shadow(vec![BoxShadow {
                    color: alpha(0x1d9bd1, 0.38),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(3.0),
                    inset: false,
                }])
            })
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(38.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_schedule_calendar_icon()),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .child(self.slack_schedule_date_input_entity(cx)),
            )
            .child(self.render_slack_schedule_date_toggle(picker_open, cx))
    }

    fn render_slack_schedule_date_toggle(
        &self,
        picker_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-schedule-date-toggle")
            .role(Role::Button)
            .aria_label("Open scheduled date picker")
            .aria_expanded(picker_open)
            .focusable()
            .tab_stop(true)
            .w(px(36.0))
            .h_full()
            .flex_none()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_custom_schedule_date_picker(cx);
            }))
            .on_key_down(cx.listener(Self::handle_slack_schedule_date_control_key_down))
            .child(slack_icon(SlackShellIcon::ChevronDown, 0xb9babd, 14.0, cx))
    }

    fn render_slack_schedule_dialog_time_control(
        &self,
        custom: &SlackScheduleCustomState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let picker_open = matches!(
            custom.picker.as_ref(),
            Some(SlackScheduleNestedPicker::Time(_))
        );
        let active = picker_open || custom.time_input_focused;
        div()
            .id("slack-schedule-time")
            .absolute()
            .top(px(87.0))
            .left(px(318.4))
            .w(px(173.6))
            .h(px(36.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x85888c))
            .when(active, |this| {
                this.border_color(rgb(0x1d9bd1)).shadow(vec![BoxShadow {
                    color: alpha(0x1d9bd1, 0.38),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(3.0),
                    inset: false,
                }])
            })
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(38.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_schedule_clock_icon()),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .child(self.slack_schedule_time_input_entity(cx)),
            )
            .child(self.render_slack_schedule_time_toggle(picker_open, cx))
    }

    fn render_slack_schedule_time_toggle(
        &self,
        picker_open: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-schedule-time-toggle")
            .role(Role::Button)
            .aria_label("Open scheduled time picker")
            .aria_expanded(picker_open)
            .focusable()
            .tab_stop(true)
            .w(px(36.0))
            .h_full()
            .flex_none()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_custom_schedule_time_picker(cx);
            }))
            .on_key_down(cx.listener(Self::handle_slack_schedule_time_control_key_down))
            .child(slack_icon(SlackShellIcon::ChevronDown, 0xb9babd, 14.0, cx))
    }

    fn render_slack_schedule_nested_picker(
        &self,
        custom: &SlackScheduleCustomState,
        picker: &SlackScheduleNestedPicker,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match picker {
            SlackScheduleNestedPicker::Date(picker) => {
                self.render_slack_schedule_date_picker(custom, picker, cx)
            }
            SlackScheduleNestedPicker::Time(picker) => {
                self.render_slack_schedule_time_picker(custom, picker, cx)
            }
        }
    }
}
