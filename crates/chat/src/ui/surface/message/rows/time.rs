use chrono::{Offset, TimeZone, Utc};
use chrono_tz::Tz;
use time::{Date, Duration, OffsetDateTime, UtcOffset};

use crate::ui::surface::{
    slack_attachment_date_label, slack_date_label, slack_dm_month_label, slack_dm_ordinal_suffix,
    slack_dm_weekday_label,
};
use crate::ui::{initials, SlackMessage};

#[derive(Clone, Copy)]
pub(super) struct SlackMessageMoment {
    pub(super) instant: OffsetDateTime,
    pub(super) local_timestamp: OffsetDateTime,
    pub(super) local_date: Date,
    pub(super) sort_key: (u64, u32),
}

impl SlackMessageMoment {
    pub(super) fn parse(message_id: &str, timezone: Tz) -> Option<Self> {
        let (seconds, nanoseconds) = slack_message_timestamp_sort_key(message_id)?;
        let unix_nanoseconds = i128::from(seconds)
            .checked_mul(1_000_000_000)?
            .checked_add(i128::from(nanoseconds))?;
        let instant = OffsetDateTime::from_unix_timestamp_nanos(unix_nanoseconds).ok()?;
        let local_offset = slack_message_offset_at(instant, timezone);
        let local_timestamp = instant.to_offset(local_offset);
        Some(Self {
            instant,
            local_timestamp,
            local_date: local_timestamp.date(),
            sort_key: (seconds, nanoseconds),
        })
    }
}

pub(crate) fn slack_message_timezone(timezone_id: Option<&str>) -> Tz {
    timezone_id.map_or(chrono_tz::UTC, |timezone_id| {
        timezone_id
            .parse::<Tz>()
            .expect("validated Slack self timezone must remain valid")
    })
}

fn slack_message_offset_at(instant: OffsetDateTime, timezone: Tz) -> UtcOffset {
    let timestamp =
        chrono::DateTime::<Utc>::from_timestamp(instant.unix_timestamp(), instant.nanosecond())
            .expect("validated Slack timestamp must fit chrono");
    let offset_seconds = timezone
        .offset_from_utc_datetime(&timestamp.naive_utc())
        .fix()
        .local_minus_utc();
    UtcOffset::from_whole_seconds(offset_seconds)
        .expect("IANA timezone offset must fit time::UtcOffset")
}

fn slack_message_timestamp_sort_key(timestamp: &str) -> Option<(u64, u32)> {
    let (seconds, fractional) = timestamp.split_once('.')?;
    if seconds.is_empty()
        || fractional.is_empty()
        || fractional.len() > 9
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let fractional_digits = fractional.len();
    let seconds = seconds.parse::<u64>().ok()?;
    let fractional = fractional.parse::<u32>().ok()?;
    let nanoseconds = fractional.checked_mul(
        10_u32.pow(
            u32::try_from(9 - fractional_digits)
                .expect("validated Slack message timestamp precision must fit u32"),
        ),
    )?;
    Some((seconds, nanoseconds))
}

pub(super) fn slack_message_avatar_label(message: &SlackMessage) -> String {
    if message.author.trim().is_empty() {
        return String::new();
    }
    message
        .avatar_label
        .clone()
        .unwrap_or_else(|| initials(&message.author))
}

pub(super) fn slack_message_clock_label(message: &SlackMessage, timezone: Tz) -> String {
    SlackMessageMoment::parse(&message.id, timezone)
        .and_then(slack_message_moment_clock_label)
        .unwrap_or_else(|| message.timestamp.clone())
}

pub(crate) fn slack_all_threads_timestamp_label(
    message: &SlackMessage,
    timezone: Tz,
    today: Date,
) -> String {
    let Some(moment) = SlackMessageMoment::parse(&message.id, timezone) else {
        return message.timestamp.clone();
    };
    let clock_label =
        slack_message_moment_clock_label(moment).unwrap_or_else(|| message.timestamp.clone());
    let days_ago = today.to_julian_day() - moment.local_date.to_julian_day();
    let date_label = match days_ago {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        2..=6 => slack_dm_weekday_label(moment.local_date.weekday()).to_string(),
        _ => format!(
            "{} {}{}",
            slack_dm_month_label(moment.local_date.month()),
            moment.local_date.day(),
            slack_dm_ordinal_suffix(moment.local_date.day())
        ),
    };
    format!("{date_label} at {clock_label}")
}

fn slack_message_moment_clock_label(moment: SlackMessageMoment) -> Option<String> {
    moment
        .local_timestamp
        .format(time::macros::format_description!(
            "[hour repr:12 padding:none]:[minute] [period case:upper]"
        ))
        .ok()
}

pub(super) fn slack_message_divider_label(
    previous_message: Option<&SlackMessage>,
    previous_moment: Option<SlackMessageMoment>,
    message: &SlackMessage,
    moment: Option<SlackMessageMoment>,
    today: Date,
) -> Option<String> {
    if let Some(label) = message.date_divider_label.clone() {
        return Some(label);
    }
    if let Some(moment) = moment {
        return previous_moment
            .is_none_or(|previous| previous.local_date != moment.local_date)
            .then(|| slack_sticky_date_label(moment.local_date, today));
    }
    let attachment = message.attachments.first()?;
    let label = slack_attachment_date_label(attachment)?;
    let previous_attachment_label = previous_message
        .and_then(|previous| previous.attachments.first())
        .and_then(slack_attachment_date_label);
    if previous_attachment_label.as_deref() == Some(label.as_str()) {
        return None;
    }
    Some(label)
}

pub(crate) fn slack_local_today(timezone: Tz) -> Date {
    let now = OffsetDateTime::now_utc();
    let offset = slack_message_offset_at(now, timezone);
    now.to_offset(offset).date()
}

pub(super) fn slack_sticky_date_label(date: Date, today: Date) -> String {
    if date == today {
        "Today".to_string()
    } else if today.previous_day() == Some(date) {
        "Yesterday".to_string()
    } else {
        slack_date_label(date)
    }
}

pub(super) fn slack_message_is_compact(
    previous_message: Option<&SlackMessage>,
    previous_moment: Option<SlackMessageMoment>,
    message: &SlackMessage,
    moment: Option<SlackMessageMoment>,
) -> bool {
    let (Some(previous_message), Some(previous_moment), Some(moment)) =
        (previous_message, previous_moment, moment)
    else {
        return false;
    };
    let elapsed = moment.instant - previous_moment.instant;
    slack_messages_have_same_sender(previous_message, message)
        && previous_moment.local_date == moment.local_date
        && elapsed >= Duration::ZERO
        && elapsed <= Duration::minutes(5)
}

fn slack_messages_have_same_sender(previous: &SlackMessage, message: &SlackMessage) -> bool {
    match (previous.user_id.as_deref(), message.user_id.as_deref()) {
        (Some(previous_user_id), Some(user_id)) => previous_user_id == user_id,
        _ => !message.author.trim().is_empty() && previous.author.trim() == message.author.trim(),
    }
}
