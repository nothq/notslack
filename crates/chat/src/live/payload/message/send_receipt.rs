use std::collections::HashMap;

use crate::model::{SlackMessage, SlackMessageClientId, SlackMessageSendReceipt};
use chrono_tz::Tz;
use serde::Deserialize;
use serde_json::Value;

use super::slack_message_from_value_with_context_in_timezone;

pub(in crate::live::payload) struct SlackMessageSendReceiptPayloads<'a> {
    pub(in crate::live::payload) team_id: &'a str,
    pub(in crate::live::payload) conversation_id: &'a str,
    pub(in crate::live::payload) client_message_id: &'a SlackMessageClientId,
    pub(in crate::live::payload) response: &'a Value,
    pub(in crate::live::payload) users: &'a HashMap<String, Value>,
    pub(in crate::live::payload) self_user_id: &'a str,
    pub(in crate::live::payload) self_timezone_id: Option<&'a str>,
    pub(in crate::live::payload) timezone: Tz,
}

#[derive(Deserialize)]
struct SlackPostMessageResponseWire {
    channel: String,
    ts: String,
    message: Value,
}

#[derive(Deserialize)]
struct SlackPostedMessageIdentityWire {
    #[serde(default)]
    team: Option<String>,
    user: String,
    ts: String,
}

pub(in crate::live::payload) fn slack_message_send_receipt_from_payload(
    payloads: SlackMessageSendReceiptPayloads<'_>,
) -> Result<SlackMessageSendReceipt, String> {
    let SlackMessageSendReceiptPayloads {
        team_id,
        conversation_id,
        client_message_id,
        response,
        users,
        self_user_id,
        self_timezone_id,
        timezone,
    } = payloads;
    let response =
        validated_post_message_response(response, team_id, conversation_id, self_user_id)?;
    let mut message = slack_message_from_value_with_context_in_timezone(
        response.message,
        users,
        None,
        Some(self_user_id),
        timezone,
    );
    match message.client_message_id.as_ref() {
        Some(response_client_message_id) if response_client_message_id != client_message_id => {
            return Err(
                "Slack chat.postMessage response returned a different client message id"
                    .to_string(),
            );
        }
        Some(_) => {}
        None => message.client_message_id = Some(client_message_id.clone()),
    }
    if !valid_posted_message(&message, &response.ts, self_user_id) {
        return Err(
            "Slack chat.postMessage response did not contain one visible message from the authenticated user"
                .to_string(),
        );
    }
    Ok(SlackMessageSendReceipt {
        team_id: team_id.to_string(),
        conversation_id: conversation_id.to_string(),
        self_timezone_id: self_timezone_id.map(str::to_string),
        timestamp: response.ts,
        self_user_id: self_user_id.to_string(),
        message,
    })
}

fn validated_post_message_response(
    response: &Value,
    team_id: &str,
    conversation_id: &str,
    self_user_id: &str,
) -> Result<SlackPostMessageResponseWire, String> {
    let response = serde_json::from_value::<SlackPostMessageResponseWire>(response.clone())
        .map_err(|error| format!("failed to decode Slack chat.postMessage response: {error}"))?;
    if response.channel != conversation_id {
        return Err(format!(
            "Slack chat.postMessage returned channel {} for requested channel {conversation_id}",
            response.channel
        ));
    }
    let identity =
        serde_json::from_value::<SlackPostedMessageIdentityWire>(response.message.clone())
            .map_err(|error| {
                format!(
                    "failed to decode Slack chat.postMessage response message identity: {error}"
                )
            })?;
    if identity
        .team
        .as_deref()
        .is_some_and(|response_team_id| response_team_id != team_id)
    {
        return Err(format!(
            "Slack chat.postMessage returned a message from a different team than {team_id}"
        ));
    }
    if identity.user != self_user_id {
        return Err(format!(
            "Slack chat.postMessage returned author {} for authenticated user {self_user_id}",
            identity.user
        ));
    }
    if response.ts != identity.ts {
        return Err(
            "Slack chat.postMessage returned mismatched top-level and message timestamps"
                .to_string(),
        );
    }
    crate::model::SlackMessageTimestamp::parse(&response.ts).map_err(|error| {
        format!("Slack chat.postMessage returned an invalid message timestamp: {error}")
    })?;
    Ok(response)
}

fn valid_posted_message(message: &SlackMessage, timestamp: &str, self_user_id: &str) -> bool {
    message.id == timestamp
        && message.user_id.as_deref() == Some(self_user_id)
        && !message.author.trim().is_empty()
        && (!message.body.is_empty() || !message.attachments.is_empty())
}
