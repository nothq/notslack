use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatAudioClipCaptureStatus {
    Idle,
    RequestingPermission,
    Recording,
    Finalizing,
    Cancelling,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatAudioClipCaptureState {
    pub status: ChatAudioClipCaptureStatus,
    pub session_id: Option<String>,
    pub duration_millis: u64,
    pub owned_by_current_draft: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatVideoClipCaptureStatus {
    Idle,
    Preparing,
    RequestingPermissions,
    Ready,
    Previewing,
    CountingDown,
    Starting,
    Recording,
    Finalizing,
    Reviewing,
    Attaching,
    Cancelling,
    Attached,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatVideoClipCaptureState {
    pub status: ChatVideoClipCaptureStatus,
    pub generation: Option<u64>,
    pub owner: Option<ChatComposerCaptureOwner>,
    pub session_id: Option<String>,
    pub duration_millis: u64,
    pub recording_started_at_unix_millis: Option<i64>,
    pub owned_by_current_draft: bool,
    pub preview_available: bool,
    pub preview_generation: Option<u64>,
    pub countdown_seconds_remaining: Option<u8>,
    pub attachment: ChatVideoClipAttachmentState,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatComposerCaptureOwner {
    pub draft_id: u64,
    pub target: ChatComposerCaptureTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatComposerCaptureTarget {
    Conversation {
        team_id: String,
        self_user_id: String,
        conversation_id: String,
    },
    NewMessage {
        team_id: String,
        self_user_id: String,
        draft_key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatVideoClipAttachmentState {
    pub status: ChatVideoClipAttachmentStatus,
    pub file: Option<ChatComposerFileSummary>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatVideoClipAttachmentStatus {
    None,
    Attaching,
    Staging,
    Attached,
    Error,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatThreadPanelSummary {
    pub conversation_id: String,
    pub parent_message_id: String,
    pub loaded_reply_count: usize,
    pub expected_reply_count: u32,
    pub loading: bool,
    pub error: Option<String>,
    pub draft_text: String,
    pub broadcast_available: bool,
    pub broadcast: bool,
    pub files: Vec<ChatComposerFileSummary>,
    pub composer_focused: bool,
    pub reply_send_pending: bool,
    pub reply_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatComposerFileStatus {
    Queued,
    Uploading,
    Complete,
    Error,
    RetainedUnknown,
    RemoteLoading,
    RemoteReady,
    RemoteError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatComposerFileSummary {
    pub file_id: String,
    pub title: String,
    pub mimetype: String,
    pub status: ChatComposerFileStatus,
    pub operation_id: Option<String>,
    pub pending_retry_operation_id: Option<String>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatComposerFilesAttached {
    pub files: Vec<ChatComposerFileSummary>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatFileCleanupSummary {
    pub operation_id: String,
    pub status: ChatFileCleanupStatus,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatFileCleanupStatus {
    WaitingForStageAndCancel,
    WaitingForStage,
    WaitingForCancel,
    WaitingForRecovery,
    Cleaning,
    RetainedUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRemoteDraftFileCleanupSummary {
    pub team_id: String,
    pub owner_user_id: String,
    pub origin_draft_id: String,
    pub composer_file_id: String,
    pub status: ChatRemoteDraftFileCleanupStatus,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRemoteDraftFileCleanupStatus {
    Cleaning,
    PreservedShared,
    ConfirmedDeleted,
    RetainedUnknown,
    Failed,
}
