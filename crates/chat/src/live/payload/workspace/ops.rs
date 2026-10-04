mod drafts;
mod file_shares;
mod files;

use crate::model::{
    SlackMessageDraft, SlackMessageForwardReceipt, SlackMessageTimestamp, SlackReactionMutation,
    SlackReactionName, SlackSavedMessageMutation, SlackStarMutation, SlackThreadReplyTarget,
};
use serde::Deserialize;
use serde_json::Value;

use crate::live::{api::SlackApiClient, payload::message_draft::slack_message_blocks_json};

pub(super) use drafts::{
    create_draft, create_scheduled_draft, delete_draft, update_draft, update_scheduled_draft,
    SlackScheduledDraftMutationOutcome,
};
pub(super) use file_shares::{share_files, SlackFileSharePostOutcome};
pub(super) use files::{
    conversations_contain_id, join_slack_request, load_attachment_preview, load_profile,
    upload_files, workspace_name,
};

pub(super) fn post_message(
    api: &SlackApiClient,
    conversation_id: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &SlackMessageDraft,
) -> Result<Value, String> {
    post_message_payload(api, conversation_id, client_message_id, draft, None)
}

pub(super) fn post_thread_reply(
    api: &SlackApiClient,
    target: SlackThreadReplyTarget<'_>,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &SlackMessageDraft,
) -> Result<Value, String> {
    let SlackThreadReplyTarget {
        conversation_id,
        thread_timestamp,
        broadcast,
    } = target;
    post_message_payload(
        api,
        conversation_id,
        client_message_id,
        draft,
        Some((thread_timestamp, broadcast)),
    )
}

fn post_message_payload(
    api: &SlackApiClient,
    conversation_id: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &SlackMessageDraft,
    thread: Option<(&SlackMessageTimestamp, bool)>,
) -> Result<Value, String> {
    let fallback_text = draft.fallback_text();
    if fallback_text.trim().is_empty() {
        return Err("cannot send an empty Slack message".to_string());
    }
    let mut parameters = vec![
        ("channel", conversation_id.to_string()),
        ("text", fallback_text.to_string()),
        ("client_msg_id", client_message_id.as_str().to_string()),
    ];
    if let Some(blocks) = slack_message_blocks_json(draft)? {
        parameters.push(("blocks", blocks));
    }
    if let Some((thread_timestamp, broadcast)) = thread {
        parameters.push(("thread_ts", thread_timestamp.as_str().to_string()));
        if broadcast {
            parameters.push(("reply_broadcast", "true".to_string()));
        }
    }
    api.post("chat.postMessage", &parameters)
}

pub(super) fn forward_message(
    api: &SlackApiClient,
    source_conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
    destination_conversation_id: &str,
    note: Option<&SlackMessageDraft>,
) -> Result<SlackMessageForwardReceipt, String> {
    let source_conversation_id =
        require_message_conversation_id("forward source", source_conversation_id)?;
    let destination_conversation_id =
        require_message_conversation_id("forward destination", destination_conversation_id)?;
    let blocks = note
        .map(slack_message_blocks_json)
        .transpose()?
        .flatten()
        .unwrap_or_else(|| "[]".to_string());
    let mut parameters = vec![
        ("timestamp", message_timestamp.as_str().to_string()),
        ("channel", source_conversation_id.to_string()),
        ("blocks", blocks),
        ("share_channel", destination_conversation_id.to_string()),
        ("pending_slug_urls", "[]".to_string()),
        ("skip_dlp_user_warning", "false".to_string()),
    ];
    if let Some(note) = note {
        parameters.push(("text", note.fallback_text().to_string()));
    }
    let payload = api.post("chat.shareMessage", &parameters)?;
    let response = serde_json::from_value::<SlackForwardMessageResponse>(payload)
        .map_err(|error| format!("failed to decode Slack chat.shareMessage response: {error}"))?;
    if let Some(timestamp) = response.timestamp.as_deref() {
        SlackMessageTimestamp::parse(timestamp).map_err(|error| {
            format!("Slack chat.shareMessage returned an invalid timestamp: {error}")
        })?;
    }
    Ok(SlackMessageForwardReceipt {
        destination_conversation_id: destination_conversation_id.to_string(),
        timestamp: response.timestamp,
    })
}

pub(super) fn mutate_reaction(
    api: &SlackApiClient,
    target: &crate::model::SlackReactionTarget,
    reaction_name: &SlackReactionName,
    mutation: SlackReactionMutation,
) -> Result<(), String> {
    let method = match mutation {
        SlackReactionMutation::Add => "reactions.add",
        SlackReactionMutation::Remove => "reactions.remove",
    };
    api.post(
        method,
        &[
            ("channel", target.conversation_id().to_string()),
            ("timestamp", target.message_timestamp().as_str().to_string()),
            ("name", reaction_name.as_str().to_string()),
        ],
    )?;
    Ok(())
}

