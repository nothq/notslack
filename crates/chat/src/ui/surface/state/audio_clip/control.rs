use super::{
    ChatAudioClipCaptureState, ChatAudioClipCaptureStatus, Context, Duration,
    SlackAudioClipCaptureState, SurfaceState,
};

type SlackAudioClipControlFields = (
    ChatAudioClipCaptureStatus,
    Option<String>,
    Duration,
    Option<String>,
);

impl SurfaceState {
    pub(crate) fn slack_audio_clip_capture_visible_for_current_draft(&self) -> bool {
        self.slack_composer_capture.audio().is_some_and(|state| {
            !matches!(state, SlackAudioClipCaptureState::Cancelled { .. })
                && self.slack_audio_clip_operation_is_current(state.operation())
        })
    }

    pub(crate) fn dismiss_slack_audio_clip_capture_status(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.slack_composer_capture.audio(),
            Some(
                SlackAudioClipCaptureState::Failed { .. }
                    | SlackAudioClipCaptureState::Cancelled { .. }
            )
        ) {
            self.slack_composer_capture.set_idle();
            cx.notify();
        }
    }

    pub(crate) fn control_slack_audio_clip_capture_state(&self) -> ChatAudioClipCaptureState {
        let (status, session_id, duration, diagnostic) =
            slack_audio_clip_control_fields(self.slack_composer_capture.audio());
        let owned_by_current_draft = self
            .slack_composer_capture
            .audio()
            .map(SlackAudioClipCaptureState::operation)
            .is_some_and(|operation| self.slack_audio_clip_operation_is_current(operation));
        ChatAudioClipCaptureState {
            status,
            session_id,
            duration_millis: u64::try_from(duration.as_millis())
                .expect("bounded audio clip duration must fit in u64 milliseconds"),
            owned_by_current_draft,
            diagnostic,
        }
    }
}

fn slack_audio_clip_control_fields(
    state: Option<&SlackAudioClipCaptureState>,
) -> SlackAudioClipControlFields {
    match state {
        None => (ChatAudioClipCaptureStatus::Idle, None, Duration::ZERO, None),
        Some(SlackAudioClipCaptureState::RequestingPermission { .. }) => (
            ChatAudioClipCaptureStatus::RequestingPermission,
            None,
            Duration::ZERO,
            None,
        ),
        Some(SlackAudioClipCaptureState::CancellingPermission { .. }) => (
            ChatAudioClipCaptureStatus::Cancelling,
            None,
            Duration::ZERO,
            None,
        ),
        Some(SlackAudioClipCaptureState::Recording {
            session_id,
            started_at,
            ..
        }) => (
            ChatAudioClipCaptureStatus::Recording,
            Some(session_id.to_string()),
            started_at.elapsed(),
            None,
        ),
        Some(state) => slack_terminal_audio_clip_control_fields(state),
    }
}

fn slack_terminal_audio_clip_control_fields(
    state: &SlackAudioClipCaptureState,
) -> SlackAudioClipControlFields {
    match state {
        SlackAudioClipCaptureState::Finalizing {
            session_id,
            duration,
            ..
        } => (
            ChatAudioClipCaptureStatus::Finalizing,
            Some(session_id.to_string()),
            *duration,
            None,
        ),
        SlackAudioClipCaptureState::Cancelling {
            session_id,
            duration,
            terminal_error,
            ..
        } => (
            ChatAudioClipCaptureStatus::Cancelling,
            Some(session_id.to_string()),
            *duration,
            terminal_error.clone(),
        ),
        SlackAudioClipCaptureState::Failed {
            duration,
            diagnostic,
            ..
        } => (
            ChatAudioClipCaptureStatus::Failed,
            None,
            *duration,
            Some(diagnostic.clone()),
        ),
        SlackAudioClipCaptureState::Cancelled { duration, .. } => {
            (ChatAudioClipCaptureStatus::Cancelled, None, *duration, None)
        }
        SlackAudioClipCaptureState::RequestingPermission { .. }
        | SlackAudioClipCaptureState::CancellingPermission { .. }
        | SlackAudioClipCaptureState::Recording { .. } => {
            unreachable!("active audio clip states are projected before terminal states")
        }
    }
}
