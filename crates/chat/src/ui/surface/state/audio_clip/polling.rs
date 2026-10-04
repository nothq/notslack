use super::{
    AudioClipCaptureStatus, AudioClipSessionId, Context, SlackAudioClipCaptureState,
    SlackAudioClipSessionTask, SlackComposerCaptureGeneration, SlackComposerCaptureOperation,
    SurfaceState, AUDIO_CLIP_STATUS_POLL_INTERVAL, MAX_AUDIO_CLIP_DURATION,
};

impl SurfaceState {
    pub(super) fn schedule_slack_audio_clip_status_poll(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        session_id: AudioClipSessionId,
        cx: &mut Context<Self>,
    ) {
        self.spawn_timer_task(
            (generation, session_id),
            AUDIO_CLIP_STATUS_POLL_INTERVAL,
            cx,
            |this, (generation, session_id), cx| {
                this.poll_slack_audio_clip_status(generation, session_id, cx);
            },
        );
    }

    fn poll_slack_audio_clip_status(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        session_id: AudioClipSessionId,
        cx: &mut Context<Self>,
    ) {
        let task = match self.slack_composer_capture.audio() {
            Some(SlackAudioClipCaptureState::Recording {
                operation,
                session_id: current_session,
                started_at,
                ..
            }) if operation.generation == generation && *current_session == session_id => {
                SlackAudioClipSessionTask::new(operation.clone(), session_id, started_at.elapsed())
            }
            _ => return,
        };
        if !self.active || !self.slack_audio_clip_operation_is_current(&task.operation) {
            self.begin_slack_audio_clip_cancellation(task.cancelling(None), cx);
            return;
        }
        if task.duration >= MAX_AUDIO_CLIP_DURATION {
            self.begin_slack_audio_clip_finalization(task, cx);
            return;
        }
        let Some(api) = self.media_capture_api.clone() else {
            self.begin_slack_audio_clip_cancellation(
                task.cancelling(Some(
                    "Audio clip recording is no longer configured.".to_string(),
                )),
                cx,
            );
            return;
        };
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api.audio_clip_status(task.session_id);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_audio_clip_status(task, result, cx);
            },
        );
    }

    fn apply_slack_audio_clip_status(
        &mut self,
        task: SlackAudioClipSessionTask,
        result: Result<AudioClipCaptureStatus, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let poll_is_current = matches!(
            self.slack_composer_capture.audio(),
            Some(SlackAudioClipCaptureState::Recording {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.operation && *current_session == task.session_id
        );
        if !poll_is_current {
            return;
        }
        match result {
            Ok(AudioClipCaptureStatus::Recording) => {
                self.notify_if_slack_audio_clip_second_changed(
                    &task.operation,
                    task.session_id,
                    cx,
                );
                self.schedule_slack_audio_clip_status_poll(
                    task.operation.generation,
                    task.session_id,
                    cx,
                );
            }
            Ok(AudioClipCaptureStatus::DurationLimitReached) => {
                self.begin_slack_audio_clip_finalization(task, cx);
            }
            Ok(AudioClipCaptureStatus::Failed(failure)) => {
                self.begin_slack_audio_clip_cancellation(
                    task.cancelling(Some(failure.to_string())),
                    cx,
                );
            }
            Err(error) => {
                self.begin_slack_audio_clip_cancellation(
                    task.cancelling(Some(error.to_string())),
                    cx,
                );
            }
        }
    }

    fn notify_if_slack_audio_clip_second_changed(
        &mut self,
        operation: &SlackComposerCaptureOperation,
        session_id: AudioClipSessionId,
        cx: &mut Context<Self>,
    ) {
        let SlackAudioClipCaptureState::Recording {
            operation: current,
            session_id: current_session,
            started_at,
            displayed_seconds,
        } = self.slack_composer_capture.audio_mut().unwrap_or_else(|| {
            panic!("an active audio status poll must retain an audio capture state")
        })
        else {
            return;
        };
        if current != operation || *current_session != session_id {
            return;
        }
        let elapsed_seconds = started_at.elapsed().as_secs();
        if *displayed_seconds != elapsed_seconds {
            *displayed_seconds = elapsed_seconds;
            cx.notify();
        }
    }
}
