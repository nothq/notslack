use super::{
    ChatVideoClipCaptureState, Context, SlackComposerCaptureGeneration,
    SlackComposerCaptureOperation, SlackComposerFileId, SlackPreparedUploadFile,
    SlackVideoClipCaptureState, SlackVideoClipModalPhase, SlackVideoClipModalTransition,
    SlackVideoClipReviewArtifact, SurfaceState, VideoClipSessionId, GENERIC_VIDEO_CLIP_FILENAME,
};

struct SlackVideoClipReviewSession {
    operation: SlackComposerCaptureOperation,
    session_id: VideoClipSessionId,
    recording_started_at_unix_millis: i64,
    artifact: SlackVideoClipReviewArtifact,
}

impl SurfaceState {
    pub(crate) fn attach_reviewed_slack_video_clip(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let generation = self
            .slack_composer_capture
            .video()
            .map(|state| state.operation().generation)
            .ok_or_else(|| "There is no reviewed Slack video clip to attach.".to_string())?;
        self.attach_reviewed_slack_video_clip_for_generation(generation, cx)
    }

    pub(crate) fn attach_reviewed_slack_video_clip_for_generation(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let Some(review) = self.reviewed_slack_video_clip_session(generation)? else {
            return Ok(self.control_slack_video_clip_capture_state(cx));
        };
        if !self.active || !self.slack_composer_capture_operation_is_current(&review.operation) {
            self.restore_slack_video_clip_review(review, cx);
            return Err(
                "The Slack composer draft changed before the clip was attached.".to_string(),
            );
        }
        let local_file_api = self.local_file_api.clone().ok_or_else(|| {
            "Slack local file access is unavailable for this surface.".to_string()
        })?;
        let prepared =
            match prepare_reviewed_slack_video_clip_upload(local_file_api.as_ref(), &review) {
                Ok(prepared) => prepared,
                Err(diagnostic) => {
                    self.restore_slack_video_clip_review(review, cx);
                    return Err(diagnostic);
                }
            };
        let duration = review.artifact.clip().duration();
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Attaching {
                operation: review.operation.clone(),
                session_id: review.session_id,
                duration,
                recording_started_at_unix_millis: review.recording_started_at_unix_millis,
            });
        self.set_slack_video_clip_modal_phase(
            &review.operation,
            SlackVideoClipModalTransition::new(SlackVideoClipModalPhase::Attaching, duration),
            cx,
        );
        match self.attach_slack_prepared_upload_files_to_main_draft_with_selection(
            &review.operation.owner,
            vec![prepared],
            false,
            cx,
        ) {
            Ok(attached) => {
                Ok(self
                    .complete_reviewed_slack_video_clip_attachment(review, duration, attached, cx))
            }
            Err(diagnostic) => {
                self.restore_slack_video_clip_review(review, cx);
                Err(diagnostic)
            }
        }
    }

    fn reviewed_slack_video_clip_session(
        &self,
        generation: SlackComposerCaptureGeneration,
    ) -> Result<Option<SlackVideoClipReviewSession>, String> {
        match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Reviewing {
                operation,
                session_id,
                recording_started_at_unix_millis,
                artifact,
            }) if operation.generation == generation => Ok(Some(SlackVideoClipReviewSession {
                operation: operation.clone(),
                session_id: *session_id,
                recording_started_at_unix_millis: *recording_started_at_unix_millis,
                artifact: artifact.clone(),
            })),
            Some(
                SlackVideoClipCaptureState::Attaching { operation, .. }
                | SlackVideoClipCaptureState::Attached { operation, .. },
            ) if operation.generation == generation => Ok(None),
            _ => Err("There is no reviewed Slack video clip to attach.".to_string()),
        }
    }

    fn complete_reviewed_slack_video_clip_attachment(
        &mut self,
        review: SlackVideoClipReviewSession,
        duration: std::time::Duration,
        attached: crate::model::ChatComposerFilesAttached,
        cx: &mut Context<Self>,
    ) -> ChatVideoClipCaptureState {
        let file = attached
            .files
            .into_iter()
            .next()
            .expect("attaching one video clip must return one composer file");
        let file_id = SlackComposerFileId::parse(&file.file_id)
            .expect("Chat-generated composer file ids must parse");
        let generation = review.operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Attached {
                operation: review.operation,
                duration,
                recording_started_at_unix_millis: review.recording_started_at_unix_millis,
                file_id,
            });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
        self.control_slack_video_clip_capture_state(cx)
    }

    fn restore_slack_video_clip_review(
        &mut self,
        review: SlackVideoClipReviewSession,
        cx: &mut Context<Self>,
    ) {
        let modal_is_current = self
            .slack_video_clip_modal
            .as_ref()
            .is_some_and(|modal| modal.read(cx).generation() == review.operation.generation);
        if !modal_is_current {
            self.open_slack_video_clip_modal(&review.operation, cx);
        }
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Reviewing {
                operation: review.operation.clone(),
                session_id: review.session_id,
                recording_started_at_unix_millis: review.recording_started_at_unix_millis,
                artifact: review.artifact.clone(),
            });
        self.begin_slack_video_clip_review(&review.operation, &review.artifact, cx);
        cx.notify();
    }

    pub(crate) fn restart_slack_video_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let generation = self
            .slack_composer_capture
            .video()
            .map(|state| state.operation().generation)
            .ok_or_else(|| "There is no reviewed Slack video clip to restart.".to_string())?;
        self.restart_slack_video_clip_capture_for_generation(generation, cx)
    }

    pub(crate) fn restart_slack_video_clip_capture_for_generation(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let review = match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Reviewing {
                operation,
                session_id,
                recording_started_at_unix_millis,
                artifact,
            }) if operation.generation == generation => SlackVideoClipReviewSession {
                operation: operation.clone(),
                session_id: *session_id,
                recording_started_at_unix_millis: *recording_started_at_unix_millis,
                artifact: artifact.clone(),
            },
            _ => {
                return Err("There is no reviewed Slack video clip to restart.".to_string());
            }
        };
        if !self.slack_video_clip_prepare_environment_available()
            || !self.slack_composer_capture_operation_is_current(&review.operation)
        {
            self.restore_slack_video_clip_review(review, cx);
            return Err("Video clip recording is unavailable for the active composer.".to_string());
        }
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Cancelled {
                operation: review.operation.clone(),
                duration: review.artifact.clip().duration(),
                recording_started_at_unix_millis: Some(review.recording_started_at_unix_millis),
            });
        self.close_slack_video_clip_modal(review.operation.generation, cx);
        match self.prepare_slack_video_clip_capture(cx) {
            Ok(capture) => Ok(capture),
            Err(diagnostic) => {
                self.restore_slack_video_clip_review(review, cx);
                if let Some(modal) = self.slack_video_clip_modal.clone() {
                    modal.update(cx, |modal, cx| {
                        modal.set_interaction_error(diagnostic.clone(), cx);
                    });
                }
                Err(diagnostic)
            }
        }
    }
}

fn prepare_reviewed_slack_video_clip_upload(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    review: &SlackVideoClipReviewSession,
) -> Result<SlackPreparedUploadFile, String> {
    local_file_api
        .open_named_upload(
            review.artifact.clip().file().path().to_path_buf(),
            GENERIC_VIDEO_CLIP_FILENAME.to_string(),
            review.artifact.clip().mimetype().to_string(),
        )
        .map(SlackPreparedUploadFile::from_upload)
}
