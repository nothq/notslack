use crate::model::ChatAudioClipCaptureState;

use super::{Context, SurfaceRoot};

impl SurfaceRoot {
    pub fn start_slack_audio_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.start_slack_audio_clip_capture(cx))
    }

    pub fn stop_slack_audio_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.stop_slack_audio_clip_capture(cx))
    }

    pub fn cancel_slack_audio_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatAudioClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.cancel_slack_audio_clip_capture(cx)
        })
    }

    pub fn slack_audio_clip_capture_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> ChatAudioClipCaptureState {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.control_slack_audio_clip_capture_state()
        })
    }
}
