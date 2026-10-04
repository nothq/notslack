mod attachments;
mod compose;
mod delivery;
mod pending;

use std::{ops::Range, sync::Arc};

use super::thread::{
    merge_slack_thread_rows, normalize_slack_thread_parent_row, refresh_slack_thread_list_rows,
    slack_thread_reply_list_index,
};
use super::{
    prepare_slack_thread_reply_receipt, prepare_slack_thread_snapshot, Context,
    PreparedSlackThreadReplyReceipt, PreparedSlackThreadSnapshot, SlackComposerFormatAction,
    SlackComposerTarget, SlackMessageRow, SurfaceState, WorkspaceApi,
};
use crate::ui::surface::{
    SlackComposerDocument, SlackComposerDraft, SlackComposerDraftKey, SlackPendingThreadReplySend,
    SlackReplyParticipantRow, SlackReplySummaryRow,
};
use crate::ui::{
    SlackMessageClientId, SlackMessageDraft, SlackMessageTimestamp,
    SLACK_MESSAGE_REPLY_PARTICIPANT_VISIBLE_LIMIT,
};

#[derive(Clone)]
struct SlackThreadReplySendRequest {
    send_generation: u64,
    team_id: String,
    draft_key: SlackComposerDraftKey,
    conversation_id: String,
    parent_message_id: String,
    thread_timestamp: SlackMessageTimestamp,
    draft_token: u64,
    draft_document_revision: u64,
    payload: Arc<SlackThreadReplySendPayload>,
    broadcast: bool,
}

impl SlackThreadReplySendRequest {
    fn pending_send(&self) -> SlackPendingThreadReplySend {
        SlackPendingThreadReplySend {
            generation: self.send_generation,
            draft_token: self.draft_token,
            draft_document_revision: self.draft_document_revision,
        }
    }
}

#[derive(Clone)]
pub(in crate::ui::surface::state) enum SlackThreadReplySendPayload {
    Message {
        client_message_id: SlackMessageClientId,
        draft: SlackMessageDraft,
    },
    Share(crate::model::SlackFileShareRequest<SlackMessageDraft>),
}

impl SlackThreadReplySendPayload {
    fn client_message_id(&self) -> &SlackMessageClientId {
        match self {
            Self::Message {
                client_message_id, ..
            } => client_message_id,
            Self::Share(share) => share.client_message_id(),
        }
    }
}

pub(in crate::ui::surface::state) struct PreparedSlackThreadReplyDraft {
    pub(in crate::ui::surface::state) payload: Arc<SlackThreadReplySendPayload>,
    pub(in crate::ui::surface::state) document_revision: u64,
    pub(in crate::ui::surface::state) broadcast: bool,
}

pub(in crate::ui::surface::state) enum PreparedSlackThreadReplySendResult {
    Message(PreparedSlackThreadReplyReceipt),
    Share(PreparedSlackThreadSnapshot),
}

pub(in crate::ui::surface::state) fn prepare_slack_thread_reply_draft(
    draft: &mut SlackComposerDraft,
    conversation_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
    broadcast_supported: bool,
    share_supported: bool,
) -> Result<PreparedSlackThreadReplyDraft, String> {
    let text = draft.text().trim().to_string();
    if draft.broadcast && !broadcast_supported {
        return Err("This Slack thread cannot broadcast replies to a channel.".to_string());
    }
    if draft.files.is_empty() {
        if text.is_empty() {
            return Err("Add a reply or attachment before sending.".to_string());
        }
        let message_draft = draft.document.export_message_draft()?;
        let client_message_id = draft
            .client_message_id
            .get_or_insert_with(SlackMessageClientId::generate)
            .clone();
        return Ok(PreparedSlackThreadReplyDraft {
            payload: Arc::new(SlackThreadReplySendPayload::Message {
                client_message_id,
                draft: message_draft,
            }),
            document_revision: draft.document.revision(),
            broadcast: draft.broadcast,
        });
    }

    prepare_slack_thread_file_share_draft(
        draft,
        conversation_id,
        thread_timestamp,
        share_supported,
        text,
    )
}

fn prepare_slack_thread_file_share_draft(
    draft: &mut SlackComposerDraft,
    conversation_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
    share_supported: bool,
    text: String,
) -> Result<PreparedSlackThreadReplyDraft, String> {
    if !share_supported {
        return Err("File sharing is unavailable for this Slack workspace.".to_string());
    }
    let file_ids = draft.files.projected_slack_file_ids()?;
    let message_draft = (!text.is_empty())
        .then(|| draft.document.export_message_draft())
        .transpose()?;
    let client_message_id = draft
        .client_message_id
        .get_or_insert_with(SlackMessageClientId::generate)
        .clone();
    let target = crate::model::SlackFileShareTarget::thread(
        conversation_id.to_string(),
        thread_timestamp.clone(),
        draft.broadcast,
    )?;
    let share = match message_draft {
        Some(message) => crate::model::SlackFileShareRequest::message(
            target,
            file_ids.to_vec(),
            client_message_id,
            message,
        )?,
        None => crate::model::SlackFileShareRequest::files_only(
            target,
            file_ids.to_vec(),
            client_message_id,
        )?,
    };
    Ok(PreparedSlackThreadReplyDraft {
        payload: Arc::new(SlackThreadReplySendPayload::Share(share)),
        document_revision: draft.document.revision(),
        broadcast: draft.broadcast,
    })
}

