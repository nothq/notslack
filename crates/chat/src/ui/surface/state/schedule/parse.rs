use super::{Datelike, Days, NaiveDate, NaiveTime, Timelike, Weekday};

pub(in crate::ui::surface::state) fn parse_slack_schedule_date_input(
    value: &str,
    opened_date: NaiveDate,
    maximum_date: NaiveDate,
) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("today") {
        return Some(opened_date);
    }
    if trimmed.eq_ignore_ascii_case("tomorrow") {
        return opened_date.succ_opt().filter(|date| *date <= maximum_date);
    }
    for format in [
        "%m/%d/%Y",
        "%m/%d/%y",
        "%Y-%m-%d",
        "%b %d, %Y",
        "%B %d, %Y",
        "%a, %b %d",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, format) {
            let date = if format == "%a, %b %d" {
                NaiveDate::from_ymd_opt(opened_date.year(), date.month(), date.day())?
            } else {
                date
            };
            if (opened_date..=maximum_date).contains(&date) {
                return Some(date);
            }
        }
    }
    let parts = trimmed
        .split(['/', '-', '.'])
        .map(str::trim)
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let first = parts[0].parse::<u32>().ok()?;
    let second = parts[1].parse::<u32>().ok()?;
    let third = parts[2].parse::<i32>().ok()?;
    let (year, month, day) = if parts[0].len() == 4 {
        (
            i32::try_from(first).ok()?,
            second,
            u32::try_from(third).ok()?,
        )
    } else {
        (
            if third < 100 { 2000 + third } else { third },
            first,
            second,
        )
    };
    NaiveDate::from_ymd_opt(year, month, day)
        .filter(|date| (opened_date..=maximum_date).contains(date))
}

pub(in crate::ui::surface::state) fn parse_slack_schedule_time_input(
    value: &str,
) -> Option<NaiveTime> {
    let normalized = value.trim().to_ascii_lowercase();
    let (clock, period) = if let Some(clock) = normalized.strip_suffix("am") {
        (clock.trim(), Some(false))
    } else if let Some(clock) = normalized.strip_suffix("pm") {
        (clock.trim(), Some(true))
    } else {
        (normalized.as_str(), None)
    };
    let (mut hour, minute) = if let Some((hour, minute)) = clock.split_once(':') {
        (
            hour.trim().parse::<u32>().ok()?,
            minute.trim().parse::<u32>().ok()?,
        )
    } else {
        let compact = clock.replace(char::is_whitespace, "");
        let numeric = compact.parse::<u32>().ok()?;
        if compact.len() >= 3 {
            (numeric / 100, numeric % 100)
        } else {
            (numeric, 0)
        }
    };
    if let Some(is_pm) = period {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour %= 12;
        if is_pm {
            hour += 12;
        }
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

pub(in crate::ui::surface::state) fn next_weekday(
    date: NaiveDate,
    weekday: Weekday,
) -> Result<NaiveDate, String> {
    let current = i64::from(date.weekday().num_days_from_monday());
    let target = i64::from(weekday.num_days_from_monday());
    let days = (target - current).rem_euclid(7);
    let days = u64::try_from(days).expect("weekday distance must be nonnegative");
    date.checked_add_days(Days::new(if days == 0 { 7 } else { days }))
        .ok_or_else(|| "Slack weekly schedule date overflowed".to_string())
}

pub(in crate::ui::surface::state) fn slack_schedule_date_label(
    opened_date: NaiveDate,
    date: NaiveDate,
) -> String {
    if date == opened_date {
        return "Today".to_string();
    }
    if opened_date.succ_opt() == Some(date) {
        return "Tomorrow".to_string();
    }
    format!(
        "{}, {} {}",
        slack_schedule_weekday_label(date.weekday()),
        slack_schedule_month_label(date.month()),
        date.day()
    )
}

pub(in crate::ui::surface::state) fn slack_schedule_time_label(time: NaiveTime) -> String {
    let hour = match time.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    let period = if time.hour() < 12 { "AM" } else { "PM" };
    format!("{hour}:{:02} {period}", time.minute())
}

pub(in crate::ui::surface::state) fn slack_schedule_weekday_label(
    weekday: Weekday,
) -> &'static str {
    match weekday {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

pub(in crate::ui::surface::state) fn slack_schedule_weekday_full_label(
    weekday: Weekday,
) -> &'static str {
    match weekday {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

pub(in crate::ui::surface::state) fn slack_schedule_month_label(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => unreachable!("chrono month must be in 1..=12"),
    }
}

pub(in crate::ui::surface::state) fn slack_schedule_month_full_label(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => unreachable!("chrono month must be in 1..=12"),
    }
}
