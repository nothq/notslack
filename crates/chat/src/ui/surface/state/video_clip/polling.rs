use super::{
    Context, SlackComposerCaptureGeneration, SlackVideoClipCaptureState, SlackVideoClipSessionTask,
    SlackVideoClipSnapshot, SurfaceState, VideoClipCaptureStatus, VideoClipSessionId,
    VIDEO_CLIP_STATUS_POLL_INTERVAL,
};

impl SurfaceState {
    pub(super) fn schedule_slack_video_clip_status_poll(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        session_id: VideoClipSessionId,
        cx: &mut Context<Self>,
    ) {
        self.spawn_timer_task(
            (generation, session_id),
            VIDEO_CLIP_STATUS_POLL_INTERVAL,
            cx,
            |this, (generation, session_id), cx| {
                this.poll_slack_video_clip_status(generation, session_id, cx);
            },
        );
    }

    fn poll_slack_video_clip_status(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        session_id: VideoClipSessionId,
        cx: &mut Context<Self>,
    ) {
        let task = match self.slack_composer_capture.video() {
            Some(
                SlackVideoClipCaptureState::Prepared {
                    operation,
                    session_id: current_session,
                    snapshot,
                }
                | SlackVideoClipCaptureState::Recording {
                    operation,
                    session_id: current_session,
                    snapshot,
                    ..
                },
            ) if operation.generation == generation && *current_session == session_id => {
                SlackVideoClipSessionTask {
                    operation: operation.clone(),
                    session_id,
                    snapshot: snapshot.clone(),
                    recording_started_at_unix_millis: self
                        .slack_composer_capture
                        .video()
                        .and_then(SlackVideoClipCaptureState::recording_started_at_unix_millis),
                }
            }
            _ => return,
        };
        if !self.active || !self.slack_composer_capture_operation_is_current(&task.operation) {
            self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
            return;
        }
        let api = self
            .media_capture_api
            .clone()
            .expect("an active video clip session must retain its media capture service");
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api
                    .video_clip_status(task.session_id)
                    .map(SlackVideoClipSnapshot::from);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_video_clip_status(task, result, cx);
            },
        );
    }

    fn apply_slack_video_clip_status(
        &mut self,
        task: SlackVideoClipSessionTask,
        result: Result<SlackVideoClipSnapshot, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let poll_is_current = matches!(
            self.slack_composer_capture.video(),
            Some(
                SlackVideoClipCaptureState::Prepared {
                    operation: current,
                    session_id: current_session,
                    ..
                }
                | SlackVideoClipCaptureState::Recording {
                    operation: current,
                    session_id: current_session,
                    ..
                },
            ) if current == &task.operation && *current_session == task.session_id
        );
        if !poll_is_current {
            return;
        }
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.begin_slack_video_clip_cancellation(
                    task.cancelling(Some(error.to_string())),
                    cx,
                );
                return;
            }
        };
        self.apply_slack_video_clip_snapshot(task, snapshot, cx);
    }

    fn apply_slack_video_clip_snapshot(
        &mut self,
        task: SlackVideoClipSessionTask,
        snapshot: SlackVideoClipSnapshot,
        cx: &mut Context<Self>,
    ) {
        match snapshot.status.clone() {
            VideoClipCaptureStatus::RequestingPermissions
            | VideoClipCaptureStatus::Ready
            | VideoClipCaptureStatus::Previewing => {
                self.apply_slack_video_clip_preview_status(task, snapshot, cx);
            }
            VideoClipCaptureStatus::Recording => {
                self.apply_slack_video_clip_recording_status(task, snapshot, cx);
            }
            VideoClipCaptureStatus::DurationLimitReached => {
                self.begin_slack_video_clip_finalization(
                    SlackVideoClipSessionTask { snapshot, ..task },
                    cx,
                );
            }
            VideoClipCaptureStatus::Failed(failure) => {
                self.begin_slack_video_clip_cancellation(
                    SlackVideoClipSessionTask { snapshot, ..task }
                        .cancelling(Some(failure.to_string())),
                    cx,
                );
            }
        }
    }

    fn apply_slack_video_clip_preview_status(
        &mut self,
        task: SlackVideoClipSessionTask,
        snapshot: SlackVideoClipSnapshot,
        cx: &mut Context<Self>,
    ) {
        if task.recording_started_at_unix_millis.is_some() {
            self.begin_slack_video_clip_cancellation(
                SlackVideoClipSessionTask { snapshot, ..task }.cancelling(Some(
                    "Video capture returned to preview after recording began.".to_string(),
                )),
                cx,
            );
            return;
        }
        self.sync_slack_video_clip_modal_snapshot(&task.operation, &snapshot, cx);
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Prepared {
                operation: task.operation.clone(),
                session_id: task.session_id,
                snapshot,
            });
        self.schedule_slack_video_clip_status_poll(task.operation.generation, task.session_id, cx);
    }

    fn apply_slack_video_clip_recording_status(
        &mut self,
        task: SlackVideoClipSessionTask,
        snapshot: SlackVideoClipSnapshot,
        cx: &mut Context<Self>,
    ) {
        let recording_started_at_unix_millis = task
            .recording_started_at_unix_millis
            .expect("a recording video status must retain its start timestamp");
        self.sync_slack_video_clip_modal_snapshot(&task.operation, &snapshot, cx);
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Recording {
                operation: task.operation.clone(),
                session_id: task.session_id,
                snapshot,
                recording_started_at_unix_millis,
            });
        self.schedule_slack_video_clip_status_poll(task.operation.generation, task.session_id, cx);
    }
}
