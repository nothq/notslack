use std::{collections::HashSet, thread};

use crate::model::{
    SlackAllocatedUpload, SlackFileId, SlackFileUploadTarget, SlackFileUploadTargetView,
    SlackProfile, SlackUploadFile,
};
use serde::Serialize;
use serde_json::Value;

use crate::live::{
    api::SlackApiClient,
    payload::{
        message::attachments::load_slack_attachment_preview,
        util::{slack_profile_from_user, string_at},
        workspace::{
            SlackAttachmentPreview, SlackAttachmentPreviewCache,
            SlackUserCache,
        },
    },
};

pub(in crate::live::payload::workspace) fn upload_files(
    api: &SlackApiClient,
    target: &SlackFileUploadTarget,
    text: &str,
    files: Vec<SlackUploadFile>,
) -> Result<(), String> {
    let uploaded_files = prepare_external_uploads(api, files)?;
    complete_external_uploads(
        api,
        &uploaded_files,
        SlackExternalUploadDestination::Message {
            target,
            initial_comment: (!text.trim().is_empty()).then_some(text.trim()),
        },
    )?;
    Ok(())
}

pub(in crate::live::payload::workspace) fn load_profile(
    api: &SlackApiClient,
    user_cache: &SlackUserCache,
    user_id: &str,
) -> Result<SlackProfile, String> {
    if let Some(user) = user_cache
        .lock()
        .map_err(|_| "slack user cache mutex poisoned".to_string())?
        .get(user_id)
        .cloned()
    {
        return Ok(slack_profile_from_user(user_id, &user));
    }
    let payload = api.post("users.info", &[("user", user_id.to_string())])?;
    let user = payload
        .get("user")
        .cloned()
        .ok_or_else(|| format!("Slack users.info response missing user payload for {user_id}"))?;
    user_cache.insert(user_id.to_string(), user.clone())?;
    Ok(slack_profile_from_user(user_id, &user))
}

pub(in crate::live::payload::workspace) fn load_attachment_preview(
    api: &SlackApiClient,
    attachment_preview_cache: &SlackAttachmentPreviewCache,
    url: &str,
    timeout: std::time::Duration,
) -> Result<Option<SlackAttachmentPreview>, String> {
    let failed = || {
        attachment_preview_cache
            .lock()
            .map_err(|_| "slack attachment preview cache mutex poisoned".to_string())
    };
    if failed()?.contains(url) {
        return Ok(None);
    }
    let preview = load_slack_attachment_preview(api, url, timeout)
        .map(|(bytes, mimetype)| SlackAttachmentPreview { bytes, mimetype })
        .ok();
    if preview.is_none() {
        failed()?.insert(url.to_string());
    }
    Ok(preview)
}

