pub(crate) mod attachments;
mod history;
mod rich_text;
mod send_receipt;

use std::collections::HashMap;

use crate::model::{
    SlackLaterState, SlackMessage, SlackMessageClientId, SlackReplyParticipant, SlackRichTextBody,
};
use chrono_tz::Tz;
use serde_json::Value;

use super::util::{
    format_slack_timestamp_in_timezone, initials, slack_user_avatar_image_url,
    slack_user_display_name, string_at, value_as_u32, SlackAvatarPurpose,
};

use attachments::slack_message_attachments;

use super::sidebar_dom::SlackSidebarSnapshot;

pub(super) use history::{
    merge_slack_conversation_history_pages, slack_conversation_history_next_cursor,
    slack_conversation_history_page, slack_conversation_history_page_state, slack_messages,
    slack_thread_messages, SlackConversationHistoryPagePayloads,
};
pub(crate) use rich_text::slack_message_body;
pub(in crate::live) use rich_text::slack_rich_text_body_from_blocks;
use rich_text::{slack_message_reactions, slack_rich_text_body};
pub(super) use send_receipt::{
    slack_message_send_receipt_from_payload, SlackMessageSendReceiptPayloads,
};

#[cfg(test)]
pub(crate) fn slack_message_from_value(
    message: Value,
    users: &HashMap<String, Value>,
) -> SlackMessage {
    slack_message_from_value_with_context(message, users, None, None)
}

#[cfg(test)]
pub(super) fn slack_message_from_value_with_context(
    message: Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
    self_user_id: Option<&str>,
) -> SlackMessage {
    slack_message_from_value_with_context_in_timezone(
        message,
        users,
        sidebar_snapshot,
        self_user_id,
        chrono_tz::UTC,
    )
}

pub(crate) fn slack_message_from_value_with_context_in_timezone(
    message: Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
    self_user_id: Option<&str>,
    timezone: Tz,
) -> SlackMessage {
    let user_id = message
        .get("user")
        .and_then(Value::as_str)
        .map(str::to_string);
    let user = user_id.as_deref().and_then(|user_id| users.get(user_id));
    let author = slack_message_author(&message, user_id.as_deref(), users);
    let attachments = slack_message_attachments(&message, users, sidebar_snapshot);
    let rich_body = slack_rich_text_body(&message, users, sidebar_snapshot);
    let body = rich_body
        .as_ref()
        .map(SlackRichTextBody::plain_text)
        .filter(|body| !body.is_empty())
        .unwrap_or_else(|| slack_message_body(&message, !attachments.is_empty()));
    let reply_count = slack_message_reply_count(&message);
    let reply_participants =
        slack_message_reply_participants(&message, users, reply_count.unwrap_or_default());
    SlackMessage {
        id: string_at(&message, &["ts"])
            .or_else(|| string_at(&message, &["client_msg_id"]))
            .unwrap_or_else(|| format!("message-{}", body.len())),
        client_message_id: string_at(&message, &["client_msg_id"]).map(|client_message_id| {
            SlackMessageClientId::new(client_message_id)
                .expect("non-empty Slack client message id must be valid")
        }),
        author: author.clone(),
        timestamp: string_at(&message, &["ts"])
            .map(|timestamp| format_slack_timestamp_in_timezone(&timestamp, timezone))
            .unwrap_or_default(),
        user_id,
        avatar_label: Some(initials(&author)),
        avatar_image_url: slack_message_avatar_image_url(&message, user),
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body,
        rich_body: rich_body.map(Box::new),
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: message
            .get("edited")
            .is_some()
            .then(|| "(edited)".to_string()),
        attachments,
        reactions: slack_message_reactions(&message, self_user_id),
        saved_state: slack_message_saved_state(&message),
        reply_count,
        latest_reply_timestamp: slack_message_latest_reply_timestamp(&message, timezone),
        reply_participants,
        replies: Vec::new(),
    }
}

fn slack_message_saved_state(message: &Value) -> Option<SlackLaterState> {
    let saved = message.get("saved")?;
    match (
        saved.get("state").and_then(Value::as_str),
        saved.get("todo_state").and_then(Value::as_str),
    ) {
        (Some("in_progress"), Some("saved")) => Some(SlackLaterState::InProgress),
        (Some("archived"), Some("saved")) => Some(SlackLaterState::Archived),
        (Some("completed"), Some("completed")) => Some(SlackLaterState::Completed),
        _ => None,
    }
}

fn slack_message_reply_count(message: &Value) -> Option<u32> {
    value_as_u32(message.get("reply_count")).filter(|count| *count > 0)
}

fn slack_message_latest_reply_timestamp(message: &Value, timezone: Tz) -> Option<String> {
    string_at(message, &["latest_reply"])
        .map(|timestamp| format_slack_timestamp_in_timezone(&timestamp, timezone))
        .filter(|timestamp| !timestamp.is_empty())
}

