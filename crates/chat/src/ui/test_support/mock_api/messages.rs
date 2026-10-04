use super::{wait_test_delay, MockSlackSendOutcome, MockSlackWorkspaceApi, SlackSendAttempt};
use crate::model::{SlackAttachment, SlackMessage, SlackUploadFile, SlackWorkspace};

pub(super) fn send_slack_message(
    api: &MockSlackWorkspaceApi,
    conversation_id: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &crate::model::SlackMessageDraft,
) -> Result<crate::model::SlackMessageSendReceipt, String> {
    let fallback_text = draft.fallback_text();
    record_send_attempt(api, conversation_id, client_message_id, fallback_text);
    apply_pre_accept_outcome(api)?;
    let receipt = accepted_send_receipt(
        api,
        conversation_id,
        client_message_id,
        draft,
        fallback_text,
    )?;
    finalize_accepted_outcome(api, receipt)
}

fn record_send_attempt(
    api: &MockSlackWorkspaceApi,
    conversation_id: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    fallback_text: &str,
) {
    api.sent_messages
        .lock()
        .expect("sent messages mutex poisoned")
        .push(SlackSendAttempt {
            conversation_id: conversation_id.to_string(),
            client_message_id: client_message_id.clone(),
            text: fallback_text.to_string(),
        });
}

fn apply_pre_accept_outcome(api: &MockSlackWorkspaceApi) -> Result<(), String> {
    if let MockSlackSendOutcome::Rejected(error) = &api.send_outcome {
        if !api.send_delay.is_zero() {
            wait_test_delay(api.send_delay);
        }
        return Err(error.clone());
    }
    if matches!(&api.send_outcome, MockSlackSendOutcome::Success) && !api.send_delay.is_zero() {
        wait_test_delay(api.send_delay);
    }
    Ok(())
}

fn accepted_send_receipt(
    api: &MockSlackWorkspaceApi,
    conversation_id: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &crate::model::SlackMessageDraft,
    fallback_text: &str,
) -> Result<crate::model::SlackMessageSendReceipt, String> {
    let mut conversations = api
        .conversations
        .lock()
        .expect("test conversations mutex poisoned");
    let workspace = conversations
        .get_mut(conversation_id)
        .ok_or_else(|| format!("missing test conversation {conversation_id}"))?;
    let timestamp = format!("1999999999.{:06}", workspace.messages.len() + 1);
    let message = accepted_send_message(&timestamp, client_message_id, draft, fallback_text);
    workspace.messages.push(message.clone());
    Ok(crate::model::SlackMessageSendReceipt {
        team_id: workspace.team_id.clone(),
        conversation_id: conversation_id.to_string(),
        self_timezone_id: workspace.self_timezone_id.clone(),
        timestamp,
        self_user_id: workspace
            .self_user_id
            .clone()
            .expect("test Slack workspace must have a self user"),
        message,
    })
}

