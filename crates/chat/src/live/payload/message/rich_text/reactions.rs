use crate::model::SlackReaction;
use serde_json::Value;

use crate::live::payload::util::{string_at, value_as_u32};

pub(in crate::live::payload::message) fn slack_message_reactions(
    message: &Value,
    self_user_id: Option<&str>,
) -> Vec<SlackReaction> {
    message
        .get("reactions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reaction| {
            let emoji = string_at(reaction, &["name"]).filter(|name| !name.is_empty())?;
            let active = self_user_id.is_some_and(|self_user_id| {
                reaction
                    .get("users")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|user_id| user_id.as_str() == Some(self_user_id))
            });
            Some(SlackReaction {
                emoji,
                count: value_as_u32(reaction.get("count")).unwrap_or(0),
                active,
            })
        })
        .collect()
}