pub(super) fn message_permalink(
    api: &SlackApiClient,
    conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
) -> Result<String, String> {
    let conversation_id = require_message_conversation_id("permalink", conversation_id)?;
    let payload = api.post(
        "chat.getPermalink",
        &[
            ("channel", conversation_id.to_string()),
            ("message_ts", message_timestamp.as_str().to_string()),
        ],
    )?;
    let response = serde_json::from_value::<SlackMessagePermalinkResponse>(payload)
        .map_err(|error| format!("failed to decode Slack chat.getPermalink response: {error}"))?;
    if response.channel != conversation_id {
        return Err(format!(
            "Slack chat.getPermalink returned channel {} for requested channel {conversation_id}",
            response.channel
        ));
    }
    if response.permalink.trim().is_empty() {
        return Err("Slack chat.getPermalink response returned an empty permalink".to_string());
    }
    Ok(response.permalink)
}

pub(super) fn update_message(
    api: &SlackApiClient,
    conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
    draft: &SlackMessageDraft,
) -> Result<(), String> {
    let conversation_id = require_message_conversation_id("edit", conversation_id)?;
    let mut parameters = vec![
        ("channel", conversation_id.to_string()),
        ("ts", message_timestamp.as_str().to_string()),
        ("text", draft.fallback_text().to_string()),
    ];
    if let Some(blocks) = slack_message_blocks_json(draft)? {
        parameters.push(("blocks", blocks));
    }
    let payload = api.post("chat.update", &parameters)?;
    validate_message_mutation_response("chat.update", payload, conversation_id, message_timestamp)
}

pub(super) fn delete_message(
    api: &SlackApiClient,
    conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
) -> Result<(), String> {
    let conversation_id = require_message_conversation_id("delete", conversation_id)?;
    let payload = api.post(
        "chat.delete",
        &[
            ("channel", conversation_id.to_string()),
            ("ts", message_timestamp.as_str().to_string()),
        ],
    )?;
    validate_message_mutation_response("chat.delete", payload, conversation_id, message_timestamp)
}

pub(super) fn mutate_saved_message(
    api: &SlackApiClient,
    conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
    mutation: SlackSavedMessageMutation,
) -> Result<(), String> {
    let conversation_id = require_message_conversation_id("save", conversation_id)?;
    let mut parameters = vec![
        ("item_type", "message".to_string()),
        ("item_id", conversation_id.to_string()),
        ("ts", message_timestamp.as_str().to_string()),
    ];
    let method = match mutation {
        SlackSavedMessageMutation::Save => {
            parameters.push(("date_due", "0".to_string()));
            parameters.push(("todo_state", "saved".to_string()));
            "saved.add"
        }
        SlackSavedMessageMutation::Remove => "saved.delete",
    };
    api.post(method, &parameters)?;
    Ok(())
}

pub(super) fn mutate_channel_star(
    api: &SlackApiClient,
    conversation_id: &str,
    mutation: SlackStarMutation,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Slack star conversation_id must not be empty".to_string());
    }
    let method = match mutation {
        SlackStarMutation::Add => "stars.add",
        SlackStarMutation::Remove => "stars.remove",
    };
    api.post(method, &[("channel", conversation_id.to_string())])?;
    Ok(())
}

#[derive(Deserialize)]
struct SlackMessagePermalinkResponse {
    channel: String,
    permalink: String,
}

#[derive(Deserialize)]
struct SlackMessageMutationResponse {
    channel: String,
    ts: String,
}

#[derive(Deserialize)]
struct SlackForwardMessageResponse {
    #[serde(default, rename = "ts")]
    timestamp: Option<String>,
}

fn validate_message_mutation_response(
    method: &str,
    payload: Value,
    requested_conversation_id: &str,
    requested_message_timestamp: &SlackMessageTimestamp,
) -> Result<(), String> {
    let response = serde_json::from_value::<SlackMessageMutationResponse>(payload)
        .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
    if response.channel != requested_conversation_id {
        return Err(format!(
            "Slack {method} returned channel {} for requested channel {requested_conversation_id}",
            response.channel
        ));
    }
    if response.ts != requested_message_timestamp.as_str() {
        return Err(format!(
            "Slack {method} returned timestamp {} for requested message {}",
            response.ts,
            requested_message_timestamp.as_str()
        ));
    }
    Ok(())
}

fn require_message_conversation_id<'a>(
    operation: &str,
    conversation_id: &'a str,
) -> Result<&'a str, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        Err(format!(
            "Slack message {operation} conversation_id must not be empty"
        ))
    } else {
        Ok(conversation_id)
    }
}
