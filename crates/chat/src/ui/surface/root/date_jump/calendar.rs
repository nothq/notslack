use super::SlackDateJumpAppearance;
use crate::ui::surface::{
    SlackDateJumpCalendarCell, SlackDateJumpCalendarCellStatus, SlackDateJumpPickerState,
};
use gpui::{
    px, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Role,
    StatefulInteractiveElement, Styled,
};

use super::super::{div, slack_icon, FluentBuilder, SlackShellIcon, SurfaceState};

const SLACK_DATE_JUMP_CALENDAR_WIDTH: f32 = 302.0;
const SLACK_DATE_JUMP_CALENDAR_CELL_WIDTH: f32 = 43.0;
const SLACK_DATE_JUMP_CALENDAR_CELL_HEIGHT: f32 = 41.0;
const SLACK_DATE_JUMP_CALENDAR_BUTTON_WIDTH: f32 = 44.0;
const SLACK_DATE_JUMP_CALENDAR_BUTTON_HEIGHT: f32 = 42.0;
const SLACK_DATE_JUMP_WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

#[derive(Clone, Copy)]
struct SlackDateJumpNavigationSpec {
    id: &'static str,
    label: &'static str,
    icon: SlackShellIcon,
    icon_count: usize,
    month_delta: i32,
    enabled: bool,
}

#[derive(Clone, Copy)]
struct SlackDateJumpCellPresentation {
    selectable: bool,
    selected: bool,
    unavailable: bool,
    cursor: bool,
    date: Option<time::Date>,
}

#[derive(Clone, Copy)]
struct SlackDateJumpCalendarCellSpec {
    index: usize,
    cursor_date: time::Date,
    appearance: SlackDateJumpAppearance,
}

impl SurfaceState {
    pub(super) fn render_slack_date_jump_calendar(
        &self,
        picker: &SlackDateJumpPickerState,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let navigation = slack_date_jump_navigation_specs(picker);
        div()
            .w(px(SLACK_DATE_JUMP_CALENDAR_WIDTH))
            .mx_auto()
            .pt(px(8.0))
            .child(self.render_slack_date_jump_calendar_header(picker, navigation, appearance, cx))
            .child(self.render_slack_date_jump_calendar_grid(picker, appearance, cx))
    }

    fn render_slack_date_jump_calendar_grid(
        &self,
        picker: &SlackDateJumpPickerState,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut grid = div().w(px(SLACK_DATE_JUMP_CALENDAR_WIDTH));
        grid = grid.child(div().h(px(29.0)).flex().items_center().children(
            SLACK_DATE_JUMP_WEEKDAYS.map(|weekday| {
                div()
                    .w(px(SLACK_DATE_JUMP_CALENDAR_CELL_WIDTH))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(appearance.weekday_text)
                    .child(weekday)
            }),
        ));
        for (row_index, row) in picker.cells.chunks(7).enumerate() {
            let mut calendar_row = div()
                .id(("slack-date-jump-week", row_index))
                .h(px(SLACK_DATE_JUMP_CALENDAR_CELL_HEIGHT))
                .flex();
            for (column_index, cell) in row.iter().enumerate() {
                calendar_row = calendar_row.child(self.render_slack_date_jump_calendar_cell(
                    cell,
                    SlackDateJumpCalendarCellSpec {
                        index: row_index * 7 + column_index,
                        cursor_date: picker.cursor_date,
                        appearance,
                    },
                    cx,
                ));
            }
            grid = grid.child(calendar_row);
        }
        grid
    }

