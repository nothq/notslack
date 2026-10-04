use super::{Context, SurfaceState};
use crate::ui::surface::SlackComposerCaptureState;

impl SurfaceState {
    pub(crate) fn cancel_slack_composer_capture_for_owner_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        match self.slack_composer_capture.state() {
            SlackComposerCaptureState::Audio(_) => {
                self.cancel_slack_audio_clip_for_owner_change(cx);
            }
            SlackComposerCaptureState::Video(_) => {
                self.cancel_slack_video_clip_for_owner_change(cx);
            }
            SlackComposerCaptureState::Idle => {}
        }
    }

    pub(crate) fn slack_composer_capture_blocks_current_draft(&self) -> bool {
        self.slack_composer_capture.blocks_draft_submission()
            && self
                .slack_composer_capture
                .operation()
                .is_some_and(|operation| {
                    self.current_slack_main_composer_draft_handle().as_ref()
                        == Some(&operation.owner)
                })
    }
}
