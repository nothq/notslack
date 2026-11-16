//! Whether someone is in Do Not Disturb right now, from Slack's DND state
//! (`dnd.info`, or the `dnd` of `client.userBoot`).
//!
//! `dnd_enabled` only says a schedule is set up, such as quiet hours every
//! night; the person is in DND while the current time falls inside the
//! scheduled window, or while a snooze runs.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct SlackDndState {
    #[serde(default)]
    dnd_enabled: bool,
    #[serde(default)]
    next_dnd_start_ts: Option<f64>,
    #[serde(default)]
    next_dnd_end_ts: Option<f64>,
    #[serde(default)]
    snooze_enabled: bool,
    #[serde(default)]
    snooze_endtime: Option<f64>,
}

impl SlackDndState {
    pub(crate) fn is_active_now(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        self.is_active_at(now)
    }

    pub(crate) fn is_active_at(&self, now: f64) -> bool {
        let snoozing = self.snooze_enabled && self.snooze_endtime.is_none_or(|end| now < end);
        let scheduled = self.dnd_enabled
            && matches!(
                (self.next_dnd_start_ts, self.next_dnd_end_ts),
                (Some(start), Some(end)) if start <= now && now < end
            );
        snoozing || scheduled
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::SlackDndState;

    fn state(value: serde_json::Value) -> SlackDndState {
        serde_json::from_value(value).expect("valid dnd state")
    }

    #[test]
    fn a_schedule_only_counts_inside_its_window() {
        let nightly = state(json!({
            "dnd_enabled": true,
            "next_dnd_start_ts": 1000,
            "next_dnd_end_ts": 2000,
            "snooze_enabled": false,
        }));
        assert!(!nightly.is_active_at(500.0));
        assert!(nightly.is_active_at(1500.0));
        assert!(!nightly.is_active_at(2000.0));
    }

    #[test]
    fn a_snooze_counts_until_it_ends() {
        let snoozed = state(json!({
            "dnd_enabled": false,
            "snooze_enabled": true,
            "snooze_endtime": 3000,
        }));
        assert!(snoozed.is_active_at(2999.0));
        assert!(!snoozed.is_active_at(3000.0));
        assert!(!state(json!({})).is_active_at(1.0));
    }
}
