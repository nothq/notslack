use std::{collections::HashMap, thread};

use super::{
    load_slack_conversation_users_with_cache, ops, slack_thread_from_payloads,
    slack_thread_reply_receipt_from_payload, ConversationUserLoadInput, SlackLiveWorkspaceLoader,
    SlackMessageDraft, SlackMessageTimestamp, SlackThreadPayloads, SlackThreadReplyPayloads,
    SlackThreadReplyReceipt, SlackThreadSnapshot, SLACK_THREAD_PAGE_SIZE,
};
use crate::model::{SlackThreadLoad, SlackThreadReadTarget, SlackThreadReplyTarget};

impl SlackLiveWorkspaceLoader {
    pub fn mark_thread_read(&self, target: &SlackThreadReadTarget) -> Result<(), String> {
        self.sidebar_api.mark_thread_read(target)
    }

    pub fn load_thread(
        &self,
        conversation_id: &str,
        thread_timestamp: &SlackMessageTimestamp,
        cursor: Option<&str>,
    ) -> Result<SlackThreadSnapshot, String> {
        self.load_thread_with_read_metadata(conversation_id, thread_timestamp, cursor)
            .map(|load| load.snapshot)
    }

    pub fn load_thread_with_read_metadata(
        &self,
        conversation_id: &str,
        thread_timestamp: &SlackMessageTimestamp,
        cursor: Option<&str>,
    ) -> Result<SlackThreadLoad, String> {
        let thread_timestamp = thread_timestamp.as_str().to_string();
        let cursor = cursor.map(str::to_string);
        let (replies_result, channel_info_result) = thread::scope(|scope| {
            let replies = scope.spawn(|| {
                let mut parameters = vec![
                    ("channel", conversation_id.to_string()),
                    ("ts", thread_timestamp.clone()),
                    ("limit", SLACK_THREAD_PAGE_SIZE.to_string()),
                    ("include_all_metadata", "true".to_string()),
                ];
                if let Some(cursor) = cursor.as_ref() {
                    parameters.push(("cursor", cursor.clone()));
                }
                self.api.post("conversations.replies", &parameters)
            });
            let channel_info = scope.spawn(|| self.load_conversation_detail(conversation_id));
            (
                replies
                    .join()
                    .map_err(|_| "Slack conversations.replies request thread panicked".to_string()),
                channel_info
                    .join()
                    .map_err(|_| "Slack conversations.info request thread panicked".to_string()),
            )
        });
        let replies = replies_result??;
        let channel_info = channel_info_result??;
        let users = load_slack_conversation_users_with_cache(
            &self.api,
            ConversationUserLoadInput {
                channel_info: &channel_info,
                history: &replies,
            },
            &self.user_cache,
            &self.user_fetch_lock,
        )?;
        let (self_user_id, _) = self.load_self_user()?;
        let sidebar_snapshot = self.cached_sidebar_snapshot()?;
        slack_thread_from_payloads(SlackThreadPayloads {
            team_id: &self.team_id,
            conversation_id,
            thread_timestamp: &thread_timestamp,
            cursor: cursor.as_deref(),
            channel_info: &channel_info,
            replies: &replies,
            users: &users,
            sidebar_snapshot: sidebar_snapshot.as_ref(),
            self_user_id: Some(&self_user_id),
            self_timezone_id: self.timezone.id.as_deref(),
            timezone: self.timezone.value,
        })
    }

    pub(crate) fn load_thread_containing_reply(
        &self,
        conversation_id: &str,
        thread_timestamp: &SlackMessageTimestamp,
        reply_timestamp: &SlackMessageTimestamp,
    ) -> Result<SlackThreadSnapshot, String> {
        let thread_timestamp = thread_timestamp.as_str().to_string();
        let reply_timestamp = reply_timestamp.as_str().to_string();
        let (replies_result, channel_info_result) = thread::scope(|scope| {
            let replies = scope.spawn(|| {
                let parameters = containing_reply_parameters(
                    conversation_id,
                    &thread_timestamp,
                    &reply_timestamp,
                );
                self.api.post("conversations.replies", &parameters)
            });
            let channel_info = scope.spawn(|| self.load_conversation_detail(conversation_id));
            (
                replies
                    .join()
                    .map_err(|_| "Slack conversations.replies request thread panicked".to_string()),
                channel_info
                    .join()
                    .map_err(|_| "Slack conversations.info request thread panicked".to_string()),
            )
        });
        let replies = replies_result??;
        let channel_info = channel_info_result??;
        let users = load_slack_conversation_users_with_cache(
            &self.api,
            ConversationUserLoadInput {
                channel_info: &channel_info,
                history: &replies,
            },
            &self.user_cache,
            &self.user_fetch_lock,
        )?;
        let (self_user_id, _) = self.load_self_user()?;
        let sidebar_snapshot = self.cached_sidebar_snapshot()?;
        slack_thread_from_payloads(SlackThreadPayloads {
            team_id: &self.team_id,
            conversation_id,
            thread_timestamp: &thread_timestamp,
            cursor: None,
            channel_info: &channel_info,
            replies: &replies,
            users: &users,
            sidebar_snapshot: sidebar_snapshot.as_ref(),
            self_user_id: Some(&self_user_id),
            self_timezone_id: self.timezone.id.as_deref(),
            timezone: self.timezone.value,
        })
        .map(|load| load.snapshot)
    }

    pub fn post_thread_reply(
        &self,
        target: SlackThreadReplyTarget<'_>,
        client_message_id: &crate::model::SlackMessageClientId,
        draft: &SlackMessageDraft,
    ) -> Result<SlackThreadReplyReceipt, String> {
        let SlackThreadReplyTarget {
            conversation_id,
            thread_timestamp,
            broadcast,
        } = target;
        let (self_user_id, self_user) = self.load_self_user()?;
        let users = HashMap::from([(self_user_id.clone(), self_user)]);
        let response = ops::post_thread_reply(&self.api, target, client_message_id, draft)?;
        slack_thread_reply_receipt_from_payload(SlackThreadReplyPayloads {
            team_id: &self.team_id,
            conversation_id,
            thread_timestamp,
            client_message_id,
            broadcast,
            response: &response,
            users: &users,
            self_user_id: &self_user_id,
            self_timezone_id: self.timezone.id.as_deref(),
            timezone: self.timezone.value,
        })
    }
}

fn containing_reply_parameters(
    conversation_id: &str,
    thread_timestamp: &str,
    reply_timestamp: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("channel", conversation_id.to_string()),
        ("ts", thread_timestamp.to_string()),
        ("oldest", reply_timestamp.to_string()),
        ("latest", reply_timestamp.to_string()),
        ("inclusive", "true".to_string()),
        ("limit", SLACK_THREAD_PAGE_SIZE.to_string()),
        ("include_all_metadata", "true".to_string()),
    ]
}
