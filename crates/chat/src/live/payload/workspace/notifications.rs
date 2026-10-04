use std::collections::HashMap;

use crate::model::{
    SlackChannelNotificationMode, SlackChannelNotificationMutation,
    SlackChannelNotificationPreference,
};
use serde::{Deserialize, Serialize};

use super::SlackLiveWorkspaceLoader;

#[derive(Clone, Deserialize)]
pub(super) struct SlackNotificationPreferences {
    global: SlackGlobalNotificationPreferences,
    channels: HashMap<String, SlackChannelNotificationOverrides>,
}

#[derive(Clone, Deserialize)]
struct SlackGlobalNotificationPreferences {
    global_desktop: String,
    global_mobile: String,
}

#[derive(Clone, Default, Deserialize)]
struct SlackChannelNotificationOverrides {
    desktop: Option<String>,
    mobile: Option<String>,
    muted: Option<bool>,
}

#[derive(Serialize)]
struct SlackNotificationPrefUpdate {
    name: &'static str,
    value: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync: Option<bool>,
}

impl SlackLiveWorkspaceLoader {
    pub fn load_channel_notification_preference(
        &self,
        conversation_id: &str,
    ) -> Result<SlackChannelNotificationPreference, String> {
        let conversation_id = require_conversation_id(conversation_id)?;
        let preferences = self.user_preferences()?;
        preferences
            .notifications
            .preference_for_channel(&self.team_id, conversation_id)
    }

    pub fn mutate_channel_notification_preference(
        &self,
        conversation_id: &str,
        mutation: SlackChannelNotificationMutation,
    ) -> Result<SlackChannelNotificationPreference, String> {
        let conversation_id = require_conversation_id(conversation_id)?;
        let _mutation_guard = self
            .user_preferences_mutation_lock
            .lock()
            .map_err(|_| "Slack user-preference mutation mutex poisoned".to_string())?;
        let current = self.load_channel_notification_preference(conversation_id)?;
        if notification_mutation_is_noop(&current, mutation) {
            return Ok(current);
        }

        match mutation {
            SlackChannelNotificationMutation::SetMode(mode) => {
                self.set_channel_notification_mode(conversation_id, current, mode)?;
            }
            SlackChannelNotificationMutation::SetMuted(muted) => {
                self.set_channel_muted(conversation_id, muted)?;
            }
        }

        *self
            .user_preferences_cache
            .lock()
            .map_err(|_| "Slack user-preferences cache mutex poisoned".to_string())? = None;
        self.load_channel_notification_preference(conversation_id)
            .map_err(|error| {
                format!(
                    "Slack channel notification preference changed, but reloading it failed: {error}"
                )
            })
    }

    fn set_channel_notification_mode(
        &self,
        conversation_id: &str,
        current: SlackChannelNotificationPreference,
        mode: SlackChannelNotificationMode,
    ) -> Result<(), String> {
        let sync = current.desktop_mode == current.mobile_mode;
        let mut updates = slack_notification_mode_updates(mode, sync);
        if current.muted
            && matches!(
                mode,
                SlackChannelNotificationMode::Everything | SlackChannelNotificationMode::Mentions
            )
        {
            updates.push(SlackNotificationPrefUpdate {
                name: "muted",
                value: "false",
                sync: None,
            });
        }
        self.api.post(
            "users.prefs.setNotifications",
            &[
                ("global", "false".to_string()),
                (
                    "channel_ids",
                    serde_json::to_string(&[conversation_id]).map_err(|error| {
                        format!("failed to encode Slack notification channel_ids: {error}")
                    })?,
                ),
                (
                    "prefs",
                    serde_json::to_string(&updates).map_err(|error| {
                        format!("failed to encode Slack notification preference updates: {error}")
                    })?,
                ),
                ("sync", "false".to_string()),
            ],
        )?;
        Ok(())
    }

    fn set_channel_muted(&self, conversation_id: &str, muted: bool) -> Result<(), String> {
        self.api.post(
            "users.prefs.setNotifications",
            &[
                ("name", "muted".to_string()),
                ("value", muted.to_string()),
                ("channel_id", conversation_id.to_string()),
                ("global", "false".to_string()),
                ("sync", "false".to_string()),
            ],
        )?;
        Ok(())
    }
}

impl SlackNotificationPreferences {
    pub(super) fn preference_for_channel(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Result<SlackChannelNotificationPreference, String> {
        let channel = self.channels.get(conversation_id);
        let desktop = channel
            .and_then(|channel| channel.desktop.as_deref())
            .unwrap_or(&self.global.global_desktop);
        let mobile = channel
            .and_then(|channel| channel.mobile.as_deref())
            .unwrap_or(&self.global.global_mobile);
        Ok(SlackChannelNotificationPreference {
            team_id: team_id.to_string(),
            conversation_id: conversation_id.to_string(),
            desktop_mode: slack_notification_mode(desktop, "desktop")?,
            mobile_mode: slack_notification_mode(mobile, "mobile")?,
            muted: channel.and_then(|channel| channel.muted).unwrap_or(false),
        })
    }
}

fn slack_notification_mode(
    value: &str,
    field: &str,
) -> Result<SlackChannelNotificationMode, String> {
    match value {
        "everything" => Ok(SlackChannelNotificationMode::Everything),
        "mentions_dms" => Ok(SlackChannelNotificationMode::Mentions),
        "nothing" => Ok(SlackChannelNotificationMode::Nothing),
        _ => Err(format!(
            "Slack all_notifications_prefs returned unsupported {field} mode {value:?}"
        )),
    }
}

fn slack_notification_mode_updates(
    mode: SlackChannelNotificationMode,
    sync: bool,
) -> Vec<SlackNotificationPrefUpdate> {
    let primary = SlackNotificationPrefUpdate {
        name: "desktop",
        value: match mode {
            SlackChannelNotificationMode::Everything => "everything",
            SlackChannelNotificationMode::Mentions => "mentions_dms",
            SlackChannelNotificationMode::Nothing => "nothing",
        },
        sync: Some(sync),
    };
    let mut updates = vec![primary];
    match mode {
        SlackChannelNotificationMode::Everything => {
            updates.push(SlackNotificationPrefUpdate {
                name: "follow_all_threads",
                value: "false",
                sync: None,
            });
            updates.push(SlackNotificationPrefUpdate {
                name: "badge_all_unreads",
                value: "false",
                sync: None,
            });
        }
        SlackChannelNotificationMode::Mentions => {
            updates.push(SlackNotificationPrefUpdate {
                name: "follow_all_threads",
                value: "false",
                sync: None,
            });
            updates.push(SlackNotificationPrefUpdate {
                name: "suppress_at_channel",
                value: "true",
                sync: None,
            });
        }
        SlackChannelNotificationMode::Nothing => {}
    }
    updates
}

fn notification_mutation_is_noop(
    preference: &SlackChannelNotificationPreference,
    mutation: SlackChannelNotificationMutation,
) -> bool {
    match mutation {
        SlackChannelNotificationMutation::SetMode(mode) => preference.desktop_mode == mode,
        SlackChannelNotificationMutation::SetMuted(muted) => preference.muted == muted,
    }
}

fn require_conversation_id(conversation_id: &str) -> Result<&str, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        Err("Slack channel notification conversation_id must not be empty".to_string())
    } else {
        Ok(conversation_id)
    }
}
