mod encoding;

use std::collections::HashSet;

use crate::model::{
    SlackDraftClientMutationTimestamp, SlackDraftContent, SlackDraftFileDeletion,
    SlackDraftReceipt, SlackDraftTarget, SlackDraftUpdateTarget, SlackDraftWriteTarget,
    SlackDraftWriteTargetView, SlackFileId, SlackMessageDraft, SlackMessageTimestamp,
    SlackScheduledDraftCreateTarget, SlackScheduledDraftReceipt, SlackScheduledDraftUpdateTarget,
};
use serde::Serialize;

use crate::live::api::{SlackApiClient, SlackObservedApiPost};
use encoding::{decode_draft_mutation_response, encode_draft_mutation, SlackDraftMutationReceipt};

pub(in crate::live::payload::workspace) enum SlackScheduledDraftMutationOutcome {
    Confirmed(SlackScheduledDraftReceipt),
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

pub(in crate::live::payload::workspace) fn create_scheduled_draft(
    api: &SlackApiClient,
    target: SlackScheduledDraftCreateTarget<'_>,
    content: SlackDraftContent<'_, SlackMessageDraft>,
    file_ids: &[SlackFileId],
) -> SlackScheduledDraftMutationOutcome {
    let post_at_unix_seconds =
        match require_scheduled_draft_timestamp(target.post_at_unix_seconds()) {
            Ok(post_at_unix_seconds) => post_at_unix_seconds,
            Err(diagnostic) => {
                return SlackScheduledDraftMutationOutcome::NotSent { diagnostic };
            }
        };
    let destinations = [SlackDraftDestinationRequest::from_write_target(
        target.write_target(),
    )];
    observe_scheduled_draft_mutation(
        api,
        SlackDraftMutation::Create {
            client_message_id: target.client_message_id(),
        },
        SlackDraftMutationPayload {
            destinations: &destinations,
            content,
            file_ids,
            date_scheduled: post_at_unix_seconds,
        },
    )
}

pub(in crate::live::payload::workspace) fn update_scheduled_draft(
    api: &SlackApiClient,
    target: SlackScheduledDraftUpdateTarget<'_>,
    content: SlackDraftContent<'_, SlackMessageDraft>,
    file_ids: &[SlackFileId],
) -> SlackScheduledDraftMutationOutcome {
    let post_at_unix_seconds =
        match require_scheduled_draft_timestamp(target.post_at_unix_seconds()) {
            Ok(post_at_unix_seconds) => post_at_unix_seconds,
            Err(diagnostic) => {
                return SlackScheduledDraftMutationOutcome::NotSent { diagnostic };
            }
        };
    let destination_requests = target
        .write_targets()
        .iter()
        .map(SlackDraftDestinationRequest::from_write_target)
        .collect::<Vec<_>>();
    observe_scheduled_draft_mutation(
        api,
        SlackDraftMutation::Update {
            update_target: target.update_target(),
        },
        SlackDraftMutationPayload {
            destinations: &destination_requests,
            content,
            file_ids,
            date_scheduled: post_at_unix_seconds,
        },
    )
}

pub(in crate::live::payload::workspace) fn create_draft(
    api: &SlackApiClient,
    client_message_id: &crate::model::SlackMessageClientId,
    write_target: &SlackDraftWriteTarget,
    content: SlackDraftContent<'_, SlackMessageDraft>,
    file_ids: &[SlackFileId],
) -> Result<SlackDraftReceipt, String> {
    let destinations = [SlackDraftDestinationRequest::from_write_target(
        write_target,
    )];
    mutate_draft(
        api,
        SlackDraftMutation::Create { client_message_id },
        SlackDraftMutationPayload {
            destinations: &destinations,
            content,
            file_ids,
            date_scheduled: 0,
        },
    )
    .map(|receipt| SlackDraftReceipt {
        target: receipt.target,
    })
}

pub(in crate::live::payload::workspace) fn update_draft(
    api: &SlackApiClient,
    update_target: SlackDraftUpdateTarget<'_>,
    write_target: &SlackDraftWriteTarget,
    content: SlackDraftContent<'_, SlackMessageDraft>,
    file_ids: &[SlackFileId],
) -> Result<SlackDraftReceipt, String> {
    let destinations = [SlackDraftDestinationRequest::from_write_target(
        write_target,
    )];
    mutate_draft(
        api,
        SlackDraftMutation::Update { update_target },
        SlackDraftMutationPayload {
            destinations: &destinations,
            content,
            file_ids,
            date_scheduled: 0,
        },
    )
    .map(|receipt| SlackDraftReceipt {
        target: receipt.target,
    })
}

pub(in crate::live::payload::workspace) fn delete_draft(
    api: &SlackApiClient,
    target: &SlackDraftTarget,
    file_deletion: SlackDraftFileDeletion,
) -> Result<(), String> {
    let client_mutation_timestamp =
        SlackDraftClientMutationTimestamp::from_loaded_revision(target.revision());
    api.post(
        "drafts.delete",
        &[
            (
                "client_last_updated_ts",
                client_mutation_timestamp.as_str().to_string(),
            ),
            ("draft_id", target.draft_id().as_str().to_string()),
            (
                "skip_file_deletion",
                file_deletion.preserves_files().to_string(),
            ),
        ],
    )?;
    Ok(())
}

enum SlackDraftMutation<'a> {
    Create {
        client_message_id: &'a crate::model::SlackMessageClientId,
    },
    Update {
        update_target: SlackDraftUpdateTarget<'a>,
    },
}

