mod cancellation;
mod control;
mod finalization;
mod polling;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::model::{ChatAudioClipCaptureState, ChatAudioClipCaptureStatus};

use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackAudioClipCaptureState, SlackComposerCaptureGeneration, SlackComposerCaptureOperation,
    SlackPreparedUploadFile, SLACK_COMPOSER_FILE_LIMIT,
};
use crate::ui::{
    AudioClipCaptureStatus, AudioClipSessionId, CapturedAudioClip, MediaCaptureApi,
    SlackUploadFile, MAX_AUDIO_CLIP_DURATION,
};

const AUDIO_CLIP_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);

struct SlackAudioClipSessionTask {
    operation: SlackComposerCaptureOperation,
    session_id: AudioClipSessionId,
    duration: Duration,
}

impl SlackAudioClipSessionTask {
    fn new(
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        duration: Duration,
    ) -> Self {
        Self {
            operation,
            session_id,
            duration,
        }
    }

    fn cancelling(self, terminal_error: Option<String>) -> SlackAudioClipCancellationTask {
        SlackAudioClipCancellationTask {
            session: self,
            terminal_error,
        }
    }
}

struct SlackAudioClipCancellationTask {
    session: SlackAudioClipSessionTask,
    terminal_error: Option<String>,
}

impl SurfaceState {
    pub(crate) fn can_start_slack_audio_clip_capture(&self) -> bool {
        self.media_capture_api
            .as_ref()
            .is_some_and(|api| api.capabilities().audio_clip)
            && self.slack_composer_capture.can_start()
            && self.can_mutate_current_slack_send_draft()
            && self.slack_active_scheduled_edit.is_none()
            && self.slack_workspace_api_capabilities.upload_files
            && self.slack_workspace_api_capabilities.stage_file
            && self.slack_composer_files.len() < SLACK_COMPOSER_FILE_LIMIT
    }

    pub(crate) fn start_slack_audio_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        if !self.can_start_slack_audio_clip_capture() {
            return Err(
                "Audio clip recording is unavailable for the active Slack composer.".to_string(),
            );
        }
        let api = self
            .media_capture_api
            .clone()
            .ok_or_else(|| "Audio clip recording is not configured.".to_string())?;
        let owner = self
            .current_slack_main_composer_draft_handle()
            .ok_or_else(|| "There is no active Slack composer draft.".to_string())?;
        let operation = self.slack_composer_capture.next_operation(owner);
        self.slack_composer_capture
            .set_audio(SlackAudioClipCaptureState::RequestingPermission {
                operation: operation.clone(),
            });
        cx.notify();

        self.spawn_background_task(
            (api, operation),
            cx,
            |(api, operation)| {
                let result = api.start_audio_clip();
                (api, operation, result)
            },
            |this, (api, operation, result), cx| {
                this.apply_slack_audio_clip_start(api, operation, result, cx);
            },
        );
        Ok(self.control_slack_audio_clip_capture_state())
    }

    fn apply_slack_audio_clip_start(
        &mut self,
        api: Arc<MediaCaptureApi>,
        operation: SlackComposerCaptureOperation,
        result: Result<AudioClipSessionId, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let (request_is_current, cancellation_is_pending) =
            self.slack_audio_clip_start_status(&operation);
        match result {
            Ok(session_id)
                if request_is_current
                    && self.active
                    && self.slack_audio_clip_operation_is_current(&operation) =>
            {
                self.apply_current_slack_audio_clip_session(operation, session_id, cx);
            }
            Ok(session_id) if cancellation_is_pending => {
                self.begin_slack_audio_clip_cancellation(
                    SlackAudioClipSessionTask::new(operation, session_id, Duration::ZERO)
                        .cancelling(None),
                    cx,
                );
            }
            Ok(session_id) if request_is_current => {
                self.begin_slack_audio_clip_cancellation(
                    SlackAudioClipSessionTask::new(operation, session_id, Duration::ZERO)
                        .cancelling(None),
                    cx,
                );
            }
            Ok(session_id) => {
                self.spawn_stale_slack_audio_clip_cancellation(api, operation, session_id, cx);
            }
            Err(error) if request_is_current => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::Failed {
                        operation,
                        duration: Duration::ZERO,
                        diagnostic: error.to_string(),
                    });
                cx.notify();
            }
            Err(_) if cancellation_is_pending => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::Cancelled {
                        operation,
                        duration: Duration::ZERO,
                    });
                cx.notify();
            }
            Err(_) => {}
        }
    }

    fn slack_audio_clip_start_status(
        &self,
        operation: &SlackComposerCaptureOperation,
    ) -> (bool, bool) {
        let state = self.slack_composer_capture.audio();
        let request_is_current = matches!(
            state,
            Some(SlackAudioClipCaptureState::RequestingPermission {
                operation: current,
            }) if current == operation
        );
        let cancellation_is_pending = matches!(
            state,
            Some(SlackAudioClipCaptureState::CancellingPermission {
                operation: current,
            }) if current == operation
        );
        (request_is_current, cancellation_is_pending)
    }

    fn apply_current_slack_audio_clip_session(
        &mut self,
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        cx: &mut Context<Self>,
    ) {
        self.slack_composer_capture
            .set_audio(SlackAudioClipCaptureState::Recording {
                operation: operation.clone(),
                session_id,
                started_at: Instant::now(),
                displayed_seconds: 0,
            });
        self.schedule_slack_audio_clip_status_poll(operation.generation, session_id, cx);
        cx.notify();
    }

    fn slack_audio_clip_operation_is_current(
        &self,
        operation: &SlackComposerCaptureOperation,
    ) -> bool {
        self.current_slack_main_composer_draft_handle().as_ref() == Some(&operation.owner)
    }
}
