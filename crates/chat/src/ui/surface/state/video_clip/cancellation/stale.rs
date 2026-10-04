use super::{
    Arc, Context, Duration, MediaCaptureApi, SlackComposerCaptureOperation,
    SlackVideoClipCaptureState, SlackVideoClipModalPhase, SlackVideoClipModalTransition,
    SurfaceState, VideoClipSessionId,
};

impl SurfaceState {
    pub(in super::super) fn spawn_stale_slack_video_clip_cancellation(
        &mut self,
        api: Arc<MediaCaptureApi>,
        operation: SlackComposerCaptureOperation,
        session_id: VideoClipSessionId,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (api, operation, session_id),
            cx,
            |(api, operation, session_id)| (operation, api.cancel_video_clip(session_id)),
            |this, (operation, result), cx| {
                let Err(error) = result else {
                    return;
                };
                let diagnostic = format!("Failed to clean up a stale video clip session: {error}");
                this.slack_error = Some(diagnostic.clone());
                if this
                    .slack_composer_capture
                    .operation()
                    .is_some_and(|current| current == &operation)
                {
                    this.set_slack_video_clip_modal_phase(
                        &operation,
                        SlackVideoClipModalTransition::new(
                            SlackVideoClipModalPhase::Failed,
                            Duration::ZERO,
                        )
                        .with_diagnostic(Some(diagnostic.clone())),
                        cx,
                    );
                    this.slack_composer_capture
                        .set_video(SlackVideoClipCaptureState::Failed {
                            operation,
                            session_id: None,
                            duration: Duration::ZERO,
                            recording_started_at_unix_millis: None,
                            diagnostic,
                        });
                }
                cx.notify();
            },
        );
    }
}
