use crate::model::{
    SlackDraftContent, SlackDraftId, SlackDraftRevision, SlackDraftTarget, SlackFileId,
    SlackMessageDraft,
};
use serde::Deserialize;
use serde_json::Value;

use crate::live::payload::message_draft::slack_draft_blocks_json;

use super::{
    encode_draft_destinations, encode_draft_file_ids, SlackDraftMutation, SlackDraftMutationPayload,
};

pub(super) struct SlackEncodedDraftMutation<'a> {
    pub(super) method: &'static str,
    pub(super) parameters: Vec<(&'static str, String)>,
    pub(super) expected_draft_id: Option<&'a SlackDraftId>,
    pub(super) expected_date_scheduled: i64,
}

pub(super) fn encode_draft_mutation<'a>(
    mutation: SlackDraftMutation<'a>,
    payload: SlackDraftMutationPayload<'_>,
) -> Result<SlackEncodedDraftMutation<'a>, String> {
    let SlackDraftMutationPayload {
        destinations,
        content,
        file_ids,
        date_scheduled,
    } = payload;
    if date_scheduled < 0 {
        return Err("Slack draft date_scheduled must not be negative".to_string());
    }
    let method = mutation.method();
    let blocks = encode_draft_blocks(content, file_ids)?;
    let mut parameters = vec![
        ("blocks", blocks),
        ("attachments", "[]".to_string()),
        ("destinations", encode_draft_destinations(destinations)?),
        ("file_ids", encode_draft_file_ids(file_ids)?),
        ("is_from_composer", "true".to_string()),
        ("date_scheduled", date_scheduled.to_string()),
    ];
    let expected_draft_id = match mutation {
        SlackDraftMutation::Create { client_message_id } => {
            parameters.push(("client_msg_id", client_message_id.as_str().to_string()));
            None
        }
        SlackDraftMutation::Update { update_target } => {
            parameters.push((
                "client_last_updated_ts",
                update_target
                    .client_mutation_timestamp()
                    .as_str()
                    .to_string(),
            ));
            parameters.push((
                "draft_id",
                update_target.target().draft_id().as_str().to_string(),
            ));
            Some(update_target.target().draft_id())
        }
    };
    Ok(SlackEncodedDraftMutation {
        method,
        parameters,
        expected_draft_id,
        expected_date_scheduled: date_scheduled,
    })
}

fn encode_draft_blocks(
    content: SlackDraftContent<'_, SlackMessageDraft>,
    file_ids: &[SlackFileId],
) -> Result<String, String> {
    match content {
        SlackDraftContent::Message(draft) => slack_draft_blocks_json(draft),
        SlackDraftContent::FilesOnly if file_ids.is_empty() => {
            Err("Slack files-only draft requires at least one file id".to_string())
        }
        SlackDraftContent::FilesOnly => Ok("[]".to_string()),
    }
}

pub(super) fn decode_draft_mutation_response(
    method: &str,
    payload: Value,
    expected_draft_id: Option<&SlackDraftId>,
    expected_date_scheduled: i64,
) -> Result<SlackDraftMutationReceipt, String> {
    let response = serde_json::from_value::<SlackDraftMutationResponse>(payload)
        .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
    let draft_id = SlackDraftId::parse(response.draft.id)?;
    if let Some(expected_draft_id) = expected_draft_id {
        if expected_draft_id != &draft_id {
            return Err(format!(
                "Slack {method} returned draft {draft_id} for requested draft {expected_draft_id}"
            ));
        }
    }
    let revision = SlackDraftRevision::parse(response.draft.last_updated_timestamp)?;
    let returned_post_at = i64::try_from(response.draft.date_scheduled).map_err(|_| {
        format!(
            "Slack {method} returned an overflowing date_scheduled {}",
            response.draft.date_scheduled
        )
    })?;
    if returned_post_at != expected_date_scheduled {
        return Err(format!(
            "Slack {method} returned date_scheduled {returned_post_at} for requested timestamp {expected_date_scheduled}"
        ));
    }
    Ok(SlackDraftMutationReceipt {
        target: SlackDraftTarget::new(draft_id, revision),
        date_scheduled: returned_post_at,
    })
}

pub(super) struct SlackDraftMutationReceipt {
    pub(super) target: SlackDraftTarget,
    pub(super) date_scheduled: i64,
}

#[derive(Deserialize)]
struct SlackDraftMutationResponse {
    draft: SlackDraftMutationWire,
}

#[derive(Deserialize)]
struct SlackDraftMutationWire {
    id: String,
    #[serde(rename = "last_updated_ts")]
    last_updated_timestamp: String,
    date_scheduled: u64,
}
