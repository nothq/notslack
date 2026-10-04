use super::{
    CapturedVideoClip, ChatVideoClipCaptureState, Context, SlackComposerCaptureGeneration,
    SlackComposerCaptureOperation, SlackComposerFileId, SlackPreparedUploadFile,
    SlackVideoClipCaptureState, SlackVideoClipModalPhase, SlackVideoClipModalTransition,
    SlackVideoClipReviewArtifact, SlackVideoClipSessionTask, SurfaceState, VideoClipSessionId,
    GENERIC_VIDEO_CLIP_FILENAME,
};

mod review;

impl SurfaceState {
    pub(crate) fn stop_slack_video_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let generation = self
            .slack_composer_capture
            .video()
            .map(|state| state.operation().generation)
            .ok_or_else(|| "There is no active Slack video clip recording to stop.".to_string())?;
        self.stop_slack_video_clip_capture_for_generation(generation, cx)
    }

    pub(crate) fn stop_slack_video_clip_capture_for_generation(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let modal = self
            .slack_video_clip_modal
            .as_ref()
            .filter(|modal| modal.read(cx).generation() == generation)
            .cloned();
        if modal.is_some_and(|modal| modal.update(cx, |modal, cx| modal.cancel_countdown(cx))) {
            return Ok(self.control_slack_video_clip_capture_state(cx));
        }
        let task = match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Recording {
                operation,
                session_id,
                snapshot,
                recording_started_at_unix_millis,
            }) if operation.generation == generation => SlackVideoClipSessionTask {
                operation: operation.clone(),
                session_id: *session_id,
                snapshot: snapshot.clone(),
                recording_started_at_unix_millis: Some(*recording_started_at_unix_millis),
            },
            Some(
                SlackVideoClipCaptureState::Finalizing { operation, .. }
                | SlackVideoClipCaptureState::Attaching { operation, .. },
            ) if operation.generation == generation => {
                return Ok(self.control_slack_video_clip_capture_state(cx));
            }
            _ => {
                return Err("There is no active Slack video clip recording to stop.".to_string());
            }
        };
        if !self.slack_composer_capture_operation_is_current(&task.operation) {
            self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
            return Err(
                "The Slack composer draft changed before the video clip could be stopped."
                    .to_string(),
            );
        }
        self.begin_slack_video_clip_finalization(task, cx);
        Ok(self.control_slack_video_clip_capture_state(cx))
    }

    pub(super) fn begin_slack_video_clip_finalization(
        &mut self,
        task: SlackVideoClipSessionTask,
        cx: &mut Context<Self>,
    ) {
        let recording_started_at_unix_millis = task
            .recording_started_at_unix_millis
            .expect("only a started video clip may be finalized");
        let api = self
            .media_capture_api
            .clone()
            .expect("an active video clip session must retain its media capture service");
        self.set_slack_video_clip_modal_phase(
            &task.operation,
            SlackVideoClipModalTransition::new(
                SlackVideoClipModalPhase::Finalizing,
                task.snapshot.duration,
            ),
            cx,
        );
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Finalizing {
                operation: task.operation.clone(),
                session_id: task.session_id,
                snapshot: task.snapshot.clone(),
                recording_started_at_unix_millis,
            });
        cx.notify();
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api.stop_video_clip(task.session_id);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_video_clip_finalization(task, result, cx);
            },
        );
    }

    fn apply_slack_video_clip_finalization(
        &mut self,
        task: SlackVideoClipSessionTask,
        result: Result<CapturedVideoClip, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let finalization_is_current = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::Finalizing {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.operation && *current_session == task.session_id
        );
        if !finalization_is_current {
            return;
        }
        let recording_started_at_unix_millis = task
            .recording_started_at_unix_millis
            .expect("a finalized video clip must retain its recording start timestamp");
        if !self.active || !self.slack_composer_capture_operation_is_current(&task.operation) {
            self.cancel_finalized_slack_video_clip(task, recording_started_at_unix_millis, cx);
            return;
        }
        match result {
            Ok(clip) => {
                self.review_finalized_slack_video_clip(
                    task,
                    recording_started_at_unix_millis,
                    clip,
                    cx,
                );
            }
            Err(error) => self.fail_slack_video_clip_finalization(
                task,
                recording_started_at_unix_millis,
                error.to_string(),
                cx,
            ),
        }
    }

    fn cancel_finalized_slack_video_clip(
        &mut self,
        task: SlackVideoClipSessionTask,
        recording_started_at_unix_millis: i64,
        cx: &mut Context<Self>,
    ) {
        let generation = task.operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Cancelled {
                operation: task.operation,
                duration: task.snapshot.duration,
                recording_started_at_unix_millis: Some(recording_started_at_unix_millis),
            });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
    }

    fn review_finalized_slack_video_clip(
        &mut self,
        task: SlackVideoClipSessionTask,
        recording_started_at_unix_millis: i64,
        clip: CapturedVideoClip,
        cx: &mut Context<Self>,
    ) {
        let artifact = SlackVideoClipReviewArtifact::new(clip);
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Reviewing {
                operation: task.operation.clone(),
                session_id: task.session_id,
                recording_started_at_unix_millis,
                artifact: artifact.clone(),
            });
        self.begin_slack_video_clip_review(&task.operation, &artifact, cx);
        cx.notify();
    }

    fn fail_slack_video_clip_finalization(
        &mut self,
        task: SlackVideoClipSessionTask,
        recording_started_at_unix_millis: i64,
        diagnostic: String,
        cx: &mut Context<Self>,
    ) {
        self.set_slack_video_clip_modal_phase(
            &task.operation,
            SlackVideoClipModalTransition::new(
                SlackVideoClipModalPhase::Failed,
                task.snapshot.duration,
            )
            .with_diagnostic(Some(diagnostic.clone())),
            cx,
        );
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Failed {
                operation: task.operation,
                session_id: Some(task.session_id),
                duration: task.snapshot.duration,
                recording_started_at_unix_millis: Some(recording_started_at_unix_millis),
                diagnostic,
            });
        cx.notify();
    }
}
