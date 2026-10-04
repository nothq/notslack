use super::{
    Arc, ChatVideoClipCaptureState, Context, Duration, MediaCaptureApi,
    SlackComposerCaptureGeneration, SlackComposerCaptureOperation, SlackVideoClipCancellationTask,
    SlackVideoClipCaptureState, SlackVideoClipModalPhase, SlackVideoClipModalTransition,
    SlackVideoClipSessionTask, SlackVideoClipSnapshot, SurfaceState, VideoClipSessionId, Window,
};

mod owner_change;
mod stale;

struct SlackVideoClipStartCancellation {
    operation: SlackComposerCaptureOperation,
    session_id: VideoClipSessionId,
    snapshot: SlackVideoClipSnapshot,
    recording_started_at_unix_millis: i64,
}

impl SurfaceState {
    pub(crate) fn cancel_slack_video_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let generation = self
            .slack_composer_capture
            .video()
            .map(|state| state.operation().generation)
            .ok_or_else(|| "There is no active Slack video clip to cancel.".to_string())?;
        self.cancel_slack_video_clip_capture_for_generation(generation, cx)
    }

    pub(crate) fn cancel_slack_video_clip_capture_for_generation(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let Some(state) = self.slack_composer_capture.video().cloned() else {
            return Err("There is no active Slack video clip to cancel.".to_string());
        };
        if state.operation().generation != generation {
            return Err("There is no active Slack video clip to cancel.".to_string());
        }
        let recording_started_at_unix_millis = state.recording_started_at_unix_millis();
        self.cancel_slack_video_clip_state(state, recording_started_at_unix_millis, cx)?;
        Ok(self.control_slack_video_clip_capture_state(cx))
    }

    fn cancel_slack_video_clip_state(
        &mut self,
        state: SlackVideoClipCaptureState,
        recording_started_at_unix_millis: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match state {
            state @ (SlackVideoClipCaptureState::Preparing { .. }
            | SlackVideoClipCaptureState::Starting { .. }) => {
                self.cancel_pending_slack_video_clip_state(state, cx);
            }
            state @ (SlackVideoClipCaptureState::Prepared { .. }
            | SlackVideoClipCaptureState::Recording { .. }) => {
                self.cancel_active_slack_video_clip_state(
                    state,
                    recording_started_at_unix_millis,
                    cx,
                );
            }
            state @ (SlackVideoClipCaptureState::Reviewing { .. }
            | SlackVideoClipCaptureState::Failed { .. }) => {
                self.cancel_finished_slack_video_clip_state(state, cx);
            }
            SlackVideoClipCaptureState::Finalizing { .. }
            | SlackVideoClipCaptureState::Attaching { .. } => {
                return Err("The Slack video clip is already being finalized.".to_string());
            }
            SlackVideoClipCaptureState::Attached { .. } => {
                return Err("The Slack video clip is already attached.".to_string());
            }
            SlackVideoClipCaptureState::CancellingPreparation { .. }
            | SlackVideoClipCaptureState::CancellingStart { .. }
            | SlackVideoClipCaptureState::Cancelling { .. }
            | SlackVideoClipCaptureState::Cancelled { .. } => {}
        }
        Ok(())
    }

    fn cancel_pending_slack_video_clip_state(
        &mut self,
        state: SlackVideoClipCaptureState,
        cx: &mut Context<Self>,
    ) {
        match state {
            SlackVideoClipCaptureState::Preparing { operation } => {
                self.cancel_slack_video_clip_preparation(operation, cx);
            }
            SlackVideoClipCaptureState::Starting {
                operation,
                session_id,
                snapshot,
                recording_started_at_unix_millis,
            } => self.cancel_slack_video_clip_start(
                SlackVideoClipStartCancellation {
                    operation,
                    session_id,
                    snapshot,
                    recording_started_at_unix_millis,
                },
                cx,
            ),
            _ => unreachable!("pending video clip cancellation requires a pending state"),
        }
    }

    fn cancel_active_slack_video_clip_state(
        &mut self,
        state: SlackVideoClipCaptureState,
        recording_started_at_unix_millis: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        match state {
            SlackVideoClipCaptureState::Prepared {
                operation,
                session_id,
                snapshot,
            }
            | SlackVideoClipCaptureState::Recording {
                operation,
                session_id,
                snapshot,
                ..
            } => self.cancel_slack_video_clip_session(
                SlackVideoClipSessionTask {
                    operation,
                    session_id,
                    snapshot,
                    recording_started_at_unix_millis,
                },
                cx,
            ),
            _ => unreachable!("active video clip cancellation requires an active state"),
        }
    }

    fn cancel_finished_slack_video_clip_state(
        &mut self,
        state: SlackVideoClipCaptureState,
        cx: &mut Context<Self>,
    ) {
        match state {
            SlackVideoClipCaptureState::Reviewing {
                operation,
                recording_started_at_unix_millis,
                artifact,
                ..
            } => self.finish_slack_video_clip_cancellation(
                operation,
                artifact.clip().duration(),
                Some(recording_started_at_unix_millis),
                cx,
            ),
            SlackVideoClipCaptureState::Failed {
                operation,
                duration,
                recording_started_at_unix_millis,
                ..
            } => self.finish_slack_video_clip_cancellation(
                operation,
                duration,
                recording_started_at_unix_millis,
                cx,
            ),
            _ => unreachable!("finished video clip cancellation requires a finished state"),
        }
    }

    fn cancel_slack_video_clip_preparation(
        &mut self,
        operation: SlackComposerCaptureOperation,
        cx: &mut Context<Self>,
    ) {
        let generation = operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::CancellingPreparation { operation });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
    }

    fn cancel_slack_video_clip_start(
        &mut self,
        cancellation: SlackVideoClipStartCancellation,
        cx: &mut Context<Self>,
    ) {
        let SlackVideoClipStartCancellation {
            operation,
            session_id,
            snapshot,
            recording_started_at_unix_millis,
        } = cancellation;
        let generation = operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::CancellingStart {
                operation,
                session_id,
                snapshot,
                recording_started_at_unix_millis,
            });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
    }

    fn cancel_slack_video_clip_session(
        &mut self,
        task: SlackVideoClipSessionTask,
        cx: &mut Context<Self>,
    ) {
        self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
    }

    fn finish_slack_video_clip_cancellation(
        &mut self,
        operation: SlackComposerCaptureOperation,
        duration: Duration,
        recording_started_at_unix_millis: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        let generation = operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Cancelled {
                operation,
                duration,
                recording_started_at_unix_millis,
            });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
    }

    pub(crate) fn upload_video_from_slack_clip_modal(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let ready = matches!(
            self.slack_composer_capture.video(),
            Some(
                SlackVideoClipCaptureState::Preparing { operation }
                    | SlackVideoClipCaptureState::Prepared { operation, .. }
            ) if operation.generation == generation
                && self.slack_composer_capture_operation_is_current(operation)
        );
        if !ready {
            return Err("The video clip recorder no longer owns the active draft.".to_string());
        }
        self.prompt_for_single_slack_video_attachment(generation, window, cx)
    }

    pub(super) fn begin_slack_video_clip_cancellation(
        &mut self,
        task: SlackVideoClipCancellationTask,
        cx: &mut Context<Self>,
    ) {
        let api = self
            .media_capture_api
            .clone()
            .expect("an active video clip session must retain its media capture service");
        self.set_slack_video_clip_modal_phase(
            &task.session.operation,
            SlackVideoClipModalTransition::new(
                SlackVideoClipModalPhase::Cancelling,
                task.session.snapshot.duration,
            )
            .with_diagnostic(task.terminal_error.clone()),
            cx,
        );
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Cancelling {
                operation: task.session.operation.clone(),
                session_id: task.session.session_id,
                snapshot: task.session.snapshot.clone(),
                recording_started_at_unix_millis: task.session.recording_started_at_unix_millis,
                terminal_error: task.terminal_error.clone(),
            });
        if task.terminal_error.is_none() {
            self.close_slack_video_clip_modal(task.session.operation.generation, cx);
        }
        cx.notify();
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api.cancel_video_clip(task.session.session_id);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_video_clip_cancellation(task, result, cx);
            },
        );
    }

    fn apply_slack_video_clip_cancellation(
        &mut self,
        task: SlackVideoClipCancellationTask,
        result: Result<(), crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let cancellation_is_current = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::Cancelling {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.session.operation
                && *current_session == task.session.session_id
        );
        if !cancellation_is_current {
            return;
        }
        let diagnostic = slack_video_clip_cancellation_diagnostic(task.terminal_error, result);
        if let Some(diagnostic) = diagnostic {
            self.fail_slack_video_clip_cancellation(task.session, diagnostic, cx);
        } else {
            self.complete_slack_video_clip_cancellation(task.session, cx);
        }
        cx.notify();
    }

    fn fail_slack_video_clip_cancellation(
        &mut self,
        session: SlackVideoClipSessionTask,
        diagnostic: String,
        cx: &mut Context<Self>,
    ) {
        self.set_slack_video_clip_modal_phase(
            &session.operation,
            SlackVideoClipModalTransition::new(
                SlackVideoClipModalPhase::Failed,
                session.snapshot.duration,
            )
            .with_diagnostic(Some(diagnostic.clone())),
            cx,
        );
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Failed {
                operation: session.operation,
                session_id: Some(session.session_id),
                duration: session.snapshot.duration,
                recording_started_at_unix_millis: session.recording_started_at_unix_millis,
                diagnostic,
            });
    }

    fn complete_slack_video_clip_cancellation(
        &mut self,
        session: SlackVideoClipSessionTask,
        cx: &mut Context<Self>,
    ) {
        self.finish_slack_video_clip_cancellation(
            session.operation,
            session.snapshot.duration,
            session.recording_started_at_unix_millis,
            cx,
        );
    }
}

fn slack_video_clip_cancellation_diagnostic(
    terminal_error: Option<String>,
    result: Result<(), crate::ui::MediaCaptureError>,
) -> Option<String> {
    match (terminal_error, result) {
        (Some(diagnostic), Ok(())) => Some(diagnostic),
        (Some(diagnostic), Err(error)) => {
            Some(format!("{diagnostic}; video clip cleanup failed: {error}"))
        }
        (None, Err(error)) => Some(error.to_string()),
        (None, Ok(())) => None,
    }
}