fn slack_message_reply_participants(
    message: &Value,
    users: &HashMap<String, Value>,
    reply_count: u32,
) -> Vec<SlackReplyParticipant> {
    let participant_limit = usize::try_from(reply_count)
        .expect("Slack reply count must fit the desktop target's usize");
    let Some(reply_users) = message.get("reply_users").and_then(Value::as_array) else {
        return Vec::new();
    };
    if participant_limit == 0 {
        return Vec::new();
    }
    let mut participants =
        Vec::<SlackReplyParticipant>::with_capacity(participant_limit.min(reply_users.len()));
    for user_id in reply_users.iter().filter_map(Value::as_str) {
        if participants
            .iter()
            .any(|participant| participant.user_id == user_id)
        {
            continue;
        }
        let user = users.get(user_id);
        let display_name = user
            .and_then(slack_user_display_name)
            .unwrap_or_else(|| user_id.to_string());
        participants.push(SlackReplyParticipant {
            user_id: user_id.to_string(),
            avatar_label: initials(&display_name),
            avatar_image_url: user
                .and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Message)),
            display_name,
        });
        if participants.len() == participant_limit {
            break;
        }
    }
    participants
}

fn slack_message_avatar_image_url(message: &Value, user: Option<&Value>) -> Option<String> {
    user.and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Message))
        .or_else(|| {
            [
                ["bot_profile", "icons", "image_72"].as_slice(),
                ["icons", "image_72"].as_slice(),
                ["bot_profile", "icons", "image_48"].as_slice(),
                ["icons", "image_48"].as_slice(),
                ["bot_profile", "icons", "image_36"].as_slice(),
                ["icons", "image_36"].as_slice(),
            ]
            .into_iter()
            .find_map(|path| string_at(message, path))
        })
}

fn slack_message_author(
    message: &Value,
    user_id: Option<&str>,
    users: &HashMap<String, Value>,
) -> String {
    let user = user_id.and_then(|user_id| users.get(user_id));
    message
        .get("username")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| user.and_then(slack_user_display_name))
        .unwrap_or_else(|| "Slack".to_string())
}

#[cfg(test)]
mod tests {
    use crate::live::payload::message::*;

    #[gpui::test]
    fn slack_message_avatar_image_uses_bot_icon_when_user_is_missing() {
        let message = slack_message_from_value(
            serde_json::json!({
                "ts": "1710000000.000100",
                "subtype": "bot_message",
                "username": "MetaBot",
                "icons": {
                    "image_48": "https://example.com/metabot.png"
                },
                "text": "hello"
            }),
            &HashMap::new(),
        );

        assert_eq!(
            message.avatar_image_url.as_deref(),
            Some("https://example.com/metabot.png")
        );
    }

    #[gpui::test]
    fn slack_message_preserves_client_message_id() {
        let message = slack_message_from_value(
            serde_json::json!({
                "ts": "1710000000.000100",
                "client_msg_id": "0a45a086-f507-49f0-a68b-85c04fc2804e",
                "user": "U_SELF",
                "text": "hello"
            }),
            &HashMap::new(),
        );

        assert_eq!(
            message
                .client_message_id
                .as_ref()
                .map(SlackMessageClientId::as_str),
            Some("0a45a086-f507-49f0-a68b-85c04fc2804e")
        );
    }

    #[gpui::test]
    fn slack_send_receipt_injects_the_requested_client_message_id() {
        let client_message_id =
            SlackMessageClientId::new("0a45a086-f507-49f0-a68b-85c04fc2804e".to_string())
                .expect("test client message id must be valid");
        let response = serde_json::json!({
            "channel": "C_DESIGN",
            "ts": "1710000000.000100",
            "message": {
                "team": "TTEST",
                "user": "U_SELF",
                "ts": "1710000000.000100",
                "text": "hello"
            }
        });

        let receipt = slack_message_send_receipt_from_payload(SlackMessageSendReceiptPayloads {
            team_id: "TTEST",
            conversation_id: "C_DESIGN",
            client_message_id: &client_message_id,
            response: &response,
            users: &HashMap::new(),
            self_user_id: "U_SELF",
            self_timezone_id: None,
            timezone: chrono_tz::UTC,
        })
        .expect("matching Slack send response must produce a receipt");

        assert_eq!(
            receipt.message.client_message_id.as_ref(),
            Some(&client_message_id)
        );
    }

    #[gpui::test]
    fn slack_send_receipt_rejects_a_different_client_message_id() {
        let client_message_id =
            SlackMessageClientId::new("0a45a086-f507-49f0-a68b-85c04fc2804e".to_string())
                .expect("test client message id must be valid");
        let response = serde_json::json!({
            "channel": "C_DESIGN",
            "ts": "1710000000.000100",
            "message": {
                "team": "TTEST",
                "user": "U_SELF",
                "ts": "1710000000.000100",
                "client_msg_id": "719459dd-5442-41f0-9db5-29f870083bc7",
                "text": "hello"
            }
        });

        let error = slack_message_send_receipt_from_payload(SlackMessageSendReceiptPayloads {
            team_id: "TTEST",
            conversation_id: "C_DESIGN",
            client_message_id: &client_message_id,
            response: &response,
            users: &HashMap::new(),
            self_user_id: "U_SELF",
            self_timezone_id: None,
            timezone: chrono_tz::UTC,
        })
        .expect_err("mismatched Slack client message id must fail");

        assert!(error.contains("different client message id"));
    }

    #[gpui::test]
    fn slack_client_message_id_deserialization_rejects_empty_values() {
        let error = serde_json::from_value::<SlackMessageClientId>(serde_json::json!(""))
            .expect_err("empty cached Slack client message id must fail");

        assert!(error.to_string().contains("must not be empty"));
    }
}