impl SlackDraftMutation<'_> {
    fn method(&self) -> &'static str {
        match self {
            Self::Create { .. } => "drafts.create",
            Self::Update { .. } => "drafts.update",
        }
    }
}

struct SlackDraftMutationPayload<'a> {
    destinations: &'a [SlackDraftDestinationRequest<'a>],
    content: SlackDraftContent<'a, SlackMessageDraft>,
    file_ids: &'a [SlackFileId],
    date_scheduled: i64,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SlackDraftDestinationRequest<'a> {
    Conversation {
        channel_id: &'a str,
        #[serde(skip_serializing_if = "<[_]>::is_empty")]
        user_ids: &'a [String],
        #[serde(rename = "message_ts", skip_serializing_if = "Option::is_none")]
        message_timestamp: Option<&'a str>,
    },
    Thread {
        channel_id: &'a str,
        #[serde(skip_serializing_if = "<[_]>::is_empty")]
        user_ids: &'a [String],
        #[serde(rename = "thread_ts")]
        thread_timestamp: &'a str,
        #[serde(rename = "message_ts", skip_serializing_if = "Option::is_none")]
        message_timestamp: Option<&'a str>,
        #[serde(skip_serializing_if = "is_false")]
        broadcast: bool,
    },
}

impl<'a> SlackDraftDestinationRequest<'a> {
    fn from_write_target(target: &'a SlackDraftWriteTarget) -> Self {
        match target.view() {
            SlackDraftWriteTargetView::Conversation {
                conversation_id,
                user_ids,
                message_timestamp,
            } => Self::Conversation {
                channel_id: conversation_id,
                user_ids,
                message_timestamp: message_timestamp.map(SlackMessageTimestamp::as_str),
            },
            SlackDraftWriteTargetView::Thread {
                conversation_id,
                user_ids,
                thread_timestamp,
                message_timestamp,
                broadcast,
            } => Self::Thread {
                channel_id: conversation_id,
                user_ids,
                thread_timestamp: thread_timestamp.as_str(),
                message_timestamp: message_timestamp.map(SlackMessageTimestamp::as_str),
                broadcast,
            },
        }
    }
}

fn encode_draft_destinations(
    destinations: &[SlackDraftDestinationRequest<'_>],
) -> Result<String, String> {
    if destinations.is_empty() {
        return Err("Slack draft requires at least one destination".to_string());
    }
    serde_json::to_string(destinations)
        .map_err(|error| format!("failed to encode Slack draft destinations: {error}"))
}

fn encode_draft_file_ids(file_ids: &[SlackFileId]) -> Result<String, String> {
    let mut unique_file_ids = HashSet::with_capacity(file_ids.len());
    for file_id in file_ids {
        if !unique_file_ids.insert(file_id) {
            return Err(format!("Slack draft contains duplicate file id {file_id}"));
        }
    }
    serde_json::to_string(file_ids)
        .map_err(|error| format!("failed to encode Slack draft file ids: {error}"))
}

fn require_scheduled_draft_timestamp(post_at_unix_seconds: i64) -> Result<i64, String> {
    if post_at_unix_seconds <= 0 {
        Err("Slack scheduled draft date_scheduled must be a positive Unix timestamp".to_string())
    } else {
        Ok(post_at_unix_seconds)
    }
}

fn mutate_draft(
    api: &SlackApiClient,
    mutation: SlackDraftMutation<'_>,
    payload: SlackDraftMutationPayload<'_>,
) -> Result<SlackDraftMutationReceipt, String> {
    let request = encode_draft_mutation(mutation, payload)?;
    let response = api.post(request.method, &request.parameters)?;
    decode_draft_mutation_response(
        request.method,
        response,
        request.expected_draft_id,
        request.expected_date_scheduled,
    )
}

fn observe_scheduled_draft_mutation(
    api: &SlackApiClient,
    mutation: SlackDraftMutation<'_>,
    payload: SlackDraftMutationPayload<'_>,
) -> SlackScheduledDraftMutationOutcome {
    let request = match encode_draft_mutation(mutation, payload) {
        Ok(request) => request,
        Err(diagnostic) => {
            return SlackScheduledDraftMutationOutcome::NotSent { diagnostic };
        }
    };
    match api.post_observed(request.method, &request.parameters) {
        SlackObservedApiPost::Accepted(response) => match decode_draft_mutation_response(
            request.method,
            response,
            request.expected_draft_id,
            request.expected_date_scheduled,
        ) {
            Ok(receipt) => {
                SlackScheduledDraftMutationOutcome::Confirmed(SlackScheduledDraftReceipt {
                    target: receipt.target,
                    post_at_unix_seconds: receipt.date_scheduled,
                })
            }
            Err(diagnostic) => SlackScheduledDraftMutationOutcome::Unknown { diagnostic },
        },
        SlackObservedApiPost::NotSent { diagnostic } => {
            SlackScheduledDraftMutationOutcome::NotSent { diagnostic }
        }
        SlackObservedApiPost::Rejected { diagnostic } => {
            SlackScheduledDraftMutationOutcome::Rejected { diagnostic }
        }
        SlackObservedApiPost::Unknown { diagnostic } => {
            SlackScheduledDraftMutationOutcome::Unknown { diagnostic }
        }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}
