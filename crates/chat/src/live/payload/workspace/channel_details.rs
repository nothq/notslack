use crate::model::{
    SlackChannelCreator, SlackChannelDetailsSnapshot, SlackChannelMembership, SlackChannelText,
};
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::util::{slack_conversation_kind, slack_user_display_name, string_at};

impl SlackLiveWorkspaceLoader {
    pub fn load_channel_details(
        &self,
        conversation_id: &str,
    ) -> Result<SlackChannelDetailsSnapshot, String> {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("Slack channel details conversation_id must not be empty".to_string());
        }
        let response = self.load_conversation_detail(conversation_id)?;
        let channel = response
            .get("channel")
            .ok_or_else(|| "Slack conversations.info response omitted channel".to_string())?;
        let returned_conversation_id = string_at(channel, &["id"])
            .ok_or_else(|| "Slack conversations.info channel omitted id".to_string())?;
        if returned_conversation_id != conversation_id {
            return Err(format!(
                "Slack conversations.info returned channel {returned_conversation_id} for {conversation_id}"
            ));
        }
        if !slack_conversation_kind(channel).is_channel() {
            return Err(format!(
                "Slack conversations.info returned a non-channel conversation for {conversation_id}"
            ));
        }
        let name = string_at(channel, &["name"])
            .ok_or_else(|| "Slack conversations.info channel omitted name".to_string())?;
        let creator = string_at(channel, &["creator"])
            .map(|user_id| self.cached_channel_creator(user_id))
            .transpose()?;
        Ok(SlackChannelDetailsSnapshot {
            team_id: self.team_id.clone(),
            conversation_id: returned_conversation_id,
            name,
            text: SlackChannelText {
                topic: string_at(channel, &["topic", "value"]),
                purpose: string_at(channel, &["purpose", "value"]),
            },
            creator,
            created_at_unix_seconds: optional_unix_timestamp(channel, "created")?,
            membership: SlackChannelMembership {
                is_member: optional_bool(channel, "is_member")?,
                is_general: optional_bool(channel, "is_general")?,
                is_org_mandatory: optional_bool(channel, "is_org_mandatory")?,
            },
        })
    }

    fn cached_channel_creator(&self, user_id: String) -> Result<SlackChannelCreator, String> {
        let cached_users = self
            .user_cache
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?;
        let cached_user = cached_users.get(&user_id);
        Ok(SlackChannelCreator {
            display_name: cached_user.and_then(slack_user_display_name),
            is_deactivated: cached_user
                .and_then(|user| user.get("deleted"))
                .and_then(Value::as_bool),
            user_id,
        })
    }
}

fn optional_bool(value: &Value, field: &str) -> Result<Option<bool>, String> {
    let Some(value) = value.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| format!("Slack channel field {field} was not a boolean"))
}

fn optional_unix_timestamp(value: &Value, field: &str) -> Result<Option<i64>, String> {
    let Some(value) = value.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let timestamp = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        .ok_or_else(|| format!("Slack channel field {field} was not a Unix timestamp"))?;
    if timestamp < 0 {
        return Err(format!(
            "Slack channel field {field} was a negative Unix timestamp"
        ));
    }
    Ok(Some(timestamp))
}
