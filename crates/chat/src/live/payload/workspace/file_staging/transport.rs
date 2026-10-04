use crate::model::{
    SlackAllocatedUpload, SlackFileId, SlackFileStagingAllocatedAttempts, SlackFileUploadMetadata,
};
use serde::Serialize;
use serde_json::Value;

use crate::live::{
    api::{SlackApiClient, SlackObservedApiPost},
    payload::util::string_at,
};

pub(super) enum SlackUploadAllocation {
    Ready {
        allocated: SlackAllocatedUpload,
        upload_url: String,
    },
    AllocatedWithoutUploadUrl {
        allocated: SlackAllocatedUpload,
        diagnostic: String,
    },
    Failed {
        diagnostic: String,
    },
}

pub(super) enum SlackUploadCompletion {
    Accepted,
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

pub(super) enum SlackFileInfoObservation {
    Present,
    StillUnknown { diagnostic: String },
}

#[derive(Serialize)]
struct SlackExternalUploadFileRequest<'a> {
    id: &'a SlackFileId,
    title: &'a str,
}

pub(super) fn allocate_upload(
    api: &SlackApiClient,
    metadata: &SlackFileUploadMetadata,
) -> SlackUploadAllocation {
    let payload = match api.post_observed(
        "files.getUploadURLExternal",
        &[
            ("filename", metadata.name().to_string()),
            ("length", metadata.size_bytes().to_string()),
        ],
    ) {
        SlackObservedApiPost::Accepted(payload) => payload,
        SlackObservedApiPost::NotSent { diagnostic }
        | SlackObservedApiPost::Rejected { diagnostic }
        | SlackObservedApiPost::Unknown { diagnostic } => {
            return SlackUploadAllocation::Failed { diagnostic };
        }
    };
    let file_id = match string_at(&payload, &["file_id"])
        .ok_or_else(|| "Slack files.getUploadURLExternal omitted file_id".to_string())
        .and_then(SlackFileId::parse)
    {
        Ok(file_id) => file_id,
        Err(diagnostic) => return SlackUploadAllocation::Failed { diagnostic },
    };
    let allocated = SlackAllocatedUpload::new(file_id, metadata.clone());
    match string_at(&payload, &["upload_url"]) {
        Some(upload_url) => SlackUploadAllocation::Ready {
            allocated,
            upload_url,
        },
        None => SlackUploadAllocation::AllocatedWithoutUploadUrl {
            allocated,
            diagnostic: "Slack files.getUploadURLExternal omitted upload_url".to_string(),
        },
    }
}

pub(super) fn complete_upload(
    api: &SlackApiClient,
    attempts: &SlackFileStagingAllocatedAttempts,
) -> SlackUploadCompletion {
    let completing = attempts.latest();
    let files = [SlackExternalUploadFileRequest {
        id: completing.file_id(),
        title: completing.name(),
    }];
    let files = match serde_json::to_string(&files) {
        Ok(files) => files,
        Err(error) => {
            return SlackUploadCompletion::NotSent {
                diagnostic: format!("failed to encode Slack upload completion payload: {error}"),
            };
        }
    };
    match api.post_observed("files.completeUploadExternal", &[("files", files)]) {
        SlackObservedApiPost::Accepted(payload) => {
            match validate_completed_file(&payload, completing.file_id()) {
                Ok(()) => SlackUploadCompletion::Accepted,
                Err(diagnostic) => SlackUploadCompletion::Unknown { diagnostic },
            }
        }
        SlackObservedApiPost::NotSent { diagnostic } => {
            SlackUploadCompletion::NotSent { diagnostic }
        }
        SlackObservedApiPost::Rejected { diagnostic } => {
            SlackUploadCompletion::Rejected { diagnostic }
        }
        SlackObservedApiPost::Unknown { diagnostic } => {
            SlackUploadCompletion::Unknown { diagnostic }
        }
    }
}

fn validate_completed_file(payload: &Value, expected_file_id: &SlackFileId) -> Result<(), String> {
    let files = payload
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "Slack files.completeUploadExternal response omitted files array".to_string()
        })?;
    let [file] = files.as_slice() else {
        return Err(format!(
            "Slack files.completeUploadExternal returned {} files for one upload",
            files.len()
        ));
    };
    let returned_file_id = file
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "Slack files.completeUploadExternal response omitted file.id".to_string())
        .and_then(|file_id| SlackFileId::parse(file_id.to_string()))?;
    if &returned_file_id != expected_file_id {
        return Err(format!(
            "Slack files.completeUploadExternal returned file {returned_file_id} for expected file {expected_file_id}"
        ));
    }
    Ok(())
}

pub(super) fn observe_file_info(
    api: &SlackApiClient,
    file_id: &SlackFileId,
) -> SlackFileInfoObservation {
    let payload = match api.post_observed("files.info", &[("file", file_id.as_str().to_string())]) {
        SlackObservedApiPost::Accepted(payload) => payload,
        SlackObservedApiPost::NotSent { diagnostic }
        | SlackObservedApiPost::Rejected { diagnostic }
        | SlackObservedApiPost::Unknown { diagnostic } => {
            return SlackFileInfoObservation::StillUnknown { diagnostic };
        }
    };
    let returned_id = payload
        .get("file")
        .and_then(|file| file.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Slack files.info response omitted file.id".to_string())
        .and_then(|file_id| SlackFileId::parse(file_id.to_string()));
    match returned_id {
        Ok(returned_id) if &returned_id == file_id => SlackFileInfoObservation::Present,
        Ok(returned_id) => SlackFileInfoObservation::StillUnknown {
            diagnostic: format!(
                "Slack files.info returned file {returned_id} for requested file {file_id}"
            ),
        },
        Err(diagnostic) => SlackFileInfoObservation::StillUnknown { diagnostic },
    }
}