    fn render_slack_date_jump_calendar_header(
        &self,
        picker: &SlackDateJumpPickerState,
        navigation: [SlackDateJumpNavigationSpec; 4],
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .h(px(48.0))
            .mb(px(0.0))
            .flex()
            .items_start()
            .child(self.render_slack_date_jump_navigation_button(navigation[0], appearance, cx))
            .child(self.render_slack_date_jump_navigation_button(navigation[1], appearance, cx))
            .child(
                div()
                    .h(px(32.0))
                    .flex_grow(1.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(appearance.dialog_text)
                    .child(picker.month_label.clone()),
            )
            .child(self.render_slack_date_jump_navigation_button(navigation[2], appearance, cx))
            .child(self.render_slack_date_jump_navigation_button(navigation[3], appearance, cx))
    }

    fn render_slack_date_jump_navigation_button(
        &self,
        spec: SlackDateJumpNavigationSpec,
        appearance: SlackDateJumpAppearance,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(spec.id)
            .role(Role::Button)
            .aria_label(spec.label)
            .when(spec.enabled, |this| this.focusable().tab_stop(true))
            .size(px(32.0))
            .flex_none()
            .rounded(px(4.0))
            .opacity(if spec.enabled {
                1.0
            } else {
                appearance.disabled_control_opacity
            })
            .when(spec.enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(appearance.control_hover_background))
                    .focus_visible(|style| style.bg(appearance.control_focus_background))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shift_slack_date_jump_picker_month(spec.month_delta, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                            || event.keystroke.modifiers.modified()
                        {
                            return;
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        this.shift_slack_date_jump_picker_month(spec.month_delta, cx);
                    }))
            })
            .flex()
            .items_center()
            .justify_center()
            .children((0..spec.icon_count).map(|index| {
                div()
                    .ml(px(if index == 0 { 0.0 } else { -8.0 }))
                    .size(px(20.0))
                    .child(slack_icon(
                        spec.icon,
                        if spec.enabled {
                            appearance.control_icon
                        } else {
                            appearance.disabled_control_icon
                        },
                        20.0,
                        cx,
                    ))
            }))
    }

    fn render_slack_date_jump_calendar_cell(
        &self,
        cell: &SlackDateJumpCalendarCell,
        spec: SlackDateJumpCalendarCellSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let presentation = slack_date_jump_cell_presentation(cell, spec.cursor_date);
        self.slack_date_jump_calendar_cell_button(cell, spec, presentation, cx)
            .w(px(SLACK_DATE_JUMP_CALENDAR_BUTTON_WIDTH))
            .h(px(SLACK_DATE_JUMP_CALENDAR_BUTTON_HEIGHT))
            .mr(px(-1.0))
            .mb(px(-1.0))
            .flex_none()
            .when(presentation.selected, |this| {
                this.bg(spec.appearance.today_background)
            })
            .when_some(
                presentation
                    .unavailable
                    .then_some(spec.appearance.unavailable_background)
                    .flatten(),
                |this, background| this.bg(background),
            )
            .flex()
            .items_center()
            .justify_center()
            .child(slack_date_jump_calendar_cell_content(
                cell,
                presentation,
                spec.appearance,
            ))
    }

    fn slack_date_jump_calendar_cell_button(
        &self,
        cell: &SlackDateJumpCalendarCell,
        spec: SlackDateJumpCalendarCellSpec,
        presentation: SlackDateJumpCellPresentation,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(("slack-date-jump-calendar-cell", spec.index))
            .when(presentation.date.is_some(), |this| {
                this.role(Role::Button)
                    .aria_label(cell.accessibility_label.clone())
                    .border_1()
                    .border_color(if presentation.selected {
                        spec.appearance.today_border
                    } else {
                        spec.appearance.calendar_border
                    })
                    .when(presentation.selectable, |this| {
                        this.focusable()
                            .tab_stop(presentation.cursor)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(date) = presentation.date {
                                    this.select_slack_date_jump_calendar_date(date, cx);
                                }
                            }))
                            .on_key_down(cx.listener(
                                move |this, event: &KeyDownEvent, window, cx| {
                                    if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                                        || event.keystroke.modifiers.modified()
                                    {
                                        return;
                                    }
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    this.activate_slack_date_jump_picker_cursor(cx);
                                },
                            ))
                    })
            })
    }
}

fn slack_date_jump_cell_presentation(
    cell: &SlackDateJumpCalendarCell,
    cursor_date: time::Date,
) -> SlackDateJumpCellPresentation {
    SlackDateJumpCellPresentation {
        selectable: cell.status.is_selectable(),
        selected: cell.status == SlackDateJumpCalendarCellStatus::Today,
        unavailable: cell.status == SlackDateJumpCalendarCellStatus::Unavailable,
        cursor: cell.date == Some(cursor_date),
        date: cell.date,
    }
}

fn slack_date_jump_calendar_cell_content(
    cell: &SlackDateJumpCalendarCell,
    presentation: SlackDateJumpCellPresentation,
    appearance: SlackDateJumpAppearance,
) -> gpui::Div {
    div()
        .size(px(32.0))
        .rounded_full()
        .when(presentation.selected, |this| {
            this.border_1().border_color(appearance.today_text)
        })
        .when(presentation.cursor && !presentation.selected, |this| {
            this.border_1().border_color(appearance.today_text)
        })
        .when(presentation.selectable && !presentation.selected, |this| {
            this.hover(|style| style.bg(appearance.calendar_hover_background))
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(15.0))
        .line_height(px(20.0))
        .font_weight(FontWeight::NORMAL)
        .text_color(if presentation.selected {
            appearance.today_text
        } else if presentation.selectable {
            appearance.calendar_text
        } else {
            appearance.unavailable_text
        })
        .child(cell.day_label.clone())
}

fn slack_date_jump_navigation_specs(
    picker: &SlackDateJumpPickerState,
) -> [SlackDateJumpNavigationSpec; 4] {
    [
        SlackDateJumpNavigationSpec {
            id: "slack-date-jump-previous-year",
            label: "Previous year",
            icon: SlackShellIcon::ChevronLeft,
            icon_count: 2,
            month_delta: -12,
            enabled: picker.can_show_previous_year,
        },
        SlackDateJumpNavigationSpec {
            id: "slack-date-jump-previous-month",
            label: "Previous month",
            icon: SlackShellIcon::ChevronLeft,
            icon_count: 1,
            month_delta: -1,
            enabled: picker.can_show_previous_month,
        },
        SlackDateJumpNavigationSpec {
            id: "slack-date-jump-next-month",
            label: "Next month",
            icon: SlackShellIcon::ChevronRight,
            icon_count: 1,
            month_delta: 1,
            enabled: picker.can_show_next_month,
        },
        SlackDateJumpNavigationSpec {
            id: "slack-date-jump-next-year",
            label: "Next year",
            icon: SlackShellIcon::ChevronRight,
            icon_count: 2,
            month_delta: 12,
            enabled: picker.can_show_next_year,
        },
    ]
}
