use super::{
    alpha, px, rgb, slack_schedule_month_full_label, slack_schedule_time_label,
    slack_schedule_weekday_full_label, Arc, Datelike, Months, NaiveDate, NaiveTime, SharedString,
    SlackScheduleCalendarCell, SlackScheduleCalendarCellStatus, SlackScheduleTimeOption,
    TextInputStyle, Timelike, Tz, Utc, SLACK_SCHEDULE_CALENDAR_CELL_COUNT,
    SLACK_SCHEDULE_CUSTOM_STEP_MINUTES,
};

pub(in crate::ui::surface::state) fn slack_schedule_month_start(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
        .expect("chrono date month start must be representable")
}

pub(in crate::ui::surface::state) fn slack_schedule_days_in_month(month: NaiveDate) -> u32 {
    month
        .checked_add_months(Months::new(1))
        .expect("bounded Slack schedule month successor must be representable")
        .pred_opt()
        .expect("bounded Slack schedule month end must be representable")
        .day()
}

pub(in crate::ui::surface::state) fn slack_schedule_date_in_month(
    month: NaiveDate,
    preferred_day: u32,
    minimum_date: NaiveDate,
    maximum_date: NaiveDate,
) -> NaiveDate {
    let day = preferred_day.min(slack_schedule_days_in_month(month));
    NaiveDate::from_ymd_opt(month.year(), month.month(), day)
        .expect("bounded Slack schedule day must be valid")
        .clamp(minimum_date, maximum_date)
}

pub(in crate::ui::surface::state) fn slack_schedule_calendar_cells(
    displayed_month: NaiveDate,
    opened_date: NaiveDate,
    maximum_date: NaiveDate,
    selected_date: NaiveDate,
) -> Arc<[SlackScheduleCalendarCell]> {
    let leading_blanks = usize::try_from(displayed_month.weekday().num_days_from_sunday())
        .expect("weekday column must fit usize");
    let days_in_month =
        usize::try_from(slack_schedule_days_in_month(displayed_month)).expect("day must fit usize");
    (0..SLACK_SCHEDULE_CALENDAR_CELL_COUNT)
        .map(|cell_index| {
            let Some(day_index) = cell_index.checked_sub(leading_blanks) else {
                return slack_schedule_blank_calendar_cell();
            };
            if day_index >= days_in_month {
                return slack_schedule_blank_calendar_cell();
            }
            let day = u32::try_from(day_index + 1).expect("calendar day must fit u32");
            let date =
                NaiveDate::from_ymd_opt(displayed_month.year(), displayed_month.month(), day)
                    .expect("calendar day must be valid for displayed month");
            let status = if date < opened_date || date > maximum_date {
                SlackScheduleCalendarCellStatus::Unavailable
            } else if date == selected_date {
                SlackScheduleCalendarCellStatus::Selected
            } else {
                SlackScheduleCalendarCellStatus::Available
            };
            let mut accessibility_label = format!(
                "{}, {} {}, {}",
                slack_schedule_weekday_full_label(date.weekday()),
                slack_schedule_month_full_label(date.month()),
                date.day(),
                date.year()
            );
            if status == SlackScheduleCalendarCellStatus::Unavailable {
                accessibility_label.push_str(", unavailable");
            } else if status == SlackScheduleCalendarCellStatus::Selected {
                accessibility_label.push_str(", selected");
            }
            SlackScheduleCalendarCell {
                date: Some(date),
                day_label: date.day().to_string().into(),
                accessibility_label: accessibility_label.into(),
                status,
            }
        })
        .collect::<Vec<_>>()
        .into()
}

pub(in crate::ui::surface::state) fn slack_schedule_blank_calendar_cell(
) -> SlackScheduleCalendarCell {
    SlackScheduleCalendarCell {
        date: None,
        day_label: SharedString::default(),
        accessibility_label: "Empty calendar cell".into(),
        status: SlackScheduleCalendarCellStatus::Blank,
    }
}

pub(in crate::ui::surface::state) fn slack_schedule_time_options(
    timezone: Tz,
    date: NaiveDate,
) -> Arc<[SlackScheduleTimeOption]> {
    let now = Utc::now().with_timezone(&timezone);
    let step_seconds = SLACK_SCHEDULE_CUSTOM_STEP_MINUTES * 60;
    let mut seconds = match date.cmp(&now.date_naive()) {
        std::cmp::Ordering::Less => 24 * 60 * 60,
        std::cmp::Ordering::Equal => {
            ((now.time().num_seconds_from_midnight() / step_seconds) + 1) * step_seconds
        }
        std::cmp::Ordering::Greater => 0,
    };
    let mut options = Vec::with_capacity(
        usize::try_from((24 * 60 * 60 - seconds) / step_seconds)
            .expect("Slack schedule time option count must fit usize"),
    );
    while seconds < 24 * 60 * 60 {
        let time = NaiveTime::from_num_seconds_from_midnight_opt(seconds, 0)
            .expect("Slack schedule quarter-hour must be within one day");
        options.push(SlackScheduleTimeOption {
            time,
            label: slack_schedule_time_label(time).into(),
        });
        seconds += step_seconds;
    }
    options.into()
}

pub(in crate::ui::surface::state) fn slack_schedule_time_cursor_index(
    options: &[SlackScheduleTimeOption],
    selected_time: NaiveTime,
) -> Option<usize> {
    options
        .iter()
        .position(|option| option.time >= selected_time)
        .or_else(|| options.len().checked_sub(1))
}

pub(in crate::ui::surface::state) fn slack_schedule_input_style() -> TextInputStyle {
    TextInputStyle {
        height: px(34.0),
        min_height: px(34.0),
        padding_x: px(0.0),
        padding_y: px(6.0),
        radius: px(0.0),
        background: alpha(0x000000, 0.0),
        border: alpha(0x000000, 0.0),
        focused_border: alpha(0x000000, 0.0),
        text: rgb(0xf8f8f8).into(),
        placeholder: rgb(0xababad).into(),
        selection: alpha(0x1264a3, 0.45),
        caret: rgb(0xf8f8f8).into(),
        font_size: px(15.0),
        line_height: px(22.0),
        font_family: Some("Lato".into()),
    }
}
