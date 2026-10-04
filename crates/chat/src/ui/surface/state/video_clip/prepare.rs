use super::{
    Arc, ChatVideoClipCaptureState, Context, Duration, MediaCaptureApi,
    SlackComposerCaptureOperation, SlackVideoClipCaptureState, SlackVideoClipModalPhase,
    SlackVideoClipModalTransition, SlackVideoClipPrepareFailure, SlackVideoClipSessionTask,
    SlackVideoClipSnapshot, SurfaceState, VideoClipCaptureStatus, VideoClipSessionId,
    SLACK_COMPOSER_FILE_LIMIT,
};

#[derive(Clone, Copy)]
struct SlackVideoClipPrepareRequestState {
    request_is_current: bool,
    cancellation_is_pending: bool,
    installable: bool,
}

struct SlackVideoClipFailedPrepare {
    api: Arc<MediaCaptureApi>,
    operation: SlackComposerCaptureOperation,
    failure: SlackVideoClipPrepareFailure,
    request_state: SlackVideoClipPrepareRequestState,
}

impl SurfaceState {
    pub(crate) fn can_prepare_slack_video_clip_capture(&self) -> bool {
        self.slack_composer_capture.can_start()
            && self.slack_video_clip_prepare_environment_available()
    }

    pub(super) fn slack_video_clip_prepare_environment_available(&self) -> bool {
        self.media_capture_api
            .as_ref()
            .is_some_and(|api| api.capabilities().video_clip)
            && self.can_mutate_current_slack_send_draft()
            && self.slack_active_scheduled_edit.is_none()
            && self.slack_workspace_api_capabilities.upload_files
            && self.slack_workspace_api_capabilities.stage_file
            && self.slack_composer_files.len() < SLACK_COMPOSER_FILE_LIMIT
    }

