use std::time::Duration;

use crate::ui::{
    CapturedVideoClip, VideoClipCaptureStatus, VideoClipPreview, VideoClipSessionId,
    VideoClipSnapshot,
};

use super::{SlackComposerCaptureOperation, SlackComposerFileId};

#[derive(Clone, Debug)]
pub(crate) struct SlackVideoClipSnapshot {
    pub(crate) status: VideoClipCaptureStatus,
    pub(crate) duration: Duration,
    pub(crate) latest_preview: Option<VideoClipPreview>,
}

impl From<VideoClipSnapshot> for SlackVideoClipSnapshot {
    fn from(snapshot: VideoClipSnapshot) -> Self {
        Self {
            status: snapshot.status().clone(),
            duration: snapshot.duration(),
            latest_preview: snapshot.latest_preview().cloned(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SlackVideoClipReviewArtifact {
    clip: CapturedVideoClip,
}

impl SlackVideoClipReviewArtifact {
    pub(crate) fn new(clip: CapturedVideoClip) -> Self {
        Self { clip }
    }

    pub(crate) const fn clip(&self) -> &CapturedVideoClip {
        &self.clip
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SlackVideoClipCaptureState {
    Preparing {
        operation: SlackComposerCaptureOperation,
    },
    CancellingPreparation {
        operation: SlackComposerCaptureOperation,
    },
    Prepared {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
    },
    Starting {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
        recording_started_at_unix_millis: i64,
    },
    CancellingStart {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
        recording_started_at_unix_millis: i64,
    },
    Recording {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
        recording_started_at_unix_millis: i64,
    },
    Finalizing {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
        recording_started_at_unix_millis: i64,
    },
    Reviewing {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        recording_started_at_unix_millis: i64,
        artifact: SlackVideoClipReviewArtifact,
    },
    Cancelling {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        snapshot: SlackVideoClipSnapshot,
        recording_started_at_unix_millis: Option<i64>,
        terminal_error: Option<String>,
    },
    Attaching {
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        duration: Duration,
        recording_started_at_unix_millis: i64,
    },
    Attached {
        operation: SlackComposerCaptureOperation,
        duration: Duration,
        recording_started_at_unix_millis: i64,
        file_id: SlackComposerFileId,
    },
    Failed {
        operation: SlackComposerCaptureOperation,
        session_id: Option<VideoClipSessionId>,
        duration: Duration,
        recording_started_at_unix_millis: Option<i64>,
        diagnostic: String,
    },
    Cancelled {
        operation: SlackComposerCaptureOperation,
        duration: Duration,
        recording_started_at_unix_millis: Option<i64>,
    },
}

impl SlackVideoClipCaptureState {
    pub(crate) fn operation(&self) -> &SlackComposerCaptureOperation {
        match self {
            Self::Preparing { operation }
            | Self::CancellingPreparation { operation }
            | Self::Prepared { operation, .. }
            | Self::Starting { operation, .. }
            | Self::CancellingStart { operation, .. }
            | Self::Recording { operation, .. }
            | Self::Finalizing { operation, .. }
            | Self::Reviewing { operation, .. }
            | Self::Cancelling { operation, .. }
            | Self::Attaching { operation, .. }
            | Self::Attached { operation, .. }
            | Self::Failed { operation, .. }
            | Self::Cancelled { operation, .. } => operation,
        }
    }

    pub(crate) fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Attached { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }

    pub(crate) fn blocks_draft_submission(&self) -> bool {
        !self.terminal()
    }

    pub(crate) fn duration(&self) -> Duration {
        match self {
            Self::Preparing { .. } | Self::CancellingPreparation { .. } => Duration::ZERO,
            Self::Prepared { snapshot, .. }
            | Self::Starting { snapshot, .. }
            | Self::CancellingStart { snapshot, .. }
            | Self::Recording { snapshot, .. }
            | Self::Finalizing { snapshot, .. }
            | Self::Cancelling { snapshot, .. } => snapshot.duration,
            Self::Reviewing { artifact, .. } => artifact.clip().duration(),
            Self::Attaching { duration, .. }
            | Self::Attached { duration, .. }
            | Self::Failed { duration, .. }
            | Self::Cancelled { duration, .. } => *duration,
        }
    }

    pub(crate) fn session_id(&self) -> Option<VideoClipSessionId> {
        match self {
            Self::Preparing { .. }
            | Self::CancellingPreparation { .. }
            | Self::Attached { .. }
            | Self::Cancelled { .. } => None,
            Self::Prepared { session_id, .. }
            | Self::Starting { session_id, .. }
            | Self::CancellingStart { session_id, .. }
            | Self::Recording { session_id, .. }
            | Self::Finalizing { session_id, .. }
            | Self::Reviewing { session_id, .. }
            | Self::Cancelling { session_id, .. }
            | Self::Attaching { session_id, .. } => Some(*session_id),
            Self::Failed { session_id, .. } => *session_id,
        }
    }

    pub(crate) fn snapshot(&self) -> Option<&SlackVideoClipSnapshot> {
        match self {
            Self::Prepared { snapshot, .. }
            | Self::Starting { snapshot, .. }
            | Self::CancellingStart { snapshot, .. }
            | Self::Recording { snapshot, .. }
            | Self::Finalizing { snapshot, .. }
            | Self::Cancelling { snapshot, .. } => Some(snapshot),
            Self::Preparing { .. }
            | Self::CancellingPreparation { .. }
            | Self::Reviewing { .. }
            | Self::Attaching { .. }
            | Self::Attached { .. }
            | Self::Failed { .. }
            | Self::Cancelled { .. } => None,
        }
    }

    pub(crate) fn recording_started_at_unix_millis(&self) -> Option<i64> {
        match self {
            Self::Starting {
                recording_started_at_unix_millis,
                ..
            }
            | Self::CancellingStart {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Recording {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Finalizing {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Reviewing {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Attaching {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Attached {
                recording_started_at_unix_millis,
                ..
            } => Some(*recording_started_at_unix_millis),
            Self::Cancelling {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Failed {
                recording_started_at_unix_millis,
                ..
            }
            | Self::Cancelled {
                recording_started_at_unix_millis,
                ..
            } => *recording_started_at_unix_millis,
            Self::Preparing { .. } | Self::CancellingPreparation { .. } | Self::Prepared { .. } => {
                None
            }
        }
    }
}