pub(in crate::live::payload::workspace) fn conversations_contain_id(
    conversations: &Value,
    conversation_id: &str,
) -> bool {
    conversations
        .get("channels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|channel| string_at(channel, &["id"]))
        .any(|id| id == conversation_id)
}

pub(in crate::live::payload::workspace) fn join_slack_request(
    method: &str,
    task: thread::JoinHandle<Result<Value, String>>,
) -> Result<Value, String> {
    task.join()
        .map_err(|_| format!("Slack API {method} request thread panicked"))?
}

pub(in crate::live::payload::workspace) fn workspace_name(team_info: &Value) -> String {
    string_at(team_info, &["team", "name"]).unwrap_or_else(|| "Slack".to_string())
}

fn prepare_external_uploads(
    api: &SlackApiClient,
    files: Vec<SlackUploadFile>,
) -> Result<PreparedExternalUploads, String> {
    if files.is_empty() {
        return Err("cannot complete a Slack file upload without files".to_string());
    }
    let mut uploaded_files = Vec::with_capacity(files.len());
    let mut file_ids = HashSet::with_capacity(files.len());
    for file in files {
        let pending = prepare_external_upload(api, file)?;
        if !file_ids.insert(pending.staged.file_id().clone()) {
            return Err(format!(
                "Slack files.getUploadURLExternal returned duplicate file id {}",
                pending.staged.file_id()
            ));
        }
        let PendingExternalUpload {
            staged,
            upload_url,
            file,
        } = pending;
        api.upload_file(&upload_url, &file)?;
        uploaded_files.push(staged);
    }
    Ok(PreparedExternalUploads {
        files: uploaded_files,
    })
}

fn prepare_external_upload(
    api: &SlackApiClient,
    file: SlackUploadFile,
) -> Result<PendingExternalUpload, String> {
    let metadata = file.metadata().clone();
    let payload = api.post(
        "files.getUploadURLExternal",
        &[
            ("filename", metadata.name().to_string()),
            ("length", metadata.size_bytes().to_string()),
        ],
    )?;
    let upload_url = string_at(&payload, &["upload_url"]).ok_or_else(|| {
        "Slack files.getUploadURLExternal response missing upload_url".to_string()
    })?;
    let file_id =
        SlackFileId::parse(string_at(&payload, &["file_id"]).ok_or_else(|| {
            "Slack files.getUploadURLExternal response missing file_id".to_string()
        })?)?;
    Ok(PendingExternalUpload {
        staged: SlackAllocatedUpload::new(file_id, metadata),
        upload_url,
        file,
    })
}

fn complete_external_uploads(
    api: &SlackApiClient,
    uploaded_files: &PreparedExternalUploads,
    destination: SlackExternalUploadDestination<'_>,
) -> Result<(), String> {
    let files = uploaded_files
        .files
        .iter()
        .map(|file| SlackExternalUploadFileRequest {
            id: file.file_id(),
            title: file.name(),
        })
        .collect::<Vec<_>>();
    let mut params = vec![(
        "files",
        serde_json::to_string(&files)
            .map_err(|error| format!("failed to encode Slack upload payload: {error}"))?,
    )];
    let SlackExternalUploadDestination::Message {
        target,
        initial_comment,
    } = destination;
    match target.view() {
        SlackFileUploadTargetView::Conversation { conversation_id } => {
            params.push(("channel_id", conversation_id.to_string()));
        }
        SlackFileUploadTargetView::Thread {
            conversation_id,
            thread_timestamp,
        } => {
            params.push(("channel_id", conversation_id.to_string()));
            params.push(("thread_ts", thread_timestamp.as_str().to_string()));
        }
    }
    if let Some(initial_comment) = initial_comment {
        params.push(("initial_comment", initial_comment.to_string()));
    }
    let payload = api.post("files.completeUploadExternal", &params)?;
    validate_completed_uploads(&payload, uploaded_files)
}

fn validate_completed_uploads(
    payload: &Value,
    uploaded_files: &PreparedExternalUploads,
) -> Result<(), String> {
    let returned_files = payload
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "Slack files.completeUploadExternal response missing files array".to_string()
        })?;
    if returned_files.len() != uploaded_files.files.len() {
        return Err(format!(
            "Slack files.completeUploadExternal returned {} files for {} requested uploads",
            returned_files.len(),
            uploaded_files.files.len()
        ));
    }
    let mut expected_file_ids = uploaded_files
        .files
        .iter()
        .map(|file| file.file_id().clone())
        .collect::<HashSet<_>>();
    for (index, returned_file) in returned_files.iter().enumerate() {
        let returned_file_id = SlackFileId::parse(
            returned_file
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!("Slack files.completeUploadExternal response file {index} missing id")
                })?
                .to_string(),
        )?;
        if !expected_file_ids.remove(&returned_file_id) {
            return Err(format!(
                "Slack files.completeUploadExternal returned unexpected or duplicate file id {returned_file_id}"
            ));
        }
    }
    if let Some(missing_file_id) = expected_file_ids.into_iter().next() {
        return Err(format!(
            "Slack files.completeUploadExternal omitted requested file id {missing_file_id}"
        ));
    }
    Ok(())
}

struct PendingExternalUpload {
    staged: SlackAllocatedUpload,
    upload_url: String,
    file: SlackUploadFile,
}

struct PreparedExternalUploads {
    files: Vec<SlackAllocatedUpload>,
}

enum SlackExternalUploadDestination<'a> {
    Message {
        target: &'a SlackFileUploadTarget,
        initial_comment: Option<&'a str>,
    },
}

#[derive(Serialize)]
struct SlackExternalUploadFileRequest<'a> {
    id: &'a SlackFileId,
    title: &'a str,
}
