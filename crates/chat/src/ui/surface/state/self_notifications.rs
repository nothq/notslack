//! Pausing notifications from the self menu, as Slack's "Notifications"
//! submenu does with `dnd.setSnooze` and `dnd.endSnooze`.

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone};

use super::{Context, SurfaceState};
use crate::ui::{spawn_background_task_for_entity, SlackWorkspace};

/// What the self menu last did to notifications, which stands until the
/// next workspace refresh reports Slack's own state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackNotificationsPause {
    /// Paused until this Unix time.
    Until(i64),
    Resumed,
}

/// The submenu's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackNotificationPausePreset {
    ThirtyMinutes,
    OneHour,
    TwoHours,
    UntilTomorrow,
    UntilNextWeek,
}

impl SlackNotificationPausePreset {
    pub(crate) const ALL: [Self; 5] = [
        Self::ThirtyMinutes,
        Self::OneHour,
        Self::TwoHours,
        Self::UntilTomorrow,
        Self::UntilNextWeek,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ThirtyMinutes => "30 minutes",
            Self::OneHour => "1 hour",
            Self::TwoHours => "2 hours",
            Self::UntilTomorrow => "Until tomorrow",
            Self::UntilNextWeek => "Until next week",
        }
    }

    /// Minutes from `now` until the pause ends. "Tomorrow" and "next week"
    /// end at 9 AM, on the next day and on the next Monday.
    pub(crate) fn minutes_from(self, now: DateTime<Local>) -> u32 {
        let minutes = match self {
            Self::ThirtyMinutes => 30,
            Self::OneHour => 60,
            Self::TwoHours => 120,
            Self::UntilTomorrow => minutes_until_morning(now, 1),
            Self::UntilNextWeek => {
                let days = 7 - i64::from(now.weekday().num_days_from_monday());
                minutes_until_morning(now, days)
            }
        };
        u32::try_from(minutes.max(1)).unwrap_or(u32::MAX)
    }
}

fn minutes_until_morning(now: DateTime<Local>, days_ahead: i64) -> i64 {
    let day = now.date_naive() + Duration::days(days_ahead);
    let morning = day.and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default());
    Local
        .from_local_datetime(&morning)
        .earliest()
        .map_or(24 * 60, |morning| (morning - now).num_minutes())
}

impl SurfaceState {
    /// Whether notifications are paused, by the menu's last change or by
    /// what Slack reported.
    pub(crate) fn slack_self_notifications_paused(&self, workspace: &SlackWorkspace) -> bool {
        match self.slack_self_notifications_pause {
            Some(SlackNotificationsPause::Until(until)) => Local::now().timestamp() < until,
            Some(SlackNotificationsPause::Resumed) => false,
            None => workspace.rail_badges.self_notifications_paused,
        }
    }

    pub(crate) fn set_slack_self_menu_notifications_open(
        &mut self,
        open: bool,
        cx: &mut Context<Self>,
    ) {
        if self.slack_self_menu_notifications_open != open {
            self.slack_self_menu_notifications_open = open;
            cx.notify();
        }
    }

    /// Pauses notifications for the preset, or resumes them with `None`.
    pub(crate) fn pause_slack_notifications(
        &mut self,
        preset: Option<SlackNotificationPausePreset>,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let now = Local::now();
        let minutes = preset.map(|preset| preset.minutes_from(now));
        let pause = match minutes {
            Some(minutes) => {
                SlackNotificationsPause::Until(now.timestamp() + i64::from(minutes) * 60)
            }
            None => SlackNotificationsPause::Resumed,
        };
        self.slack_self_menu_open = false;
        self.slack_self_menu_notifications_open = false;
        self.slack_self_settings_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, minutes),
            cx,
            |(workspace_api, minutes)| workspace_api.pause_slack_notifications(minutes),
            move |this, result, cx| {
                match result {
                    Ok(_) => this.slack_self_notifications_pause = Some(pause),
                    Err(error) => this.slack_self_settings_error = Some(error),
                }
                cx.notify();
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::SlackNotificationPausePreset;

    #[test]
    fn pauses_end_at_the_expected_time() {
        // Wednesday 2026-11-18, 10:00.
        let now = Local
            .with_ymd_and_hms(2026, 11, 18, 10, 0, 0)
            .single()
            .expect("valid local time");
        assert_eq!(
            SlackNotificationPausePreset::ThirtyMinutes.minutes_from(now),
            30
        );
        assert_eq!(
            SlackNotificationPausePreset::TwoHours.minutes_from(now),
            120
        );
        // Until 9 AM Thursday.
        assert_eq!(
            SlackNotificationPausePreset::UntilTomorrow.minutes_from(now),
            23 * 60
        );
        // Until 9 AM Monday.
        assert_eq!(
            SlackNotificationPausePreset::UntilNextWeek.minutes_from(now),
            (4 * 24 + 23) * 60
        );
    }
}
