use serde_json::Value;

use super::types::{SlackClientCountsResponse, SlackCountConversation};
use crate::model::{SlackDirectMessageUnreadState, SlackMessageTimestamp};

impl SlackClientCountsResponse {
    pub(super) fn conversations(&self) -> impl Iterator<Item = &SlackCountConversation> {
        self.channels
            .iter()
            .chain(self.ims.iter())
            .chain(self.mpims.iter())
    }

    pub(super) fn has_unread_state_fields(&self) -> bool {
        self.conversations()
            .any(SlackCountConversation::has_unread_state_field)
    }

    pub(super) fn activity_count(&self) -> Option<u32> {
        self.activity_v2.as_ref().map(|activity| {
            activity
                .values()
                .filter_map(|value| value_as_u32(Some(value)))
                .sum()
        })
    }

    pub(super) fn dms_unread_messages(&self) -> Result<Option<u32>, String> {
        let total = self
            .ims
            .iter()
            .chain(self.mpims.iter())
            .filter_map(SlackCountConversation::display_count)
            .try_fold(0_u32, |total, count| total.checked_add(count))
            .ok_or_else(|| {
                "Slack client.counts DM unread message total exceeded u32".to_string()
            })?;
        Ok((total > 0).then_some(total))
    }

    pub(super) fn direct_message_unread_states(
        &self,
    ) -> Result<Vec<SlackDirectMessageUnreadState>, String> {
        self.ims
            .iter()
            .chain(self.mpims.iter())
            .map(|conversation| {
                Ok(SlackDirectMessageUnreadState {
                    conversation_id: conversation.id.clone(),
                    unread: conversation.unread(),
                    display_count: conversation.display_count(),
                    latest_message_timestamp: conversation.latest_message_timestamp()?,
                })
            })
            .collect()
    }
}

impl SlackCountConversation {
    pub(super) fn unread(&self) -> bool {
        self.unread_count() > 0
            || self.has_unreads == Some(true)
            || self.is_unread == Some(true)
            || self.latest_after_last_read()
    }

    pub(super) fn display_count(&self) -> Option<u32> {
        value_as_u32(self.unread_count_display.as_ref())
            .filter(|count| *count > 0)
            .or_else(|| self.mention_display_count())
    }

    pub(super) fn mention_display_count(&self) -> Option<u32> {
        value_as_u32(self.mention_count.as_ref()).filter(|count| *count > 0)
    }

    pub(super) fn latest_message_timestamp(&self) -> Result<Option<SlackMessageTimestamp>, String> {
        let Some(value) = self.latest.as_ref() else {
            return Ok(None);
        };
        if value.is_null() {
            return Ok(None);
        }
        let value = value.as_str().ok_or_else(|| {
            format!(
                "Slack client.counts returned a non-string latest timestamp for {}",
                self.id
            )
        })?;
        SlackMessageTimestamp::parse(value)
            .map(Some)
            .map_err(|error| format!("Slack client.counts latest for {}: {error}", self.id))
    }

    fn unread_count(&self) -> u32 {
        value_as_u32(self.unread_count.as_ref())
            .or_else(|| value_as_u32(self.mention_count.as_ref()))
            .or_else(|| value_as_u32(self.unread_count_display.as_ref()))
            .unwrap_or(0)
    }

    fn latest_after_last_read(&self) -> bool {
        let Some(latest) = timestamp_value(self.latest.as_ref()) else {
            return false;
        };
        let last_read = timestamp_value(self.last_read.as_ref()).unwrap_or_default();
        latest > last_read
    }

    fn has_unread_state_field(&self) -> bool {
        self.unread_count.is_some()
            || self.unread_count_display.is_some()
            || self.mention_count.is_some()
            || self.has_unreads.is_some()
            || self.is_unread.is_some()
            || self.latest.is_some()
            || self.last_read.is_some()
    }
}

pub(super) fn value_as_u32(value: Option<&Value>) -> Option<u32> {
    let value = value?;
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_str()?.parse::<u32>().ok())
}

fn timestamp_value(value: Option<&Value>) -> Option<u128> {
    let value = value?;
    let value = value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_u64().map(|value| value.to_string()))?;
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u128>().ok()
}
