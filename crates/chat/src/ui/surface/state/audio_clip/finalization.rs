use super::{
    CapturedAudioClip, ChatAudioClipCaptureState, Context, SlackAudioClipCaptureState,
    SlackAudioClipSessionTask, SlackComposerCaptureOperation, SlackPreparedUploadFile,
    SlackUploadFile, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn stop_slack_audio_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        let task = match self.slack_composer_capture.audio() {
            Some(SlackAudioClipCaptureState::Recording {
                operation,
                session_id,
                started_at,
                ..
            }) => {
                SlackAudioClipSessionTask::new(operation.clone(), *session_id, started_at.elapsed())
            }
            Some(SlackAudioClipCaptureState::Finalizing { .. }) => {
                return Ok(self.control_slack_audio_clip_capture_state());
            }
            _ => return Err("There is no active Slack audio clip recording to stop.".to_string()),
        };
        if !self.slack_audio_clip_operation_is_current(&task.operation) {
            self.begin_slack_audio_clip_cancellation(task.cancelling(None), cx);
            return Err(
                "The Slack composer draft changed before the audio clip could be stopped."
                    .to_string(),
            );
        }
        self.begin_slack_audio_clip_finalization(task, cx);
        Ok(self.control_slack_audio_clip_capture_state())
    }

    pub(super) fn begin_slack_audio_clip_finalization(
        &mut self,
        task: SlackAudioClipSessionTask,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.media_capture_api.clone() else {
            self.slack_composer_capture
                .set_audio(SlackAudioClipCaptureState::Failed {
                    operation: task.operation,
                    duration: task.duration,
                    diagnostic: "Audio clip recording is no longer configured.".to_string(),
                });
            cx.notify();
            return;
        };
        self.slack_composer_capture
            .set_audio(SlackAudioClipCaptureState::Finalizing {
                operation: task.operation.clone(),
                session_id: task.session_id,
                duration: task.duration,
            });
        cx.notify();
        self.spawn_background_task(
            (api, task),
            cx,
            |(api, task)| {
                let result = api.stop_audio_clip(task.session_id);
                (task, result)
            },
            |this, (task, result), cx| {
                this.apply_slack_audio_clip_finalization(task, result, cx);
            },
        );
    }

    fn apply_slack_audio_clip_finalization(
        &mut self,
        task: SlackAudioClipSessionTask,
        result: Result<CapturedAudioClip, crate::ui::MediaCaptureError>,
        cx: &mut Context<Self>,
    ) {
        let finalization_is_current = matches!(
            self.slack_composer_capture.audio(),
            Some(SlackAudioClipCaptureState::Finalizing {
                operation: current,
                session_id: current_session,
                ..
            }) if current == &task.operation && *current_session == task.session_id
        );
        if !finalization_is_current {
            return;
        }
        if !self.active || !self.slack_audio_clip_operation_is_current(&task.operation) {
            self.slack_composer_capture
                .set_audio(SlackAudioClipCaptureState::Cancelled {
                    operation: task.operation,
                    duration: task.duration,
                });
            cx.notify();
            return;
        }
        match result {
            Ok(clip) => self.attach_finalized_slack_audio_clip(task.operation, clip, cx),
            Err(error) => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::Failed {
                        operation: task.operation,
                        duration: task.duration,
                        diagnostic: error.to_string(),
                    });
                cx.notify();
            }
        }
    }

    fn attach_finalized_slack_audio_clip(
        &mut self,
        operation: SlackComposerCaptureOperation,
        clip: CapturedAudioClip,
        cx: &mut Context<Self>,
    ) {
        let handle = &operation.owner;
        let duration = clip.duration;
        let upload =
            match SlackUploadFile::from_bytes(clip.filename, clip.bytes, clip.mimetype.to_string())
            {
                Ok(upload) => upload,
                Err(diagnostic) => {
                    self.slack_composer_capture
                        .set_audio(SlackAudioClipCaptureState::Failed {
                            operation,
                            duration,
                            diagnostic,
                        });
                    cx.notify();
                    return;
                }
            };
        let prepared = SlackPreparedUploadFile::from_upload(upload);
        match self.attach_slack_prepared_upload_files_to_main_draft_with_selection(
            handle,
            vec![prepared],
            false,
            cx,
        ) {
            Ok(_) => {
                self.slack_composer_capture.set_idle();
                cx.notify();
            }
            Err(diagnostic) => {
                self.slack_composer_capture
                    .set_audio(SlackAudioClipCaptureState::Failed {
                        operation,
                        duration,
                        diagnostic,
                    });
                cx.notify();
            }
        }
    }
}
