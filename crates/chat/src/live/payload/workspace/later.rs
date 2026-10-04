use std::{collections::BTreeSet, thread};

use crate::model::SlackMessageTimestamp;
use crate::model::{
    SlackFileId, SlackLaterContent, SlackLaterCursor, SlackLaterFile, SlackLaterFilter,
    SlackLaterHydratedItem, SlackLaterHydrationTarget, SlackLaterMessage, SlackLaterSnapshot,
};
use serde::Deserialize;
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::{
    message::slack_message_from_value_with_context_in_timezone,
    users::{
        load_slack_conversation_users_with_cache, load_slack_search_users_with_cache,
        ConversationUserLoadInput,
    },
    util::{slack_conversation_kind, slack_conversation_label, slack_user_display_name},
};

impl SlackLiveWorkspaceLoader {
    pub fn load_later(
        &self,
        filter: SlackLaterFilter,
        cursor: Option<&SlackLaterCursor>,
    ) -> Result<SlackLaterSnapshot, String> {
        self.sidebar_api.load_later_page(filter, cursor)
    }

    pub fn mutate_later_reminder(
        &self,
        mutation: &crate::model::SlackReminderMutation,
    ) -> Result<(), String> {
        self.sidebar_api.mutate_reminder(mutation)
    }

    pub fn hydrate_later_item(
        &self,
        target: SlackLaterHydrationTarget,
    ) -> Result<SlackLaterHydratedItem, String> {
        let key = target.key().clone();
        let content = match target {
            SlackLaterHydrationTarget::Message { reference, .. } => {
                SlackLaterContent::Message(Box::new(
                    self.hydrate_later_message(&reference.conversation_id, &reference.timestamp)?,
                ))
            }
            SlackLaterHydrationTarget::File { reference, .. } => {
                SlackLaterContent::File(Box::new(self.hydrate_later_file(&reference.file_id)?))
            }
        };
        Ok(SlackLaterHydratedItem { key, content })
    }

    fn hydrate_later_message(
        &self,
        conversation_id: &str,
        timestamp: &str,
    ) -> Result<SlackLaterMessage, String> {
        let sources = self.load_later_message_sources(conversation_id, timestamp)?;
        let LaterMessageSources {
            permalink,
            thread_timestamp,
            history,
            channel_info,
        } = sources;
        let users = load_slack_conversation_users_with_cache(
            &self.api,
            ConversationUserLoadInput {
                channel_info: &channel_info,
                history: &history,
            },
            &self.user_cache,
            &self.user_fetch_lock,
        )?;
        let self_user_id = self.load_self_user_id()?;
        let raw_message = exact_later_message(&history, timestamp)?;
        let message = slack_message_from_value_with_context_in_timezone(
            raw_message,
            &users,
            None,
            Some(&self_user_id),
            self.timezone.value,
        );
        if message.id != timestamp {
            return Err(format!(
                "Slack Later hydration returned message {} for requested {timestamp}",
                message.id
            ));
        }
        let channel = channel_info
            .get("channel")
            .ok_or_else(|| "Slack conversations.info response omitted channel".to_string())?;
        let conversation_name = slack_conversation_label(channel, &users).ok_or_else(|| {
            format!("Slack conversations.info omitted a label for {conversation_id}")
        })?;
        Ok(SlackLaterMessage {
            conversation_id: conversation_id.to_string(),
            conversation_kind: slack_conversation_kind(channel),
            conversation_name,
            timestamp: timestamp.to_string(),
            thread_timestamp,
            permalink,
            message,
        })
    }

