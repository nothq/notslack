use super::{
    current_unix_millis, ChatVideoClipCaptureState, Context, Duration,
    SlackComposerCaptureGeneration, SlackVideoClipCaptureState, SlackVideoClipModalPhase,
    SlackVideoClipModalTransition, SlackVideoClipRecordingStartTask, SlackVideoClipSessionTask,
    SlackVideoClipSnapshot, SurfaceState, VideoClipCaptureStatus,
};

#[derive(Clone, Copy)]
struct SlackVideoClipRecordingStartRequestState {
    starting_is_current: bool,
    cancellation_is_pending: bool,
    installable: bool,
}

impl SurfaceState {
    pub(crate) fn begin_slack_video_clip_recording(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let operation = match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Prepared {
                operation,
                snapshot,
                ..
            }) if matches!(snapshot.status, VideoClipCaptureStatus::Previewing) => {
                operation.clone()
            }
            Some(SlackVideoClipCaptureState::Prepared { .. }) => {
                return Err(
                    "The Slack video clip preview is not ready to begin recording.".to_string(),
                );
            }
            Some(
                SlackVideoClipCaptureState::Starting { .. }
                | SlackVideoClipCaptureState::Recording { .. },
            ) => return Ok(self.control_slack_video_clip_capture_state(cx)),
            _ => {
                return Err(
                    "There is no prepared Slack video clip ready to begin recording.".to_string(),
                );
            }
        };
        let modal = self
            .slack_video_clip_modal
            .as_ref()
            .filter(|modal| modal.read(cx).generation() == operation.generation)
            .cloned()
            .ok_or_else(|| "The Slack video clip recorder is no longer open.".to_string())?;
        modal.update(cx, |modal, cx| modal.start_countdown(cx));
        Ok(self.control_slack_video_clip_capture_state(cx))
    }

    pub(crate) fn begin_slack_video_clip_recording_after_countdown(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let Some(task) = self.slack_video_clip_session_for_recording(generation)? else {
            return Ok(self.control_slack_video_clip_capture_state(cx));
        };
        if !self.slack_composer_capture_operation_is_current(&task.operation) {
            self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
            return Err(
                "The Slack composer draft changed before video recording could begin.".to_string(),
            );
        }
        self.start_prepared_slack_video_clip_recording(task, cx)?;
        Ok(self.control_slack_video_clip_capture_state(cx))
    }

    fn slack_video_clip_session_for_recording(
        &self,
        generation: SlackComposerCaptureGeneration,
    ) -> Result<Option<SlackVideoClipSessionTask>, String> {
        match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Prepared {
                operation,
                session_id,
                snapshot,
            }) if operation.generation == generation
                && matches!(snapshot.status, VideoClipCaptureStatus::Previewing) =>
            {
                Ok(Some(SlackVideoClipSessionTask {
                    operation: operation.clone(),
                    session_id: *session_id,
                    snapshot: snapshot.clone(),
                    recording_started_at_unix_millis: None,
                }))
            }
            Some(SlackVideoClipCaptureState::Prepared { .. }) => {
                Err("The Slack video clip preview is not ready to begin recording.".to_string())
            }
            Some(SlackVideoClipCaptureState::Starting { .. }) => Ok(None),
            _ => Err("There is no prepared Slack video clip ready to begin recording.".to_string()),
        }
    }

    fn start_prepared_slack_video_clip_recording(
        &mut self,
        task: SlackVideoClipSessionTask,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let api = self
            .media_capture_api
            .clone()
            .ok_or_else(|| "Video clip recording is not configured.".to_string())?;
        let recording_started_at_unix_millis = current_unix_millis();
        let duration = task.snapshot.duration;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Starting {
                operation: task.operation.clone(),
                session_id: task.session_id,
                snapshot: task.snapshot,
                recording_started_at_unix_millis,
            });
        self.set_slack_video_clip_modal_phase(
            &task.operation,
            SlackVideoClipModalTransition::new(SlackVideoClipModalPhase::Starting, duration),
            cx,
        );
        cx.notify();

        self.spawn_background_task(
            SlackVideoClipRecordingStartTask {
                api,
                operation: task.operation,
                session_id: task.session_id,
                recording_started_at_unix_millis,
            },
            cx,
            |task| {
                let result = task
                    .api
                    .begin_video_clip_recording(task.session_id)
                    .and_then(|()| task.api.video_clip_status(task.session_id))
                    .map(SlackVideoClipSnapshot::from);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_video_clip_recording_start(task, result, cx);
            },
        );
        Ok(())
    }

    fn apply_slack_video_clip_recording_start(
        &mut self,
        task: SlackVideoClipRecordingStartTask,
        result: Result<SlackVideoClipSnapshot, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let starting_is_current = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::Starting {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.operation && *current_session == task.session_id
        );
        let cancellation_is_pending = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::CancellingStart {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.operation && *current_session == task.session_id
        );
        let state = SlackVideoClipRecordingStartRequestState {
            starting_is_current,
            cancellation_is_pending,
            installable: starting_is_current
                && self.active
                && self.slack_composer_capture_operation_is_current(&task.operation),
        };
        match result {
            Ok(snapshot) => {
                self.apply_successful_slack_video_clip_recording_start(task, snapshot, state, cx);
            }
            Err(error) => {
                self.apply_failed_slack_video_clip_recording_start(task, error, state, cx);
            }
        }
    }

    fn apply_successful_slack_video_clip_recording_start(
        &mut self,
        task: SlackVideoClipRecordingStartTask,
        snapshot: SlackVideoClipSnapshot,
        state: SlackVideoClipRecordingStartRequestState,
        cx: &mut Context<Self>,
    ) {
        if state.installable {
            self.install_started_slack_video_clip_recording(task, snapshot, cx);
        } else if state.cancellation_is_pending || state.starting_is_current {
            let session = slack_video_clip_recording_session(task, snapshot);
            self.begin_slack_video_clip_cancellation(session.cancelling(None), cx);
        } else {
            self.spawn_stale_slack_video_clip_cancellation(
                task.api,
                task.operation,
                task.session_id,
                cx,
            );
        }
    }

    fn install_started_slack_video_clip_recording(
        &mut self,
        task: SlackVideoClipRecordingStartTask,
        snapshot: SlackVideoClipSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.sync_slack_video_clip_modal_snapshot(&task.operation, &snapshot, cx);
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Recording {
                operation: task.operation.clone(),
                session_id: task.session_id,
                snapshot,
                recording_started_at_unix_millis: task.recording_started_at_unix_millis,
            });
        self.schedule_slack_video_clip_status_poll(task.operation.generation, task.session_id, cx);
        cx.notify();
    }

    fn apply_failed_slack_video_clip_recording_start(
        &mut self,
        task: SlackVideoClipRecordingStartTask,
        error: crate::ui::MediaCaptureError,
        state: SlackVideoClipRecordingStartRequestState,
        cx: &mut Context<Self>,
    ) {
        let terminal_error = if state.starting_is_current {
            Some(error.to_string())
        } else if state.cancellation_is_pending {
            None
        } else {
            return;
        };
        let session = failed_slack_video_clip_recording_session(task);
        self.begin_slack_video_clip_cancellation(session.cancelling(terminal_error), cx);
    }
}

fn slack_video_clip_recording_session(
    task: SlackVideoClipRecordingStartTask,
    snapshot: SlackVideoClipSnapshot,
) -> SlackVideoClipSessionTask {
    SlackVideoClipSessionTask {
        operation: task.operation,
        session_id: task.session_id,
        snapshot,
        recording_started_at_unix_millis: Some(task.recording_started_at_unix_millis),
    }
}

fn failed_slack_video_clip_recording_session(
    task: SlackVideoClipRecordingStartTask,
) -> SlackVideoClipSessionTask {
    SlackVideoClipSessionTask {
        operation: task.operation,
        session_id: task.session_id,
        snapshot: SlackVideoClipSnapshot {
            status: VideoClipCaptureStatus::Ready,
            duration: Duration::ZERO,
            latest_preview: None,
        },
        recording_started_at_unix_millis: Some(task.recording_started_at_unix_millis),
    }
}
