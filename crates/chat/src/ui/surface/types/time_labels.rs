use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Timelike, Utc};

use super::{Month, OffsetDateTime, UtcOffset, Weekday};

pub(crate) fn slack_dm_peer_local_time_label(
    timezone: crate::model::SlackIanaTimezone,
    now: DateTime<Utc>,
) -> Option<String> {
    let local_now = now.with_timezone(&timezone.as_chrono_tz());
    (local_now.hour() >= 22 || local_now.hour() < 8)
        .then(|| local_now.format("%-I:%M %p").to_string())
}

pub(crate) fn slack_duration_until_next_minute() -> Duration {
    const NANOS_PER_MINUTE: u128 = 60_000_000_000;

    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("The desktop system time must follow the Unix epoch");
    let elapsed_in_minute = elapsed.as_nanos() % NANOS_PER_MINUTE;
    Duration::from_nanos(
        u64::try_from(NANOS_PER_MINUTE - elapsed_in_minute)
            .expect("one minute must fit in a duration nanosecond count"),
    )
}

pub(crate) fn slack_dm_relative_timestamp(timestamp: &str, now: OffsetDateTime) -> String {
    let Some(seconds) = timestamp
        .split('.')
        .next()
        .and_then(|seconds| seconds.parse::<i64>().ok())
    else {
        return String::new();
    };
    let Ok(timestamp) = OffsetDateTime::from_unix_timestamp(seconds) else {
        return String::new();
    };
    slack_relative_timestamp(timestamp, now)
}

pub(crate) fn slack_relative_timestamp(timestamp: OffsetDateTime, now: OffsetDateTime) -> String {
    let offset = UtcOffset::current_local_offset()
        .expect("The desktop must provide the local timezone offset");
    let local_timestamp = timestamp.to_offset(offset);
    let local_now = now.to_offset(offset);
    let age_seconds = (now - timestamp).whole_seconds();
    if age_seconds < 86_400 {
        return slack_dm_clock_label(local_timestamp);
    }
    if age_seconds < 172_800 {
        return "Yesterday".to_string();
    }
    let days_ago = local_now.date().to_julian_day() - local_timestamp.date().to_julian_day();
    match days_ago {
        2..=6 => slack_dm_weekday_label(local_timestamp.weekday()).to_string(),
        _ => format!(
            "{} {}{}",
            slack_dm_month_label(local_timestamp.month()),
            local_timestamp.day(),
            slack_dm_ordinal_suffix(local_timestamp.day())
        ),
    }
}

pub(crate) fn slack_dm_clock_label(timestamp: OffsetDateTime) -> String {
    let hour = timestamp.hour();
    let period = if hour < 12 { "AM" } else { "PM" };
    let hour = match hour % 12 {
        0 => 12,
        hour => hour,
    };
    format!("{hour}:{:02} {period}", timestamp.minute())
}

pub(crate) fn slack_dm_weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

pub(crate) fn slack_dm_month_label(month: Month) -> &'static str {
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

pub(crate) fn slack_dm_ordinal_suffix(day: u8) -> &'static str {
    if matches!(day % 100, 11..=13) {
        return "th";
    }
    match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}
