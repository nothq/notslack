use super::{
    alpha, div, point, px, rgb, AnyElement, BoxShadow, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackScheduleCalendarCellContext, SlackScheduleMonthButtonDirection,
    SlackScheduleMonthButtonSpec, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_SCHEDULE_CALENDAR_CELL_HEIGHT, SLACK_SCHEDULE_CALENDAR_CELL_WIDTH,
    SLACK_SCHEDULE_DATE_PICKER_HEIGHT, SLACK_SCHEDULE_DATE_PICKER_LEFT,
    SLACK_SCHEDULE_DATE_PICKER_TOP, SLACK_SCHEDULE_DATE_PICKER_WIDTH, SLACK_SCHEDULE_WEEKDAYS,
};
use crate::ui::surface::{
    SlackScheduleCalendarCell, SlackScheduleCalendarCellStatus, SlackScheduleCustomState,
    SlackScheduleDatePickerState,
};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_schedule_date_picker(
        &self,
        custom: &SlackScheduleCustomState,
        picker: &SlackScheduleDatePickerState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("slack-schedule-date-picker")
            .role(Role::Grid)
            .aria_label(format!("Choose a date, {}", picker.month_label.as_ref()))
            .aria_row_count(6)
            .aria_column_count(7)
            .focusable()
            .tab_stop(false)
            .absolute()
            .left(px(SLACK_SCHEDULE_DATE_PICKER_LEFT))
            .top(px(SLACK_SCHEDULE_DATE_PICKER_TOP))
            .w(px(SLACK_SCHEDULE_DATE_PICKER_WIDTH))
            .h(px(SLACK_SCHEDULE_DATE_PICKER_HEIGHT))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(0x34373b))
            .bg(rgb(0x1a1d21))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(12.0)),
                blur_radius: px(32.0),
                spread_radius: px(-8.0),
                inset: false,
            }])
            .occlude()
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(Self::handle_slack_schedule_picker_key_down))
            .child(
                div()
                    .absolute()
                    .top(px(16.0))
                    .left(px(16.0))
                    .w(px(317.0))
                    .h(px(340.0))
                    .child(self.render_slack_schedule_calendar_header(picker, cx))
                    .child(self.render_slack_schedule_calendar_weekdays())
                    .child(self.render_slack_schedule_calendar_grid(custom, picker, cx)),
            )
            .into_any_element()
    }

    fn render_slack_schedule_calendar_header(
        &self,
        picker: &SlackScheduleDatePickerState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h(px(48.0))
            .flex()
            .items_center()
            .child(self.render_slack_schedule_previous_calendar_navigation(picker, cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(picker.month_label.clone()),
            )
            .child(self.render_slack_schedule_next_calendar_navigation(picker, cx))
    }

    fn render_slack_schedule_previous_calendar_navigation(
        &self,
        picker: &SlackScheduleDatePickerState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w(px(72.0))
            .h_full()
            .flex()
            .items_center()
            .child(self.render_slack_schedule_disabled_year_button(
                "slack-schedule-previous-year",
                "Previous year unavailable",
                "«",
            ))
            .child(self.render_slack_schedule_month_button(
                SlackScheduleMonthButtonSpec {
                    element_id: "slack-schedule-previous-month",
                    label: "Previous month",
                    glyph: "‹",
                    enabled: picker.can_show_previous_month,
                    direction: SlackScheduleMonthButtonDirection::Previous,
                },
                cx,
            ))
    }

    fn render_slack_schedule_next_calendar_navigation(
        &self,
        picker: &SlackScheduleDatePickerState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w(px(72.0))
            .h_full()
            .flex()
            .items_center()
            .justify_end()
            .child(self.render_slack_schedule_month_button(
                SlackScheduleMonthButtonSpec {
                    element_id: "slack-schedule-next-month",
                    label: "Next month",
                    glyph: "›",
                    enabled: picker.can_show_next_month,
                    direction: SlackScheduleMonthButtonDirection::Next,
                },
                cx,
            ))
            .child(self.render_slack_schedule_disabled_year_button(
                "slack-schedule-next-year",
                "Next year unavailable",
                "»",
            ))
    }

    fn render_slack_schedule_month_button(
        &self,
        spec: SlackScheduleMonthButtonSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(spec.element_id)
            .role(Role::Button)
            .aria_label(if spec.enabled {
                spec.label.to_string()
            } else {
                format!("{}, unavailable", spec.label)
            })
            .focusable()
            .tab_stop(spec.enabled)
            .size(px(32.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(30.0))
            .line_height(px(30.0))
            .text_color(rgb(if spec.enabled { 0xf8f8f8 } else { 0x55575b }))
            .when(spec.enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(0x34373b)))
                    .focus_visible(|style| style.bg(rgb(0x34373b)))
                    .on_click(cx.listener(move |this, _, _, cx| match spec.direction {
                        SlackScheduleMonthButtonDirection::Previous => {
                            this.show_previous_slack_schedule_month(cx);
                        }
                        SlackScheduleMonthButtonDirection::Next => {
                            this.show_next_slack_schedule_month(cx);
                        }
                    }))
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space")
                                && !event.keystroke.modifiers.modified()
                            {
                                match spec.direction {
                                    SlackScheduleMonthButtonDirection::Previous => {
                                        this.show_previous_slack_schedule_month(cx);
                                    }
                                    SlackScheduleMonthButtonDirection::Next => {
                                        this.show_next_slack_schedule_month(cx);
                                    }
                                }
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        },
                    ))
            })
            .child(spec.glyph)
    }

    fn render_slack_schedule_disabled_year_button(
        &self,
        element_id: &'static str,
        label: &'static str,
        glyph: &'static str,
    ) -> impl IntoElement {
        div()
            .id(element_id)
            .role(Role::Button)
            .aria_label(label)
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(23.0))
            .text_color(rgb(0x55575b))
            .child(glyph)
    }

    fn render_slack_schedule_calendar_weekdays(&self) -> Div {
        div().h(px(32.0)).mx(px(8.0)).flex().children(
            SLACK_SCHEDULE_WEEKDAYS
                .into_iter()
                .enumerate()
                .map(|(column_index, label)| {
                    div()
                        .id(("slack-schedule-weekday", column_index))
                        .role(Role::ColumnHeader)
                        .aria_label(label)
                        .w(px(SLACK_SCHEDULE_CALENDAR_CELL_WIDTH))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xb9babd))
                }),
        )
    }

    fn render_slack_schedule_calendar_grid(
        &self,
        _custom: &SlackScheduleCustomState,
        picker: &SlackScheduleDatePickerState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .mx(px(8.0))
            .w(px(SLACK_SCHEDULE_CALENDAR_CELL_WIDTH * 7.0))
            .h(px(SLACK_SCHEDULE_CALENDAR_CELL_HEIGHT * 6.0))
            .flex()
            .flex_wrap()
            .children(picker.cells.iter().enumerate().map(|(cell_index, cell)| {
                let right_edge = cell_index % 7 == 6
                    || picker
                        .cells
                        .get(cell_index + 1)
                        .is_none_or(|next| next.status == SlackScheduleCalendarCellStatus::Blank);
                let bottom_edge = picker
                    .cells
                    .get(cell_index + 7)
                    .is_none_or(|next| next.status == SlackScheduleCalendarCellStatus::Blank);
                self.render_slack_schedule_calendar_cell(
                    cell,
                    SlackScheduleCalendarCellContext {
                        index: cell_index,
                        cursor_date: picker.cursor_date,
                        right_edge,
                        bottom_edge,
                    },
                    cx,
                )
            }))
    }

    fn render_slack_schedule_calendar_cell(
        &self,
        cell: &SlackScheduleCalendarCell,
        context: SlackScheduleCalendarCellContext,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = cell.status.is_selected();
        let base = div()
            .id(("slack-schedule-calendar-cell", context.index))
            .role(Role::GridCell)
            .aria_label(cell.accessibility_label.clone())
            .aria_selected(selected)
            .aria_row_index(context.index / 7 + 1)
            .aria_column_index(context.index % 7 + 1)
            .w(px(SLACK_SCHEDULE_CALENDAR_CELL_WIDTH))
            .h(px(SLACK_SCHEDULE_CALENDAR_CELL_HEIGHT))
            .flex_none();
        let interactive = self.bind_slack_schedule_calendar_cell(base, cell, context, cx);
        self.finish_slack_schedule_calendar_cell(interactive, cell, selected)
    }

    fn bind_slack_schedule_calendar_cell(
        &self,
        base: gpui::Stateful<Div>,
        cell: &SlackScheduleCalendarCell,
        context: SlackScheduleCalendarCellContext,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let selected = cell.status.is_selected();
        let cursor = cell.date == Some(context.cursor_date);
        base.when(
            cell.status != SlackScheduleCalendarCellStatus::Blank,
            |this| {
                this.border_t_1()
                    .border_l_1()
                    .when(context.right_edge, |this| this.border_r_1())
                    .when(context.bottom_edge, |this| this.border_b_1())
                    .border_color(rgb(0x3a3d41))
            },
        )
        .when(selected, |this| this.bg(rgb(0x1264a3)))
        .when(cursor && !selected, |this| {
            this.bg(rgb(0x34373b)).border_color(rgb(0x1d9bd1))
        })
        .when(cell.status.is_selectable(), |this| {
            this.focusable()
                .tab_stop(cursor)
                .cursor_pointer()
                .when(!selected, |this| {
                    this.hover(|style| style.bg(rgb(0x34373b)))
                        .focus_visible(|style| style.border_color(rgb(0x1d9bd1)))
                })
                .when_some(cell.date, |this, date| {
                    this.on_click(cx.listener(move |this, _, _, cx| {
                        this.select_slack_schedule_calendar_date(date, cx);
                    }))
                })
        })
    }

    fn finish_slack_schedule_calendar_cell(
        &self,
        cell_view: gpui::Stateful<Div>,
        cell: &SlackScheduleCalendarCell,
        selected: bool,
    ) -> gpui::Stateful<Div> {
        cell_view
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .text_color(rgb(match cell.status {
                SlackScheduleCalendarCellStatus::Blank => 0x1a1d21,
                SlackScheduleCalendarCellStatus::Unavailable => 0x77797d,
                SlackScheduleCalendarCellStatus::Available
                | SlackScheduleCalendarCellStatus::Selected => 0xf8f8f8,
            }))
            .when(selected, |this| {
                this.child(
                    div()
                        .size(px(32.0))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0xffffff))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(cell.day_label.clone()),
                )
            })
            .when(!selected, |this| this.child(cell.day_label.clone()))
    }
}
