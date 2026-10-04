use crate::model::ChatVideoClipCaptureState;

use super::{Context, SurfaceRoot};

impl SurfaceRoot {
    pub fn prepare_slack_video_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.prepare_slack_video_clip_capture(cx)
        })
    }

    pub fn begin_slack_video_clip_recording<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.begin_slack_video_clip_recording(cx)
        })
    }

    pub fn stop_slack_video_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.stop_slack_video_clip_capture(cx))
    }

    pub fn attach_slack_video_clip<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_reviewed_slack_video_clip(cx)
        })
    }

    pub fn restart_slack_video_clip<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.restart_slack_video_clip_capture(cx)
        })
    }

    pub fn cancel_slack_video_clip_capture<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatVideoClipCaptureState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.cancel_slack_video_clip_capture(cx)
        })
    }

    pub fn slack_video_clip_capture_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> ChatVideoClipCaptureState {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_slack_video_clip_capture_state(cx)
        })
    }
}
