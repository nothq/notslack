use super::{
    Arc, AudioClipSessionId, ChatAudioClipCaptureState, Context, Duration, MediaCaptureApi,
    SlackAudioClipCancellationTask, SlackAudioClipCaptureState, SlackAudioClipSessionTask,
    SlackComposerCaptureOperation, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn cancel_slack_audio_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        match self.slack_composer_capture.audio().cloned() {
            Some(SlackAudioClipCaptureState::RequestingPermission { operation }) => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::CancellingPermission { operation });
                cx.notify();
            }
            Some(SlackAudioClipCaptureState::CancellingPermission { .. }) => {}
            Some(SlackAudioClipCaptureState::Recording {
                operation,
                session_id,
                started_at,
                ..
            }) => {
                self.begin_slack_audio_clip_cancellation(
                    SlackAudioClipSessionTask::new(operation, session_id, started_at.elapsed())
                        .cancelling(None),
                    cx,
                );
            }
            Some(SlackAudioClipCaptureState::Cancelling { .. }) => {}
            Some(SlackAudioClipCaptureState::Finalizing { .. }) => {
                return Err("The Slack audio clip is already being finalized.".to_string());
            }
            None
            | Some(
                SlackAudioClipCaptureState::Failed { .. }
                | SlackAudioClipCaptureState::Cancelled { .. },
            ) => {
                return Err("There is no active Slack audio clip recording to cancel.".to_string());
            }
        }
        Ok(self.control_slack_audio_clip_capture_state())
    }

    pub(super) fn begin_slack_audio_clip_cancellation(
        &mut self,
        task: SlackAudioClipCancellationTask,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.media_capture_api.clone() else {
            self.slack_composer_capture
                .set_audio(SlackAudioClipCaptureState::Failed {
                    operation: task.session.operation,
                    duration: task.session.duration,
                    diagnostic: "Audio clip recording is no longer configured.".to_string(),
                });
            cx.notify();
            return;
        };
        self.slack_composer_capture
            .set_audio(SlackAudioClipCaptureState::Cancelling {
                operation: task.session.operation.clone(),
                session_id: task.session.session_id,
                duration: task.session.duration,
                terminal_error: task.terminal_error.clone(),
            });
        cx.notify();
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api.cancel_audio_clip(task.session.session_id);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_audio_clip_cancellation(task, result, cx);
            },
        );
    }

    fn apply_slack_audio_clip_cancellation(
        &mut self,
        task: SlackAudioClipCancellationTask,
        result: Result<(), crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let cancellation_is_current = matches!(
            self.slack_composer_capture.audio(),
            Some(SlackAudioClipCaptureState::Cancelling {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.session.operation
                && *current_session == task.session.session_id
        );
        if !cancellation_is_current {
            return;
        }
        let diagnostic = match (task.terminal_error, result) {
            (Some(diagnostic), Ok(())) => Some(diagnostic),
            (Some(diagnostic), Err(error)) => {
                Some(format!("{diagnostic}; audio clip cleanup failed: {error}"))
            }
            (None, Err(error)) => Some(error.to_string()),
            (None, Ok(())) => None,
        };
        let state = match diagnostic {
            Some(diagnostic) => SlackAudioClipCaptureState::Failed {
                operation: task.session.operation,
                duration: task.session.duration,
                diagnostic,
            },
            None => SlackAudioClipCaptureState::Cancelled {
                operation: task.session.operation,
                duration: task.session.duration,
            },
        };
        self.slack_composer_capture.set_audio(state);
        cx.notify();
    }

    pub(super) fn spawn_stale_slack_audio_clip_cancellation(
        &mut self,
        api: Arc<MediaCaptureApi>,
        operation: SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (api, operation, session_id),
            cx,
            |(api, operation, session_id)| (operation, api.cancel_audio_clip(session_id)),
            |this, (operation, result), cx| {
                let Err(error) = result else {
                    return;
                };
                let diagnostic = format!("Failed to clean up a stale audio clip session: {error}");
                this.slack_error = Some(diagnostic.clone());
                if this
                    .slack_composer_capture
                    .operation()
                    .is_some_and(|current| current == &operation)
                {
                    this.slack_composer_capture
                        .set_audio(SlackAudioClipCaptureState::Failed {
                            operation,
                            duration: Duration::ZERO,
                            diagnostic,
                        });
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn cancel_slack_audio_clip_for_owner_change(&mut self, cx: &mut Context<Self>) {
        match self.slack_composer_capture.audio().cloned() {
            Some(SlackAudioClipCaptureState::RequestingPermission { operation }) => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::CancellingPermission { operation });
                cx.notify();
            }
            Some(SlackAudioClipCaptureState::CancellingPermission { .. }) => {}
            Some(SlackAudioClipCaptureState::Recording {
                operation,
                session_id,
                started_at,
                ..
            }) => self.begin_slack_audio_clip_cancellation(
                SlackAudioClipSessionTask::new(operation, session_id, started_at.elapsed())
                    .cancelling(None),
                cx,
            ),
            None
            | Some(
                SlackAudioClipCaptureState::Finalizing { .. }
                | SlackAudioClipCaptureState::Cancelling { .. }
                | SlackAudioClipCaptureState::Failed { .. }
                | SlackAudioClipCaptureState::Cancelled { .. },
            ) => {}
        }
    }
}
