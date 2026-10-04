use std::str::FromStr;

use crate::model::{SlackPreferredSkinTone, SlackSkinTone};
use serde_json::Value;

use super::{notifications::SlackNotificationPreferences, SlackLiveWorkspaceLoader};

const ALL_NOTIFICATIONS_PREFS: &str = "all_notifications_prefs";
const PREFERRED_SKIN_TONE: &str = "preferred_skin_tone";
const BOOTSTRAP_USER_PREFERENCES: &str = "all_notifications_prefs,preferred_skin_tone";

#[derive(Clone)]
pub(super) struct SlackUserPreferences {
    pub(super) notifications: SlackNotificationPreferences,
    preferred_skin_tone: Option<SlackSkinTone>,
}

impl SlackLiveWorkspaceLoader {
    pub fn load_preferred_skin_tone(&self) -> Result<SlackPreferredSkinTone, String> {
        let preferences = self.user_preferences()?;
        Ok(SlackPreferredSkinTone {
            team_id: self.team_id.clone(),
            selection: preferences.preferred_skin_tone,
        })
    }

    pub fn mutate_preferred_skin_tone(
        &self,
        selection: SlackSkinTone,
    ) -> Result<SlackPreferredSkinTone, String> {
        let _mutation_guard = self
            .user_preferences_mutation_lock
            .lock()
            .map_err(|_| "Slack user-preference mutation mutex poisoned".to_string())?;
        let current = self.user_preferences()?;
        if current.preferred_skin_tone == Some(selection) {
            return Ok(SlackPreferredSkinTone {
                team_id: self.team_id.clone(),
                selection: Some(selection),
            });
        }

        self.api.post(
            "users.prefs.set",
            &[
                ("name", PREFERRED_SKIN_TONE.to_string()),
                ("value", selection.preference_value().to_string()),
            ],
        )?;
        let mut next = current;
        next.preferred_skin_tone = Some(selection);
        *self
            .user_preferences_cache
            .lock()
            .map_err(|_| "Slack user-preferences cache mutex poisoned".to_string())? = Some(next);
        Ok(SlackPreferredSkinTone {
            team_id: self.team_id.clone(),
            selection: Some(selection),
        })
    }

    pub(super) fn user_preferences(&self) -> Result<SlackUserPreferences, String> {
        if let Some(preferences) = self.cached_user_preferences()? {
            return Ok(preferences);
        }

        let _fetch_guard = self
            .user_preferences_fetch_lock
            .lock()
            .map_err(|_| "Slack user-preferences fetch mutex poisoned".to_string())?;
        if let Some(preferences) = self.cached_user_preferences()? {
            return Ok(preferences);
        }

        let payload = self.api.post(
            "users.prefs.get",
            &[("prefs", BOOTSTRAP_USER_PREFERENCES.to_string())],
        )?;
        let preferences = parse_user_preferences(&payload)?;
        *self
            .user_preferences_cache
            .lock()
            .map_err(|_| "Slack user-preferences cache mutex poisoned".to_string())? =
            Some(preferences.clone());
        Ok(preferences)
    }

    fn cached_user_preferences(&self) -> Result<Option<SlackUserPreferences>, String> {
        self.user_preferences_cache
            .lock()
            .map_err(|_| "Slack user-preferences cache mutex poisoned".to_string())
            .map(|preferences| preferences.clone())
    }
}

fn parse_user_preferences(payload: &Value) -> Result<SlackUserPreferences, String> {
    let preferences = payload
        .get("prefs")
        .and_then(Value::as_object)
        .ok_or_else(|| "Slack users.prefs.get response omitted prefs".to_string())?;
    let serialized_notifications = preferences
        .get(ALL_NOTIFICATIONS_PREFS)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "Slack users.prefs.get response omitted prefs.all_notifications_prefs".to_string()
        })?;
    let notifications = serde_json::from_str::<SlackNotificationPreferences>(
        serialized_notifications,
    )
    .map_err(|error| {
        format!("failed to decode Slack users.prefs.get all_notifications_prefs: {error}")
    })?;
    let preferred_skin_tone = parse_optional_skin_tone(preferences.get(PREFERRED_SKIN_TONE))?;
    Ok(SlackUserPreferences {
        notifications,
        preferred_skin_tone,
    })
}

fn parse_optional_skin_tone(value: Option<&Value>) -> Result<Option<SlackSkinTone>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => SlackSkinTone::from_str(value).map(Some),
        Some(_) => Err(
            "Slack users.prefs.get returned preferred_skin_tone with a non-string value"
                .to_string(),
        ),
    }
}
