use serde_json::Value;

use crate::model::{SlackSelfStatus, SlackUserPresence};

use super::SlackApiClient;

const SLACK_PROFILE_SET_METHOD: &str = "users.profile.set";
const SLACK_PRESENCE_SET_METHOD: &str = "presence.set";
const SLACK_DND_SET_SNOOZE_METHOD: &str = "dnd.setSnooze";
const SLACK_DND_END_SNOOZE_METHOD: &str = "dnd.endSnooze";

impl SlackApiClient {
    pub(crate) fn mutate_self_status(
        &self,
        status: &SlackSelfStatus,
    ) -> Result<SlackSelfStatus, String> {
        let profile = serde_json::json!({
            "status_text": status.text(),
            "status_emoji": status.emoji(),
            "status_expiration": status.expiration(),
        });
        let payload = self.post(
            SLACK_PROFILE_SET_METHOD,
            &[("profile", profile.to_string())],
        )?;
        slack_self_status_from_profile(
            payload.get("profile").ok_or_else(|| {
                format!("Slack {SLACK_PROFILE_SET_METHOD} response missing profile")
            })?,
        )
    }

    /// Pauses notifications for `minutes`, or resumes them with `None`.
    /// Returns whether notifications are paused afterwards.
    pub(crate) fn pause_self_notifications(&self, minutes: Option<u32>) -> Result<bool, String> {
        match minutes {
            Some(minutes) => {
                self.post(
                    SLACK_DND_SET_SNOOZE_METHOD,
                    &[("num_minutes", minutes.max(1).to_string())],
                )?;
                Ok(true)
            }
            None => {
                self.post(SLACK_DND_END_SNOOZE_METHOD, &[])?;
                Ok(false)
            }
        }
    }

    pub(crate) fn mutate_self_presence(
        &self,
        presence: SlackUserPresence,
    ) -> Result<SlackUserPresence, String> {
        let wire_presence = match presence {
            SlackUserPresence::Active => "auto",
            SlackUserPresence::Away => "away",
        };
        self.post(
            SLACK_PRESENCE_SET_METHOD,
            &[("presence", wire_presence.to_string())],
        )?;
        Ok(presence)
    }
}

fn slack_self_status_from_profile(profile: &Value) -> Result<SlackSelfStatus, String> {
    let text = profile
        .get("status_text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let emoji = profile
        .get("status_emoji")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let expiration = profile
        .get("status_expiration")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or_default();
    SlackSelfStatus::new(text, emoji, expiration)
}
