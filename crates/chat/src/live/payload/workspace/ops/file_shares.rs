use crate::model::SlackMessageDraft;
use crate::model::{SlackFileShareContentView, SlackFileShareRequest, SlackFileShareTargetView};

use crate::live::{
    api::{SlackApiClient, SlackObservedApiPost},
    payload::message_draft::slack_message_blocks_json,
};

type SlackFileShareParameters = Vec<(&'static str, String)>;

pub(in crate::live::payload::workspace) fn share_files(
    api: &SlackApiClient,
    request: &SlackFileShareRequest<SlackMessageDraft>,
) -> SlackFileSharePostOutcome {
    let parameters = match share_file_parameters(request) {
        Ok(parameters) => parameters,
        Err(diagnostic) => return SlackFileSharePostOutcome::NotSent { diagnostic },
    };
    match api.post_observed("files.share", &parameters) {
        SlackObservedApiPost::Accepted(_) => SlackFileSharePostOutcome::Accepted,
        SlackObservedApiPost::NotSent { diagnostic } => {
            SlackFileSharePostOutcome::NotSent { diagnostic }
        }
        SlackObservedApiPost::Rejected { diagnostic } => {
            SlackFileSharePostOutcome::Rejected { diagnostic }
        }
        SlackObservedApiPost::Unknown { diagnostic } => {
            SlackFileSharePostOutcome::Unknown { diagnostic }
        }
    }
}

pub(in crate::live::payload::workspace) enum SlackFileSharePostOutcome {
    Accepted,
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

fn share_file_parameters(
    request: &SlackFileShareRequest<SlackMessageDraft>,
) -> Result<SlackFileShareParameters, String> {
    let mut parameters = vec![
        (
            "files",
            request
                .file_ids()
                .iter()
                .map(|file_id| file_id.as_str())
                .collect::<Vec<_>>()
                .join(","),
        ),
        ("channel", request.target().conversation_id().to_string()),
    ];
    match request.target().view() {
        SlackFileShareTargetView::Conversation { .. } => {}
        SlackFileShareTargetView::Thread {
            thread_timestamp,
            broadcast,
            ..
        } => {
            parameters.push(("thread_ts", thread_timestamp.as_str().to_string()));
            parameters.push(("broadcast", broadcast.to_string()));
        }
    }

    let blocks = match request.content() {
        SlackFileShareContentView::Message(message) => {
            let blocks = slack_message_blocks_json(message)?;
            if blocks.is_none() {
                parameters.push(("comment", message.fallback_text().to_string()));
            }
            blocks
        }
        SlackFileShareContentView::FilesOnly => None,
    };
    parameters.push((
        "client_msg_id",
        request.client_message_id().as_str().to_string(),
    ));
    if let Some(blocks) = blocks {
        parameters.push(("blocks", blocks));
    }
    parameters.push(("resharing_aware", "true".to_string()));

    Ok(parameters)
}
