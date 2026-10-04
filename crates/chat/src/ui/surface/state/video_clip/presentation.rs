use super::{
    Context, SlackComposerCaptureGeneration, SlackComposerCaptureOperation,
    SlackVideoClipCaptureState, SlackVideoClipModal, SlackVideoClipModalTransition,
    SlackVideoClipReviewArtifact, SlackVideoClipSnapshot, SurfaceState, VideoClipCaptureStatus,
    VideoFrameFit, VideoPlayer, VideoPlayerConfig, VideoPlayerSource, GENERIC_VIDEO_CLIP_FILENAME,
};
use crate::ui::AppContext;

impl SurfaceState {
    pub(super) fn open_slack_video_clip_modal(
        &mut self,
        operation: &SlackComposerCaptureOperation,
        cx: &mut Context<Self>,
    ) {
        let surface = cx.entity().downgrade();
        let generation = operation.generation;
        self.slack_video_clip_modal =
            Some(cx.new(move |cx| SlackVideoClipModal::new(surface, generation, cx)));
        cx.notify();
    }

    pub(super) fn close_slack_video_clip_modal(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_video_clip_modal
            .as_ref()
            .is_none_or(|modal| modal.read(cx).generation() != generation)
        {
            return;
        }
        if let Some(modal) = self.slack_video_clip_modal.as_ref() {
            modal.update(cx, |modal, cx| modal.retire_review_player(cx));
        }
        self.slack_video_clip_modal = None;
        cx.notify();
    }

    pub(super) fn sync_slack_video_clip_modal_snapshot(
        &self,
        operation: &SlackComposerCaptureOperation,
        snapshot: &SlackVideoClipSnapshot,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_video_clip_modal.as_ref() else {
            return;
        };
        if modal.read(cx).generation() != operation.generation {
            return;
        }
        modal.update(cx, |modal, cx| {
            modal.apply_capture_snapshot(
                &snapshot.status,
                snapshot.duration,
                snapshot.latest_preview.as_ref(),
                cx,
            );
        });
    }

    pub(super) fn set_slack_video_clip_modal_phase(
        &self,
        operation: &SlackComposerCaptureOperation,
        transition: SlackVideoClipModalTransition,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_video_clip_modal.as_ref() else {
            return;
        };
        if modal.read(cx).generation() != operation.generation {
            return;
        }
        modal.update(cx, |modal, cx| {
            modal.set_phase(
                transition.phase,
                transition.duration,
                transition.diagnostic,
                cx,
            );
        });
    }

    pub(super) fn begin_slack_video_clip_review(
        &self,
        operation: &SlackComposerCaptureOperation,
        artifact: &SlackVideoClipReviewArtifact,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_video_clip_modal.as_ref() else {
            return;
        };
        if modal.read(cx).generation() != operation.generation {
            return;
        }
        let clip = artifact.clip();
        let player = cx.new(|_| {
            VideoPlayer::new(VideoPlayerConfig {
                show_playlist: false,
                show_controls: true,
                muted: false,
                audio_enabled: true,
                looping: false,
                frame_fit: VideoFrameFit::Contain,
            })
        });
        let source = VideoPlayerSource::new(
            format!("video-clip-review-{}", operation.generation.get()),
            GENERIC_VIDEO_CLIP_FILENAME,
            "Slack",
            clip.file().path().to_string_lossy().into_owned(),
            None,
        );
        player.update(cx, |player, cx| {
            player.open_source(source, cx);
            player.pause(cx);
        });
        modal.update(cx, |modal, cx| {
            modal.begin_review(player, clip.duration(), cx);
        });
    }

    pub(crate) fn slack_video_clip_is_ready_to_record(
        &self,
        generation: SlackComposerCaptureGeneration,
    ) -> bool {
        matches!(
            self.slack_composer_capture.video(),
            Some(SlackVideoClipCaptureState::Prepared {
                operation,
                snapshot,
                ..
            }) if operation.generation == generation
                && self.slack_composer_capture_operation_is_current(operation)
                && matches!(snapshot.status, VideoClipCaptureStatus::Previewing)
        )
    }

    pub(super) fn slack_video_clip_modal_countdown_seconds(
        &self,
        operation: &SlackComposerCaptureOperation,
        cx: &Context<Self>,
    ) -> Option<u8> {
        self.slack_video_clip_modal
            .as_ref()
            .filter(|modal| modal.read(cx).generation() == operation.generation)
            .and_then(|modal| modal.read(cx).countdown_seconds_remaining())
    }
}