    pub(crate) fn prepare_slack_video_clip_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        if !self.can_prepare_slack_video_clip_capture() {
            return Err(
                "Video clip recording is unavailable for the active Slack composer.".to_string(),
            );
        }
        let api = self
            .media_capture_api
            .clone()
            .ok_or_else(|| "Video clip recording is not configured.".to_string())?;
        let owner = self
            .current_slack_main_composer_draft_handle()
            .ok_or_else(|| "There is no active Slack composer draft.".to_string())?;
        let operation = self.slack_composer_capture.next_operation(owner);
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Preparing {
                operation: operation.clone(),
            });
        self.open_slack_video_clip_modal(&operation, cx);
        cx.notify();

        self.spawn_background_task(
            (api, operation),
            cx,
            |(api, operation)| {
                let result = match api.prepare_video_clip() {
                    Ok(session_id) => match api.video_clip_status(session_id) {
                        Ok(snapshot) => Ok((session_id, SlackVideoClipSnapshot::from(snapshot))),
                        Err(error) => Err(SlackVideoClipPrepareFailure {
                            session_id: Some(session_id),
                            diagnostic: error.to_string(),
                        }),
                    },
                    Err(error) => Err(SlackVideoClipPrepareFailure {
                        session_id: None,
                        diagnostic: error.to_string(),
                    }),
                };
                (api, operation, result)
            },
            |this, (api, operation, result), cx| {
                this.apply_slack_video_clip_prepare(api, operation, result, cx);
            },
        );
        Ok(self.control_slack_video_clip_capture_state(cx))
    }

    fn apply_slack_video_clip_prepare(
        &mut self,
        api: Arc<MediaCaptureApi>,
        operation: SlackComposerCaptureOperation,
        result: Result<(VideoClipSessionId, SlackVideoClipSnapshot), SlackVideoClipPrepareFailure>,
        cx: &mut Context<Self>,
    ) {
        let request_is_current = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::Preparing {
                operation: current,
            }) if current == &operation
        );
        let cancellation_is_pending = matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::CancellingPreparation {
                operation: current,
            }) if current == &operation
        );
        let state = SlackVideoClipPrepareRequestState {
            request_is_current,
            cancellation_is_pending,
            installable: request_is_current
                && self.active
                && self.slack_composer_capture_operation_is_current(&operation),
        };
        match result {
            Ok((session_id, snapshot)) => self.apply_successful_slack_video_clip_prepare(
                api,
                SlackVideoClipSessionTask {
                    operation,
                    session_id,
                    snapshot,
                    recording_started_at_unix_millis: None,
                },
                state,
                cx,
            ),
            Err(failure) => {
                self.apply_failed_slack_video_clip_prepare(
                    SlackVideoClipFailedPrepare {
                        api,
                        operation,
                        failure,
                        request_state: state,
                    },
                    cx,
                );
            }
        }
    }

    fn apply_successful_slack_video_clip_prepare(
        &mut self,
        api: Arc<MediaCaptureApi>,
        task: SlackVideoClipSessionTask,
        state: SlackVideoClipPrepareRequestState,
        cx: &mut Context<Self>,
    ) {
        if state.installable {
            self.sync_slack_video_clip_modal_snapshot(&task.operation, &task.snapshot, cx);
            self.slack_composer_capture
                .set_video(SlackVideoClipCaptureState::Prepared {
                    operation: task.operation.clone(),
                    session_id: task.session_id,
                    snapshot: task.snapshot,
                });
            self.schedule_slack_video_clip_status_poll(
                task.operation.generation,
                task.session_id,
                cx,
            );
            cx.notify();
        } else if state.cancellation_is_pending || state.request_is_current {
            self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
        } else {
            self.spawn_stale_slack_video_clip_cancellation(
                api,
                task.operation,
                task.session_id,
                cx,
            );
        }
    }

    fn apply_failed_slack_video_clip_prepare(
        &mut self,
        failed: SlackVideoClipFailedPrepare,
        cx: &mut Context<Self>,
    ) {
        let SlackVideoClipFailedPrepare {
            api,
            operation,
            failure,
            request_state: state,
        } = failed;
        let SlackVideoClipPrepareFailure {
            session_id,
            diagnostic,
        } = failure;
        if state.request_is_current {
            if let Some(session_id) = session_id {
                let task = failed_slack_video_clip_prepare_task(operation, session_id);
                self.begin_slack_video_clip_cancellation(task.cancelling(Some(diagnostic)), cx);
            } else {
                self.fail_slack_video_clip_prepare(operation, diagnostic, cx);
            }
        } else if state.cancellation_is_pending {
            if let Some(session_id) = session_id {
                let task = failed_slack_video_clip_prepare_task(operation, session_id);
                self.begin_slack_video_clip_cancellation(task.cancelling(None), cx);
            } else {
                self.complete_cancelled_slack_video_clip_prepare(operation, cx);
            }
        } else if let Some(session_id) = session_id {
            self.spawn_stale_slack_video_clip_cancellation(api, operation, session_id, cx);
        }
    }

    fn fail_slack_video_clip_prepare(
        &mut self,
        operation: SlackComposerCaptureOperation,
        diagnostic: String,
        cx: &mut Context<Self>,
    ) {
        self.set_slack_video_clip_modal_phase(
            &operation,
            SlackVideoClipModalTransition::new(SlackVideoClipModalPhase::Failed, Duration::ZERO)
                .with_diagnostic(Some(diagnostic.clone())),
            cx,
        );
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Failed {
                operation,
                session_id: None,
                duration: Duration::ZERO,
                recording_started_at_unix_millis: None,
                diagnostic,
            });
        cx.notify();
    }

    fn complete_cancelled_slack_video_clip_prepare(
        &mut self,
        operation: SlackComposerCaptureOperation,
        cx: &mut Context<Self>,
    ) {
        let generation = operation.generation;
        self.slack_composer_capture
            .set_video(SlackVideoClipCaptureState::Cancelled {
                operation,
                duration: Duration::ZERO,
                recording_started_at_unix_millis: None,
            });
        self.close_slack_video_clip_modal(generation, cx);
        cx.notify();
    }
}

fn failed_slack_video_clip_prepare_task(
    operation: SlackComposerCaptureOperation,
    session_id: VideoClipSessionId,
) -> SlackVideoClipSessionTask {
    SlackVideoClipSessionTask {
        operation,
        session_id,
        snapshot: SlackVideoClipSnapshot {
            status: VideoClipCaptureStatus::Ready,
            duration: Duration::ZERO,
            latest_preview: None,
        },
        recording_started_at_unix_millis: None,
    }
}
