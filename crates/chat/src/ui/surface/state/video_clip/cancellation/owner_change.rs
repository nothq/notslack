use super::{Context, SlackVideoClipCaptureState, SurfaceState};

impl SurfaceState {
    pub(crate) fn cancel_slack_video_clip_for_owner_change(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.slack_composer_capture.video().cloned() else {
            return;
        };
        let recording_started_at_unix_millis = state.recording_started_at_unix_millis();
        self.cancel_slack_video_clip_owner_state(state, recording_started_at_unix_millis, cx);
    }

    fn cancel_slack_video_clip_owner_state(
        &mut self,
        state: SlackVideoClipCaptureState,
        recording_started_at_unix_millis: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        let generation = state.operation().generation;
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
            state @ SlackVideoClipCaptureState::Reviewing { .. } => {
                self.cancel_finished_slack_video_clip_state(state, cx);
            }
            SlackVideoClipCaptureState::Finalizing { .. }
            | SlackVideoClipCaptureState::Failed { .. }
            | SlackVideoClipCaptureState::Cancelled { .. }
            | SlackVideoClipCaptureState::Attached { .. } => {
                self.close_slack_video_clip_modal(generation, cx);
            }
            SlackVideoClipCaptureState::CancellingPreparation { .. }
            | SlackVideoClipCaptureState::CancellingStart { .. }
            | SlackVideoClipCaptureState::Cancelling { .. }
            | SlackVideoClipCaptureState::Attaching { .. } => {}
        }
    }
}
