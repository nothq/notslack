mod delivery;
mod drafts;

use std::sync::Arc;

use crate::ui::surface::{
    slack_history_timestamp_key, PreparedSlackMessageSendReceipt, SlackComposerDraft,
    SlackMainComposerDraftHandle, SlackSendDraftSource, SlackSendPayload, SlackSendRequest,
    SurfaceState,
};
use crate::ui::{SlackConversationSnapshot, SlackMessage, SlackMessageDraft};

struct PreparedSlackSend {
    team_id: String,
    self_user_id: String,
    conversation_id: String,
    draft_source: SlackSendDraftSource,
    author: String,
    avatar_label: Option<String>,
    avatar_image_url: Option<String>,
    timezone: chrono_tz::Tz,
    draft_text: String,
    message_draft: Option<SlackMessageDraft>,
    accepted_draft_handle: SlackMainComposerDraftHandle,
    accepted_draft: SlackComposerDraft,
    file_ids: Arc<[crate::model::SlackFileId]>,
}

struct SlackSendWork {
    request: SlackSendRequest,
    payload: Arc<SlackSendPayload>,
    previous_message: Option<Box<SlackMessage>>,
}

enum PreparedSlackSendResult {
    Message(Box<PreparedSlackMessageSendReceipt>),
    Share(Box<SlackConversationSnapshot>),
}

fn slack_message_insertion_index(
    messages: &[SlackMessage],
    receipt_timestamp_key: (u64, u32),
) -> Result<usize, String> {
    for (index, message) in messages.iter().enumerate() {
        if slack_history_timestamp_key(&message.id)? > receipt_timestamp_key {
            return Ok(index);
        }
    }
    Ok(messages.len())
}
