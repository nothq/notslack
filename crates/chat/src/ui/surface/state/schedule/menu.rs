use super::{
    next_weekday, slack_schedule_date_label, slack_schedule_time_label, Context, Days, NaiveDate,
    NaiveTime, SlackScheduleCustomState, SlackScheduleMenuState, SlackSchedulePostAt,
    SlackSchedulePreset, SurfaceState, Tz, Utc, Weekday, Window, SLACK_SCHEDULE_EVENING_HOUR,
    SLACK_SCHEDULE_MAX_DAYS, SLACK_SCHEDULE_MORNING_HOUR,
};

struct SlackScheduleCustomSeed {
    timezone: Tz,
    timezone_label: String,
    today: NaiveDate,
    date: NaiveDate,
    time: NaiveTime,
}

pub(in crate::ui::surface::state) fn slack_schedule_menu_state(
    timezone_id: &str,
    timezone_label: String,
) -> Result<SlackScheduleMenuState, String> {
    let timezone = timezone_id
        .parse::<Tz>()
        .map_err(|error| format!("Invalid Slack workspace time zone {timezone_id}: {error}"))?;
    let now_utc = Utc::now();
    let now = now_utc.with_timezone(&timezone);
    let today = now.date_naive();
    let tomorrow = today
        .succ_opt()
        .ok_or_else(|| "Slack schedule date overflowed".to_string())?;
    let monday = next_weekday(today, Weekday::Mon)?;
    let morning = NaiveTime::from_hms_opt(SLACK_SCHEDULE_MORNING_HOUR, 0, 0)
        .expect("Slack morning preset must be a valid time");
    let evening = NaiveTime::from_hms_opt(SLACK_SCHEDULE_EVENING_HOUR, 0, 0)
        .expect("Slack evening preset must be a valid time");
    let morning_today = SlackSchedulePostAt::from_local(timezone, today, morning)?;
    let evening_today = SlackSchedulePostAt::from_local(timezone, today, evening)?;
    let later_today = slack_schedule_later_today(now_utc.timestamp(), morning_today, evening_today);
    let tomorrow_morning = SlackSchedulePostAt::from_local(timezone, tomorrow, morning)?;
    let monday_morning = SlackSchedulePostAt::from_local(timezone, monday, morning)?;
    let (custom_date, custom_time) = if morning_today.0 > now_utc.timestamp() {
        (today, morning)
    } else if evening_today.0 > now_utc.timestamp() {
        (today, evening)
    } else {
        (tomorrow, morning)
    };
    let custom = slack_schedule_custom_state(SlackScheduleCustomSeed {
        timezone,
        timezone_label,
        today,
        date: custom_date,
        time: custom_time,
    })?;
    Ok(SlackScheduleMenuState {
        presets: [
            later_today,
            Some(SlackSchedulePreset {
                label: "Tomorrow at 9:00 AM".to_string(),
                post_at: tomorrow_morning,
            }),
            Some(SlackSchedulePreset {
                label: "Monday at 9:00 AM".to_string(),
                post_at: monday_morning,
            }),
        ],
        custom,
    })
}

fn slack_schedule_later_today(
    now_unix_seconds: i64,
    morning: SlackSchedulePostAt,
    evening: SlackSchedulePostAt,
) -> Option<SlackSchedulePreset> {
    if morning.0 > now_unix_seconds {
        Some(SlackSchedulePreset {
            label: "Later today at 9:00 AM".to_string(),
            post_at: morning,
        })
    } else if evening.0 > now_unix_seconds {
        Some(SlackSchedulePreset {
            label: "Later today at 6:00 PM".to_string(),
            post_at: evening,
        })
    } else {
        None
    }
}

fn slack_schedule_custom_state(
    seed: SlackScheduleCustomSeed,
) -> Result<SlackScheduleCustomState, String> {
    let maximum_date = seed
        .today
        .checked_add_days(Days::new(SLACK_SCHEDULE_MAX_DAYS))
        .ok_or_else(|| "Slack custom schedule date overflowed".to_string())?;
    Ok(SlackScheduleCustomState {
        timezone: seed.timezone,
        timezone_label: seed.timezone_label,
        opened_date: seed.today,
        maximum_date,
        date: seed.date,
        time: seed.time,
        date_label: slack_schedule_date_label(seed.today, seed.date),
        time_label: slack_schedule_time_label(seed.time),
        date_edit_text: slack_schedule_date_label(seed.today, seed.date),
        time_edit_text: slack_schedule_time_label(seed.time),
        date_input_focused: false,
        time_input_focused: false,
        error: None,
        picker: None,
    })
}

pub(in crate::ui::surface::state) fn stop_slack_schedule_key_event(
    handled: bool,
    window: &mut Window,
    cx: &mut Context<SurfaceState>,
) {
    if handled {
        window.prevent_default();
        cx.stop_propagation();
    }
}