pub(in crate::ui::surface::state) fn execute_slack_thread_reply_send(
    workspace_api: &dyn WorkspaceApi,
    conversation_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
    payload: &SlackThreadReplySendPayload,
    broadcast: bool,
) -> Result<PreparedSlackThreadReplySendResult, String> {
    match payload {
        SlackThreadReplySendPayload::Message {
            client_message_id,
            draft,
        } => workspace_api
            .send_slack_thread_reply(
                crate::model::SlackThreadReplyTarget {
                    conversation_id,
                    thread_timestamp,
                    broadcast,
                },
                client_message_id,
                draft,
            )
            .map(prepare_slack_thread_reply_receipt)
            .map(PreparedSlackThreadReplySendResult::Message),
        SlackThreadReplySendPayload::Share(share) => {
            match workspace_api.share_slack_files(share)? {
                crate::model::SlackFileShareReceipt::Thread(snapshot) => {
                    Ok(PreparedSlackThreadReplySendResult::Share(
                        prepare_slack_thread_snapshot(snapshot),
                    ))
                }
                crate::model::SlackFileShareReceipt::Conversation(_) => Err(
                    "Slack thread file share returned a conversation refresh receipt".to_string(),
                ),
            }
        }
    }
}

impl SurfaceState {
    fn update_main_slack_thread_summary_after_reply(
        &mut self,
        reply_row: &SlackMessageRow,
        expected_reply_count: u32,
    ) {
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| !panel.origin.is_conversation())
        {
            return;
        }
        let Some(parent_message_id) = self
            .slack_thread_panel
            .as_ref()
            .map(|panel| panel.parent_message_id.clone())
        else {
            return;
        };
        if !self
            .slack_message_rows
            .iter()
            .any(|row| row.id == parent_message_id)
        {
            return;
        }
        self.advance_slack_conversation_revision();
        let parent_row = Arc::make_mut(&mut self.slack_message_rows)
            .iter_mut()
            .find(|row| row.id == parent_message_id)
            .expect("Slack thread parent disappeared during summary update");
        let reply_count = parent_row
            .reply_count
            .unwrap_or_default()
            .max(expected_reply_count);
        parent_row.reply_count = Some(reply_count);
        upsert_slack_reply_participant(parent_row, reply_row);
        parent_row.latest_reply_timestamp = Some(reply_row.timestamp.clone());
        parent_row.reply_summary = Some(SlackReplySummaryRow::new(
            &parent_row.id,
            reply_count,
            &reply_row.timestamp,
        ));
        parent_row.latest_reply_author = Some(reply_row.author.clone());
        parent_row.latest_reply_user_id = reply_row.user_id.clone();
        parent_row.latest_reply_avatar_text = Some(reply_row.avatar_text.clone());
        parent_row.latest_reply_avatar_fill = reply_row.avatar_fill;
        parent_row.latest_reply_avatar_image_url = reply_row.avatar_image_url.clone();
        self.slack_message_list_state.remeasure();
    }
}

fn upsert_slack_reply_participant(parent_row: &mut SlackMessageRow, reply_row: &SlackMessageRow) {
    let Some(user_id) = reply_row.user_id.as_deref() else {
        return;
    };
    if parent_row
        .reply_participant_user_ids
        .iter()
        .any(|participant_user_id| participant_user_id.as_ref() == user_id)
    {
        return;
    }
    let mut participant_user_ids = parent_row.reply_participant_user_ids.to_vec();
    participant_user_ids.push(user_id.to_string().into());
    parent_row.reply_participant_user_ids = participant_user_ids.into();
    if parent_row.reply_participants.len() >= SLACK_MESSAGE_REPLY_PARTICIPANT_VISIBLE_LIMIT {
        return;
    }
    let mut participants = parent_row.reply_participants.to_vec();
    participants.push(SlackReplyParticipantRow {
        profile_element_id: format!("slack-reply-participant-{}-{user_id}", parent_row.id).into(),
        profile_accessibility_label: format!("View {}’s profile", reply_row.author).into(),
        user_id: user_id.to_string().into(),
        avatar_text: reply_row.avatar_text.clone().into(),
        avatar_fill: reply_row.avatar_fill,
        avatar_image_url: reply_row.avatar_image_url.clone().map(Into::into),
    });
    parent_row.reply_participants = participants.into();
}
