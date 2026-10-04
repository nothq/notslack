use super::{
    slack_schedule_date_in_month, slack_schedule_month_start, Context, Datelike, Days, Months,
    ScrollStrategy, SlackScheduleCalendarDates, SlackScheduleCustomState,
    SlackScheduleMonthDirection, SlackScheduleNestedPicker, SurfaceState,
};

impl SurfaceState {
    pub(super) fn move_slack_schedule_month(
        &mut self,
        direction: SlackScheduleMonthDirection,
        cx: &mut Context<Self>,
    ) {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return;
        };
        let Some(SlackScheduleNestedPicker::Date(picker)) = custom.picker.as_mut() else {
            return;
        };
        if match direction {
            SlackScheduleMonthDirection::Previous => !picker.can_show_previous_month,
            SlackScheduleMonthDirection::Next => !picker.can_show_next_month,
        } {
            return;
        }
        let displayed_month = match direction {
            SlackScheduleMonthDirection::Previous => {
                picker.displayed_month.checked_sub_months(Months::new(1))
            }
            SlackScheduleMonthDirection::Next => {
                picker.displayed_month.checked_add_months(Months::new(1))
            }
        }
        .expect("bounded Slack schedule month navigation must remain representable");
        let cursor_date = slack_schedule_date_in_month(
            displayed_month,
            picker.cursor_date.day(),
            custom.opened_date,
            custom.maximum_date,
        );
        picker.rebuild(
            displayed_month,
            cursor_date,
            SlackScheduleCalendarDates {
                opened: custom.opened_date,
                maximum: custom.maximum_date,
                selected: custom.date,
            },
        );
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn move_slack_schedule_calendar_cursor(
        &mut self,
        day_delta: i64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return false;
        };
        let Some(SlackScheduleNestedPicker::Date(picker)) = custom.picker.as_mut() else {
            return false;
        };
        let next = if day_delta < 0 {
            picker
                .cursor_date
                .checked_sub_days(Days::new(day_delta.unsigned_abs()))
        } else {
            picker
                .cursor_date
                .checked_add_days(Days::new(day_delta.unsigned_abs()))
        }
        .expect("bounded Slack schedule calendar navigation must remain representable")
        .clamp(custom.opened_date, custom.maximum_date);
        let displayed_month = slack_schedule_month_start(next);
        if displayed_month != picker.displayed_month {
            picker.rebuild(
                displayed_month,
                next,
                SlackScheduleCalendarDates {
                    opened: custom.opened_date,
                    maximum: custom.maximum_date,
                    selected: custom.date,
                },
            );
            cx.notify();
        } else if next != picker.cursor_date {
            picker.cursor_date = next;
            cx.notify();
        }
        true
    }

    pub(in crate::ui::surface::state) fn select_slack_schedule_calendar_cursor(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let date = self
            .slack_schedule_overlay
            .as_ref()
            .and_then(|overlay| overlay.custom())
            .and_then(|custom| match custom {
                SlackScheduleCustomState {
                    picker: Some(SlackScheduleNestedPicker::Date(picker)),
                    ..
                } => Some(picker.cursor_date),
                _ => None,
            });
        if let Some(date) = date {
            self.select_slack_schedule_calendar_date(date, cx);
        }
    }

    pub(in crate::ui::surface::state) fn move_slack_schedule_time_cursor(
        &mut self,
        option_delta: i64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return false;
        };
        let Some(SlackScheduleNestedPicker::Time(picker)) = custom.picker.as_mut() else {
            return false;
        };
        let maximum_index = picker.options.len().saturating_sub(1);
        let option_offset = usize::try_from(option_delta.unsigned_abs())
            .expect("Slack schedule option movement must fit usize");
        let next = if option_delta < 0 {
            picker.cursor_index.saturating_sub(option_offset)
        } else {
            picker
                .cursor_index
                .saturating_add(option_offset)
                .min(maximum_index)
        };
        if next != picker.cursor_index {
            picker.cursor_index = next;
            picker
                .scroll_handle
                .scroll_to_item(next, ScrollStrategy::Nearest);
            cx.notify();
        }
        true
    }

    pub(in crate::ui::surface::state) fn select_slack_schedule_time_cursor(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let option_index = self
            .slack_schedule_overlay
            .as_ref()
            .and_then(|overlay| overlay.custom())
            .and_then(|custom| match custom {
                SlackScheduleCustomState {
                    picker: Some(SlackScheduleNestedPicker::Time(picker)),
                    ..
                } => Some(picker.cursor_index),
                _ => None,
            });
        if let Some(option_index) = option_index {
            self.select_slack_schedule_time(option_index, cx);
        }
    }
}
