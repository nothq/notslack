mod actions;
mod request;

pub(crate) use actions::SlackDateJumpMenuTarget;

use std::sync::Arc;

use chrono::{Datelike, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use gpui::{ListOffset, SharedString};
use time::{Date, Duration, Month, Weekday};

use super::{
    prepare_slack_conversation_snapshot, px, Context, KeyDownEvent,
    PreparedSlackConversationSnapshot, SurfaceState, WorkspaceApi,
};
use crate::ui::surface::slack_message_timezone;
use crate::ui::{spawn_background_task_for_entity, SlackMessageTimestamp};

const SLACK_DATE_JUMP_CALENDAR_CELL_COUNT: usize = 42;
const SLACK_DATE_JUMP_MENU_ACTION_COUNT: usize = 5;
const SLACK_DATE_JUMP_MINIMUM_YEAR: i32 = 1970;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackDateJumpMenuAction {
    Yesterday,
    LastWeek,
    LastMonth,
    Beginning,
    SpecificDate,
}

impl SlackDateJumpMenuAction {
    pub(crate) const ALL: [Self; SLACK_DATE_JUMP_MENU_ACTION_COUNT] = [
        Self::Yesterday,
        Self::LastWeek,
        Self::LastMonth,
        Self::Beginning,
        Self::SpecificDate,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Yesterday => "Yesterday",
            Self::LastWeek => "Last week",
            Self::LastMonth => "Last month",
            Self::Beginning => "The very beginning",
            Self::SpecificDate => "Jump to a specific date",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackDateJumpMenuState {
    pub(crate) divider_id: SharedString,
    pub(crate) source_date: Date,
    pub(crate) anchor_left: f32,
    pub(crate) anchor_top: f32,
    pub(crate) selected_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackDateJumpCalendarCellStatus {
    Blank,
    Unavailable,
    Available,
    Today,
}

impl SlackDateJumpCalendarCellStatus {
    pub(crate) fn is_selectable(self) -> bool {
        matches!(self, Self::Available | Self::Today)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDateJumpCalendarCell {
    pub(crate) date: Option<Date>,
    pub(crate) day_label: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) status: SlackDateJumpCalendarCellStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDateJumpPickerState {
    pub(crate) today: Date,
    pub(crate) displayed_month: Date,
    pub(crate) month_label: SharedString,
    pub(crate) cells: Arc<[SlackDateJumpCalendarCell]>,
    pub(crate) cursor_date: Date,
    pub(crate) can_show_previous_month: bool,
    pub(crate) can_show_next_month: bool,
    pub(crate) can_show_previous_year: bool,
    pub(crate) can_show_next_year: bool,
}

impl SlackDateJumpPickerState {
    fn new(today: Date) -> Self {
        let displayed_month = slack_date_jump_month_start(today);
        let mut picker = Self {
            today,
            displayed_month,
            month_label: SharedString::default(),
            cells: Arc::default(),
            cursor_date: today,
            can_show_previous_month: false,
            can_show_next_month: false,
            can_show_previous_year: false,
            can_show_next_year: false,
        };
        picker.rebuild(displayed_month, today);
        picker
    }

    fn rebuild(&mut self, displayed_month: Date, cursor_date: Date) {
        let minimum_month = slack_date_jump_minimum_date();
        let maximum_month = slack_date_jump_month_start(self.today);
        self.displayed_month = displayed_month;
        self.month_label = format!(
            "{} {}",
            slack_date_jump_month_label(displayed_month.month()),
            displayed_month.year()
        )
        .into();
        self.cells = slack_date_jump_calendar_cells(displayed_month, self.today);
        self.cursor_date = cursor_date;
        self.can_show_previous_month = displayed_month > minimum_month;
        self.can_show_next_month = displayed_month < maximum_month;
        self.can_show_previous_year = slack_date_jump_shift_month(displayed_month, -12)
            .is_some_and(|date| date >= minimum_month);
        self.can_show_next_year = slack_date_jump_shift_month(displayed_month, 12)
            .is_some_and(|date| date <= maximum_month);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SlackDateJumpOverlay {
    Menu(SlackDateJumpMenuState),
    Picker(SlackDateJumpPickerState),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SlackDateJumpTarget {
    Date {
        local_date: Date,
        anchor_timestamp: SlackMessageTimestamp,
    },
    Beginning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDateJumpRequest {
    generation: u64,
    team_id: String,
    conversation_id: String,
    target: SlackDateJumpTarget,
}

impl SurfaceState {}

fn slack_date_jump_today(timezone: Tz) -> Result<Date, String> {
    let today = Utc::now().with_timezone(&timezone).date_naive();
    slack_date_jump_time_date(today)
}

fn slack_date_jump_time_date(date: NaiveDate) -> Result<Date, String> {
    Date::from_calendar_date(
        date.year(),
        Month::try_from(u8::try_from(date.month()).expect("month must fit u8"))
            .expect("chrono month must be a valid time month"),
        u8::try_from(date.day()).expect("day must fit u8"),
    )
    .map_err(|error| format!("Slack date navigation received an invalid local date: {error}"))
}

fn slack_date_jump_naive_date(date: Date) -> Result<NaiveDate, String> {
    NaiveDate::from_ymd_opt(
        date.year(),
        u32::from(u8::from(date.month())),
        u32::from(date.day()),
    )
    .ok_or_else(|| "Slack date navigation could not convert the selected date.".to_string())
}

fn slack_date_jump_end_timestamp(
    local_date: Date,
    timezone: Tz,
) -> Result<SlackMessageTimestamp, String> {
    let next_date = local_date
        .next_day()
        .ok_or_else(|| "Slack date navigation cannot represent the following day.".to_string())?;
    let mut local_midnight = slack_date_jump_naive_date(next_date)?
        .and_hms_opt(0, 0, 0)
        .expect("midnight must be a valid local time");
    let next_day_start = (0..=2_880)
        .find_map(|_| {
            let result = match timezone.from_local_datetime(&local_midnight) {
                LocalResult::Single(value) => Some(value),
                LocalResult::Ambiguous(first, second) => {
                    Some(if first.timestamp_micros() <= second.timestamp_micros() {
                        first
                    } else {
                        second
                    })
                }
                LocalResult::None => None,
            };
            local_midnight = local_midnight
                .checked_add_signed(chrono::Duration::minutes(1))
                .expect("bounded local date search must fit chrono");
            result
        })
        .ok_or_else(|| {
            format!(
                "Slack date navigation could not resolve the selected date in the {timezone} time zone."
            )
        })?;
    let end_micros = next_day_start
        .timestamp_micros()
        .checked_sub(1)
        .ok_or_else(|| "Slack date navigation timestamp underflowed.".to_string())?;
    let seconds = end_micros.div_euclid(1_000_000);
    let micros = end_micros.rem_euclid(1_000_000);
    SlackMessageTimestamp::parse(&format!("{seconds}.{micros:06}"))
}

fn slack_date_jump_minimum_date() -> Date {
    Date::from_calendar_date(SLACK_DATE_JUMP_MINIMUM_YEAR, Month::January, 1)
        .expect("Slack date-jump minimum date must be valid")
}

fn slack_date_jump_month_start(date: Date) -> Date {
    Date::from_calendar_date(date.year(), date.month(), 1)
        .expect("Slack date-jump month start must be valid")
}

fn slack_date_jump_shift_month(month_start: Date, month_delta: i32) -> Option<Date> {
    let month_index = i32::from(u8::from(month_start.month())) - 1;
    let absolute_month = month_start
        .year()
        .checked_mul(12)?
        .checked_add(month_index)?
        .checked_add(month_delta)?;
    let year = absolute_month.div_euclid(12);
    if year < SLACK_DATE_JUMP_MINIMUM_YEAR {
        return None;
    }
    let month = Month::try_from(u8::try_from(absolute_month.rem_euclid(12) + 1).ok()?).ok()?;
    Date::from_calendar_date(year, month, 1).ok()
}

fn slack_date_jump_date_in_month(month_start: Date, day: u8, maximum: Date) -> Date {
    let next_month =
        slack_date_jump_shift_month(month_start, 1).expect("supported month must have a successor");
    let last_day = next_month
        .previous_day()
        .expect("month successor must have a previous day")
        .day();
    Date::from_calendar_date(month_start.year(), month_start.month(), day.min(last_day))
        .expect("clamped Slack date-jump day must be valid")
        .min(maximum)
}

fn slack_date_jump_previous_month_date(date: Date) -> Date {
    let previous_month = slack_date_jump_shift_month(slack_date_jump_month_start(date), -1)
        .expect("current date must have a supported previous month");
    slack_date_jump_date_in_month(previous_month, date.day(), date)
}

fn slack_date_jump_calendar_cells(
    displayed_month: Date,
    today: Date,
) -> Arc<[SlackDateJumpCalendarCell]> {
    let weekday_offset = i64::from(slack_date_jump_weekday_index(displayed_month.weekday()));
    let grid_start = displayed_month
        .checked_sub(Duration::days(weekday_offset))
        .expect("supported Slack date-jump month must have a calendar grid start");
    (0..SLACK_DATE_JUMP_CALENDAR_CELL_COUNT)
        .map(|index| {
            let date = grid_start
                .checked_add(Duration::days(
                    i64::try_from(index).expect("calendar index must fit i64"),
                ))
                .expect("bounded Slack date-jump calendar grid must fit time::Date");
            if date.month() != displayed_month.month() || date.year() != displayed_month.year() {
                return SlackDateJumpCalendarCell {
                    date: None,
                    day_label: SharedString::default(),
                    accessibility_label: SharedString::default(),
                    status: SlackDateJumpCalendarCellStatus::Blank,
                };
            }
            let status = if date > today || date < slack_date_jump_minimum_date() {
                SlackDateJumpCalendarCellStatus::Unavailable
            } else if date == today {
                SlackDateJumpCalendarCellStatus::Today
            } else {
                SlackDateJumpCalendarCellStatus::Available
            };
            SlackDateJumpCalendarCell {
                date: Some(date),
                day_label: date.day().to_string().into(),
                accessibility_label: format!(
                    "{} {}, {}",
                    slack_date_jump_month_label(date.month()),
                    date.day(),
                    date.year()
                )
                .into(),
                status,
            }
        })
        .collect::<Vec<_>>()
        .into()
}

fn slack_date_jump_weekday_index(weekday: Weekday) -> u8 {
    match weekday {
        Weekday::Sunday => 0,
        Weekday::Monday => 1,
        Weekday::Tuesday => 2,
        Weekday::Wednesday => 3,
        Weekday::Thursday => 4,
        Weekday::Friday => 5,
        Weekday::Saturday => 6,
    }
}

fn slack_date_jump_month_label(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}
