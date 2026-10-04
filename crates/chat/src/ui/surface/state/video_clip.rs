use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::model::{
    ChatComposerCaptureOwner, ChatComposerCaptureTarget, ChatComposerFileStatus,
    ChatVideoClipAttachmentState, ChatVideoClipAttachmentStatus, ChatVideoClipCaptureState,
    ChatVideoClipCaptureStatus,
};

use super::{Context, SurfaceState, Window};
use crate::ui::surface::{
    SlackComposerCaptureGeneration, SlackComposerCaptureOperation, SlackComposerDestination,
    SlackComposerFileId, SlackMainComposerDraftOwner, SlackPreparedUploadFile,
    SlackVideoClipCaptureState, SlackVideoClipModal, SlackVideoClipModalPhase,
    SlackVideoClipReviewArtifact, SlackVideoClipSnapshot, SLACK_COMPOSER_FILE_LIMIT,
};
use crate::ui::{
    CapturedVideoClip, MediaCaptureApi, VideoClipCaptureStatus, VideoClipSessionId, VideoFrameFit,
    VideoPlayer, VideoPlayerConfig, VideoPlayerSource,
};

mod cancellation;
mod control;
mod download;
mod finalization;
mod polling;
mod prepare;
mod presentation;
mod recording_start;

const VIDEO_CLIP_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(50);
const GENERIC_VIDEO_CLIP_FILENAME: &str = "Video.mp4";

struct SlackVideoClipPrepareFailure {
    session_id: Option<VideoClipSessionId>,
    diagnostic: String,
}

struct SlackVideoClipSessionTask {
    operation: SlackComposerCaptureOperation,
    session_id: VideoClipSessionId,
    snapshot: SlackVideoClipSnapshot,
    recording_started_at_unix_millis: Option<i64>,
}

struct SlackVideoClipRecordingStartTask {
    api: Arc<MediaCaptureApi>,
    operation: SlackComposerCaptureOperation,
    session_id: VideoClipSessionId,
    recording_started_at_unix_millis: i64,
}

struct SlackVideoClipModalTransition {
    phase: SlackVideoClipModalPhase,
    duration: Duration,
    diagnostic: Option<String>,
}

impl SlackVideoClipModalTransition {
    const fn new(phase: SlackVideoClipModalPhase, duration: Duration) -> Self {
        Self {
            phase,
            duration,
            diagnostic: None,
        }
    }

    fn with_diagnostic(mut self, diagnostic: Option<String>) -> Self {
        self.diagnostic = diagnostic;
        self
    }
}

impl SlackVideoClipSessionTask {
    fn cancelling(self, terminal_error: Option<String>) -> SlackVideoClipCancellationTask {
        SlackVideoClipCancellationTask {
            session: self,
            terminal_error,
        }
    }
}

struct SlackVideoClipCancellationTask {
    session: SlackVideoClipSessionTask,
    terminal_error: Option<String>,
}

impl SurfaceState {
    pub(crate) fn slack_composer_capture_operation_is_current(
        &self,
        operation: &SlackComposerCaptureOperation,
    ) -> bool {
        self.current_slack_main_composer_draft_handle().as_ref() == Some(&operation.owner)
    }
}

fn current_unix_millis() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("The desktop clock must be after the Unix epoch");
    i64::try_from(elapsed.as_millis())
        .expect("The desktop clock must fit in signed Unix milliseconds")
}
