use gpui::{Bounds, Context, Pixels, Point, SharedString};

use crate::session::VideoSession;

use super::{VideoPlaybackFrameState, VideoPlayer};

impl VideoPlayer {
    pub fn select_source_and_seek(
        &mut self,
        source_id: SharedString,
        fraction: f64,
        cx: &mut Context<Self>,
    ) {
        let fraction = fraction.clamp(0.0, 1.0);
        self.pending_seek_fraction = Some(fraction);
        self.pending_seek_seconds = None;
        if self.selected_source_id.as_ref() == Some(&source_id) {
            if let Some(session) = self.session.as_mut() {
                session.seek_to_fraction(fraction);
                self.pending_seek_fraction = None;
            } else if self.loading_source_id.as_ref() != Some(&source_id) {
                self.load_selected_source(cx);
                return;
            }
            cx.notify();
            return;
        }
        self.selected_source_id = Some(source_id);
        self.load_selected_source(cx);
    }

    pub fn select_source_and_seek_seconds(
        &mut self,
        source_id: SharedString,
        seconds: f64,
        cx: &mut Context<Self>,
    ) {
        let seconds = self.clamp_seek_seconds(&source_id, finite_seek_seconds(seconds));
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = Some(seconds);
        if self.selected_source_id.as_ref() == Some(&source_id) {
            if let Some(session) = self.session.as_mut() {
                session.seek_to_seconds(seconds);
                self.pending_seek_seconds = None;
            } else if self.loading_source_id.as_ref() != Some(&source_id) {
                self.load_selected_source(cx);
                return;
            }
            cx.notify();
            return;
        }
        self.selected_source_id = Some(source_id);
        self.load_selected_source(cx);
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.selected_source_reached_end() {
            if !self.advance_playlist(cx) {
                self.restart_playlist(cx);
            }
            return;
        }
        let Some(session) = self.session.as_mut() else {
            self.target_paused = !self.target_paused;
            cx.notify();
            return;
        };
        self.target_paused = !session.toggle_playback();
        cx.notify();
    }

    pub fn play(&mut self, cx: &mut Context<Self>) {
        if self.selected_source_reached_end() {
            if !self.advance_playlist(cx) {
                self.restart_playlist(cx);
            }
            return;
        }
        self.target_paused = false;
        if let Some(session) = self.session.as_mut() {
            session.play();
        }
        cx.notify();
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.target_paused = true;
        if let Some(session) = self.session.as_mut() {
            session.pause();
        }
        cx.notify();
    }

    pub fn seek_to_fraction(&mut self, fraction: f64, cx: &mut Context<Self>) {
        let fraction = fraction.clamp(0.0, 1.0);
        self.pending_seek_seconds = None;
        if let Some(session) = self.session.as_mut() {
            session.seek_to_fraction(fraction);
            self.pending_seek_fraction = None;
        } else {
            self.pending_seek_fraction = Some(fraction);
        }
        cx.notify();
    }

    pub fn seek_to_seconds(&mut self, seconds: f64, cx: &mut Context<Self>) {
        let seconds = self
            .selected_source_id
            .clone()
            .map(|source_id| self.clamp_seek_seconds(&source_id, finite_seek_seconds(seconds)))
            .unwrap_or_else(|| finite_seek_seconds(seconds));
        self.pending_seek_fraction = None;
        if let Some(session) = self.session.as_mut() {
            session.seek_to_seconds(seconds);
            self.pending_seek_seconds = None;
        } else {
            self.pending_seek_seconds = Some(seconds);
        }
        cx.notify();
    }

    pub fn playback_current_seconds(&self) -> f64 {
        self.session
            .as_ref()
            .map(VideoSession::current_time_seconds)
            .or(self.pending_seek_seconds)
            .unwrap_or(0.0)
    }

    pub fn playback_duration_seconds(&self) -> Option<f64> {
        self.session
            .as_ref()
            .and_then(VideoSession::duration_seconds)
    }

    pub fn playback_progress_fraction(&self) -> f32 {
        self.session
            .as_ref()
            .map(VideoSession::progress_fraction)
            .or_else(|| self.pending_seek_fraction.map(|fraction| fraction as f32))
            .unwrap_or(0.0)
    }

    pub fn playback_is_playing(&self) -> bool {
        self.session.as_ref().is_some_and(VideoSession::is_playing)
    }

    pub fn playback_is_loading(&self) -> bool {
        self.loading_source_id.is_some()
    }

    pub fn playback_frame_state(&self) -> VideoPlaybackFrameState {
        let (session_seek_pending, has_decoded_frame) = self
            .session
            .as_ref()
            .map(|session| (session.seek_pending(), session.has_decoded_frame()))
            .unwrap_or_default();
        let seek_pending = self.pending_seek_fraction.is_some()
            || self.pending_seek_seconds.is_some()
            || session_seek_pending;
        VideoPlaybackFrameState {
            seek_pending,
            decoded_frame_ready: self.loading_source_id.is_none()
                && !seek_pending
                && has_decoded_frame,
        }
    }

    pub fn playback_is_muted(&self) -> bool {
        self.session.as_ref().is_some_and(VideoSession::muted)
    }

    pub fn playback_volume(&self) -> f32 {
        self.session
            .as_ref()
            .map(VideoSession::volume)
            .unwrap_or(1.0)
    }

    pub fn playback_error_message(&self) -> Option<&str> {
        self.error.as_deref().or(self.unsupported_format.as_deref())
    }

    pub(crate) fn set_progress_bar_bounds(
        &mut self,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.progress_bar_bounds == Some(bounds) {
            return false;
        }
        self.progress_bar_bounds = Some(bounds);
        cx.notify();
        true
    }

    pub(crate) fn seek_to_progress_position(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.seek_progress_at(position, cx);
    }

    pub(crate) fn toggle_muted(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.toggle_muted();
        cx.notify();
    }

    pub fn set_muted(&mut self, muted: bool, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.muted() != muted {
            session.toggle_muted();
            cx.notify();
        }
    }

    pub fn set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.set_volume(volume);
        cx.notify();
    }

    fn seek_progress_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.progress_bar_bounds else {
            return;
        };
        let width = bounds.size.width.as_f32();
        if width <= 0.0 {
            return;
        }
        let x = (position.x - bounds.origin.x).as_f32().clamp(0.0, width);
        self.seek_to_fraction((x / width) as f64, cx);
    }
}

fn finite_seek_seconds(seconds: f64) -> f64 {
    if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    }
}
