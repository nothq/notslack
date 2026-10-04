use std::sync::Arc;

use crate::ui::surface::{SlackComposerDocument, SlackDraftsSentRowTarget, SlackScheduledEdit};
use crate::ui::{SlackDraftDestination, SlackDraftsSentItem, SlackDraftsSentTab};

pub(super) struct SlackDraftsSentRowTargetInput<'a> {
    pub(super) item: &'a SlackDraftsSentItem,
    pub(super) tab: SlackDraftsSentTab,
    pub(super) destination: &'a SlackDraftDestination,
    pub(super) document: SlackComposerDocument,
    pub(super) authenticated_self_user_id: &'a str,
    pub(super) authenticated_remote_files: Arc<[crate::model::SlackRemoteDraftFileReference]>,
}

pub(super) fn drafts_sent_row_target(
    input: SlackDraftsSentRowTargetInput<'_>,
) -> Result<SlackDraftsSentRowTarget, String> {
    let SlackDraftsSentRowTargetInput {
        item,
        tab,
        destination,
        document,
        authenticated_self_user_id,
        authenticated_remote_files,
    } = input;
    if tab == SlackDraftsSentTab::Scheduled {
        return scheduled_drafts_sent_row_target(
            item,
            destination,
            document,
            authenticated_self_user_id,
            authenticated_remote_files,
        );
    }
    match (tab, destination.message_timestamp.as_deref()) {
        (SlackDraftsSentTab::Drafts, _) => Ok(SlackDraftsSentRowTarget::Draft {
            conversation_id: destination.conversation_id.clone().into(),
            document,
        }),
        (_, Some(message_timestamp)) => Ok(SlackDraftsSentRowTarget::Message(
            slack_message_link(
                &item.team_id,
                &destination.conversation_id,
                message_timestamp,
                destination.thread_timestamp.as_deref(),
            )
            .into(),
        )),
        (_, None) => Ok(SlackDraftsSentRowTarget::Conversation(
            destination.conversation_id.clone().into(),
        )),
    }
}

fn scheduled_drafts_sent_row_target(
    item: &SlackDraftsSentItem,
    destination: &SlackDraftDestination,
    document: SlackComposerDocument,
    authenticated_self_user_id: &str,
    remote_files: Arc<[crate::model::SlackRemoteDraftFileReference]>,
) -> Result<SlackDraftsSentRowTarget, String> {
    let target = crate::model::SlackDraftTarget::new(item.id.clone(), item.revision.clone());
    let post_at_unix_seconds = i64::try_from(item.scheduled_unix_seconds)
        .map_err(|_| format!("Slack scheduled item {} timestamp overflowed", item.id))?;
    let write_targets = item
        .destinations
        .iter()
        .map(crate::model::SlackDraftWriteTarget::from_loaded)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SlackDraftsSentRowTarget::Scheduled {
        edit: SlackScheduledEdit {
            team_id: item.team_id.clone(),
            self_user_id: authenticated_self_user_id.to_string(),
            conversation_id: destination.conversation_id.clone(),
            target,
            client_message_id: crate::model::SlackMessageClientId::new(
                item.client_message_id.clone(),
            )?,
            write_targets,
            post_at_unix_seconds,
            remote_files,
        },
        document,
    })
}

fn slack_message_link(
    team_id: &str,
    conversation_id: &str,
    message_timestamp: &str,
    thread_timestamp: Option<&str>,
) -> String {
    let message_path = message_timestamp.replace('.', "");
    match thread_timestamp {
        Some(thread_timestamp) => format!(
            "https://slack.com/client/{team_id}/{conversation_id}/p{message_path}?thread_ts={thread_timestamp}&cid={conversation_id}"
        ),
        None => {
            format!("https://slack.com/client/{team_id}/{conversation_id}/p{message_path}")
        }
    }
}