    fn load_later_message_sources(
        &self,
        conversation_id: &str,
        timestamp: &str,
    ) -> Result<LaterMessageSources, String> {
        let permalink_payload = self.api.post(
            "chat.getPermalink",
            &[
                ("channel", conversation_id.to_string()),
                ("message_ts", timestamp.to_string()),
            ],
        )?;
        let permalink = serde_json::from_value::<SlackPermalinkResponse>(permalink_payload)
            .map_err(|error| format!("failed to decode Slack chat.getPermalink response: {error}"))?
            .permalink;
        let thread_timestamp = later_thread_timestamp(&permalink, timestamp)?;
        let (history_result, channel_result) = thread::scope(|scope| {
            let history = scope.spawn(|| {
                self.load_later_message_payload(conversation_id, timestamp, &thread_timestamp)
            });
            let channel = scope.spawn(|| self.load_conversation_detail(conversation_id));
            (
                history
                    .join()
                    .map_err(|_| "Slack Later message request thread panicked".to_string()),
                channel
                    .join()
                    .map_err(|_| "Slack Later conversation request thread panicked".to_string()),
            )
        });
        let history = history_result??;
        let channel_info = channel_result??;
        Ok(LaterMessageSources {
            thread_timestamp,
            permalink,
            history,
            channel_info,
        })
    }

    fn load_later_message_payload(
        &self,
        conversation_id: &str,
        timestamp: &str,
        thread_timestamp: &str,
    ) -> Result<Value, String> {
        let common = [
            ("channel", conversation_id.to_string()),
            ("include_all_metadata", "true".to_string()),
            ("inclusive", "true".to_string()),
        ];
        if thread_timestamp == timestamp {
            let mut parameters = common.to_vec();
            parameters.extend([
                ("limit", "1".to_string()),
                ("latest", timestamp.to_string()),
            ]);
            return self.api.post("conversations.history", &parameters);
        }
        let mut parameters = common.to_vec();
        parameters.extend([
            ("limit", "2".to_string()),
            ("ts", thread_timestamp.to_string()),
            ("oldest", timestamp.to_string()),
            ("latest", timestamp.to_string()),
        ]);
        self.api.post("conversations.replies", &parameters)
    }

    fn hydrate_later_file(&self, file_id: &str) -> Result<SlackLaterFile, String> {
        let file_id = SlackFileId::parse(file_id.to_string())
            .map_err(|error| format!("Slack Later file reference is invalid: {error}"))?;
        let metadata = self.load_file_metadata_entry(&file_id)?;
        let (owner_user_id, attachment) = metadata.into_owner_and_attachment();
        let users = match owner_user_id.as_ref() {
            Some(user_id) => load_slack_search_users_with_cache(
                &self.api,
                BTreeSet::from([user_id.clone()]),
                &self.user_cache,
                &self.user_fetch_lock,
                || self.ensure_user_directory_cache(),
            )?,
            None => Default::default(),
        };
        let owner_label = owner_user_id
            .as_ref()
            .and_then(|user_id| users.get(user_id))
            .and_then(slack_user_display_name);
        Ok(SlackLaterFile {
            file_id: file_id.as_str().to_string(),
            owner_user_id,
            owner_label,
            attachment,
        })
    }
}

struct LaterMessageSources {
    permalink: String,
    thread_timestamp: String,
    history: Value,
    channel_info: Value,
}

#[derive(Deserialize)]
struct SlackPermalinkResponse {
    permalink: String,
}

fn later_thread_timestamp(permalink: &str, message_timestamp: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(permalink)
        .map_err(|error| format!("Slack chat.getPermalink returned invalid permalink: {error}"))?;
    let thread_timestamp = url
        .query_pairs()
        .find_map(|(key, value)| (key == "thread_ts").then(|| value.into_owned()))
        .unwrap_or_else(|| message_timestamp.to_string());
    SlackMessageTimestamp::parse(&thread_timestamp)?;
    Ok(thread_timestamp)
}

fn exact_later_message(history: &Value, timestamp: &str) -> Result<Value, String> {
    history
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack Later message response omitted messages".to_string())?
        .iter()
        .find(|message| message.get("ts").and_then(Value::as_str) == Some(timestamp))
        .cloned()
        .ok_or_else(|| {
            format!("Slack Later message response omitted requested message {timestamp}")
        })
}
