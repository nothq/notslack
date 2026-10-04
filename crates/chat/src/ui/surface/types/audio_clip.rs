use std::time::{Duration, Instant};

use crate::ui::AudioClipSessionId;

use super::SlackComposerCaptureOperation;

#[derive(Clone, Debug)]
pub(crate) enum SlackAudioClipCaptureState {
    RequestingPermission {
        operation: SlackComposerCaptureOperation,
    },
    CancellingPermission {
        operation: SlackComposerCaptureOperation,
    },
    Recording {
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        started_at: Instant,
        displayed_seconds: u64,
    },
    Finalizing {
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        duration: Duration,
    },
    Cancelling {
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        duration: Duration,
        terminal_error: Option<String>,
    },
    Failed {
        operation: SlackComposerCaptureOperation,
        duration: Duration,
        diagnostic: String,
    },
    Cancelled {
        operation: SlackComposerCaptureOperation,
        duration: Duration,
    },
}

impl SlackAudioClipCaptureState {
    pub(crate) fn operation(&self) -> &SlackComposerCaptureOperation {
        match self {
            Self::RequestingPermission { operation }
            | Self::CancellingPermission { operation }
            | Self::Recording { operation, .. }
            | Self::Finalizing { operation, .. }
            | Self::Cancelling { operation, .. }
            | Self::Failed { operation, .. }
            | Self::Cancelled { operation, .. } => operation,
        }
    }

    pub(crate) fn terminal(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::Cancelled { .. })
    }

    pub(crate) fn blocks_draft_submission(&self) -> bool {
        matches!(
            self,
            Self::RequestingPermission { .. }
                | Self::CancellingPermission { .. }
                | Self::Recording { .. }
                | Self::Finalizing { .. }
                | Self::Cancelling { .. }
        )
    }
}