fn accepted_send_message(
    timestamp: &str,
    client_message_id: &crate::model::SlackMessageClientId,
    draft: &crate::model::SlackMessageDraft,
    fallback_text: &str,
) -> SlackMessage {
    SlackMessage {
        id: timestamp.to_string(),
        client_message_id: Some(client_message_id.clone()),
        author: "You".to_string(),
        timestamp: "Now".to_string(),
        user_id: Some("U_SELF".to_string()),
        avatar_label: Some("YO".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: fallback_text.to_string(),
        rich_body: draft.rich_text().cloned().map(Box::new),
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments: Vec::new(),
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}

fn finalize_accepted_outcome(
    api: &MockSlackWorkspaceApi,
    mut receipt: crate::model::SlackMessageSendReceipt,
) -> Result<crate::model::SlackMessageSendReceipt, String> {
    if let MockSlackSendOutcome::AcceptedWithError(error) = &api.send_outcome {
        if !api.send_delay.is_zero() {
            wait_test_delay(api.send_delay);
        }
        return Err(error.clone());
    }
    if matches!(
        &api.send_outcome,
        MockSlackSendOutcome::AcceptedWithInvalidReceipt
    ) {
        receipt.team_id = "T_INVALID_RECEIPT".to_string();
    }
    Ok(receipt)
}

pub(super) fn upload_slack_files(
    api: &MockSlackWorkspaceApi,
    target: &crate::model::SlackFileUploadTarget,
    text: &str,
    files: Vec<SlackUploadFile>,
) -> Result<
    crate::model::SlackFileUploadReceipt<
        crate::model::SlackConversationSnapshot,
        crate::model::SlackThreadSnapshot,
    >,
    String,
> {
    if files.is_empty() {
        return Err("cannot complete a Slack file upload without files".to_string());
    }
    let conversation_id = target.conversation_id();
    let mut conversations = api
        .conversations
        .lock()
        .expect("test conversations mutex poisoned");
    let workspace = conversations
        .get_mut(conversation_id)
        .ok_or_else(|| format!("missing test conversation {conversation_id}"))?;
    let attachments = uploaded_file_attachments(files);
    match target.view() {
        crate::model::SlackFileUploadTargetView::Conversation { .. } => {
            Ok(upload_files_to_conversation(workspace, text, attachments))
        }
        crate::model::SlackFileUploadTargetView::Thread {
            thread_timestamp, ..
        } => upload_files_to_thread(
            workspace,
            conversation_id,
            thread_timestamp.as_str(),
            text,
            attachments,
        ),
    }
}

fn uploaded_file_attachments(files: Vec<SlackUploadFile>) -> Vec<SlackAttachment> {
    files
        .into_iter()
        .map(|file| SlackAttachment {
            title: file.name().to_string(),
            source: Default::default(),
            mimetype: file.mimetype().to_string(),
            description: String::new(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            ..SlackAttachment::default()
        })
        .collect()
}

fn upload_files_to_conversation(
    workspace: &mut SlackWorkspace,
    text: &str,
    attachments: Vec<SlackAttachment>,
) -> crate::model::SlackFileUploadReceipt<
    crate::model::SlackConversationSnapshot,
    crate::model::SlackThreadSnapshot,
> {
    workspace.messages.push(uploaded_file_message(
        format!("1999999998.{:06}", workspace.messages.len() + 1),
        text,
        attachments,
    ));
    crate::model::SlackFileUploadReceipt::Conversation(workspace.conversation_snapshot())
}

fn upload_files_to_thread(
    workspace: &mut SlackWorkspace,
    conversation_id: &str,
    thread_timestamp: &str,
    text: &str,
    attachments: Vec<SlackAttachment>,
) -> Result<
    crate::model::SlackFileUploadReceipt<
        crate::model::SlackConversationSnapshot,
        crate::model::SlackThreadSnapshot,
    >,
    String,
> {
    let parent = workspace
        .messages
        .iter_mut()
        .find(|message| message.id == thread_timestamp)
        .ok_or_else(|| {
            format!("missing test thread {thread_timestamp} in conversation {conversation_id}")
        })?;
    let reply = uploaded_file_message(
        format!("1999999997.{:06}", parent.replies.len() + 1),
        text,
        attachments,
    );
    parent.latest_reply_timestamp = Some(reply.id.clone());
    parent.replies.push(reply);
    parent.reply_count = Some(
        u32::try_from(parent.replies.len())
            .map_err(|_| "test thread reply count overflowed".to_string())?,
    );
    Ok(crate::model::SlackFileUploadReceipt::Thread(
        crate::model::SlackThreadSnapshot {
            team_id: workspace.team_id.clone(),
            conversation_id: conversation_id.to_string(),
            self_timezone_id: workspace.self_timezone_id.clone(),
            conversation_name: workspace.channel_name.clone(),
            thread_timestamp: thread_timestamp.to_string(),
            parent: Some(parent.clone()),
            replies: parent.replies.clone(),
            next_cursor: None,
        },
    ))
}

fn uploaded_file_message(
    id: String,
    text: &str,
    attachments: Vec<SlackAttachment>,
) -> SlackMessage {
    SlackMessage {
        id,
        client_message_id: None,
        author: "You".to_string(),
        timestamp: "Now".to_string(),
        user_id: Some("U_SELF".to_string()),
        avatar_label: Some("YO".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: text.to_string(),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments,
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}
