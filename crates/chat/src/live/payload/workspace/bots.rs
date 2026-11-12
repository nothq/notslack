//! Naming the app behind a message Slack sent with nothing but a `bot_id`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{load_slack_quick_search_bots, SlackLiveWorkspaceLoader};

impl SlackLiveWorkspaceLoader {
    /// Fills in `bot_profile` on the messages of a history or replies payload.
    pub(super) fn fill_history_bot_profiles(&self, history: &mut Value) {
        let messages = history.get_mut("messages").and_then(Value::as_array_mut);
        self.fill_missing_bot_profiles(messages.into_iter().flatten());
    }

    /// Some app messages carry neither `username` nor `bot_profile`, so there
    /// is no name or icon to show. Slack's `bots.info` answer has the shape of
    /// a `bot_profile`, and takes its place. A bot that cannot be looked up
    /// leaves its messages as they came.
    pub(super) fn fill_missing_bot_profiles<'a>(
        &self,
        messages: impl IntoIterator<Item = &'a mut Value>,
    ) {
        for message in messages {
            let Some(bot_id) = anonymous_bot_id(message) else {
                continue;
            };
            let bots = load_slack_quick_search_bots(
                &self.api,
                BTreeSet::from([bot_id.clone()]),
                &self.bot_cache,
                &self.bot_fetch_lock,
            );
            match bots {
                Ok(mut bots) => {
                    if let Some(bot) = bots.remove(&bot_id) {
                        message["bot_profile"] = bot;
                    }
                }
                Err(error) => eprintln!("Slack bot {bot_id} lookup failed: {error}"),
            }
        }
    }
}

/// The bot behind a message that names no author of its own.
fn anonymous_bot_id(message: &Value) -> Option<String> {
    let has = |key: &str| message.get(key).is_some_and(|value| !value.is_null());
    if has("username") || has("bot_profile") || has("user") {
        return None;
    }
    message
        .get("bot_id")
        .and_then(Value::as_str)
        .filter(|bot_id| !bot_id.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::anonymous_bot_id;

    #[test]
    fn a_message_with_only_a_bot_id_needs_its_bot_looked_up() {
        let message = json!({"subtype": "bot_message", "bot_id": "B1", "text": "deployed"});
        assert_eq!(anonymous_bot_id(&message), Some("B1".to_string()));
    }

    #[test]
    fn a_message_that_already_names_its_author_is_left_alone() {
        for message in [
            json!({"bot_id": "B1", "username": "Deploys"}),
            json!({"bot_id": "B1", "bot_profile": {"name": "Deploys"}}),
            json!({"bot_id": "B1", "user": "U1"}),
            json!({"user": "U1", "text": "hello"}),
        ] {
            assert_eq!(anonymous_bot_id(&message), None, "{message}");
        }
    }
}
