mod controls;
mod load;

use gpui::{px, Bounds, Context, ListAlignment, ListState, Pixels, SharedString, Window};

use crate::{
    session::VideoSession,
    source::{VideoPlayerConfig, VideoPlayerSource},
};

pub use load::LoadedVideoSource;

const PLAYLIST_ROW_HEIGHT: f32 = 44.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoPlaybackFrameState {
    pub seek_pending: bool,
    pub decoded_frame_ready: bool,
}

pub struct VideoPlayer {
    pub(crate) config: VideoPlayerConfig,
    pub(crate) sources: Vec<VideoPlayerSource>,
    pub(crate) selected_source_id: Option<SharedString>,
    pub(crate) playlist_list_state: ListState,
    pub(crate) session: Option<VideoSession>,
    pub(crate) load_generation: u64,
    pub(crate) loading_source_id: Option<SharedString>,
    pub(crate) target_paused: bool,
    pub(crate) error: Option<SharedString>,
    pub(crate) unsupported_format: Option<SharedString>,
    pub(crate) pending_seek_fraction: Option<f64>,
    pub(crate) pending_seek_seconds: Option<f64>,
    pub(crate) progress_bar_bounds: Option<Bounds<Pixels>>,
}

impl VideoPlayer {
    pub fn new(config: VideoPlayerConfig) -> Self {
        Self {
            config,
            sources: Vec::new(),
            selected_source_id: None,
            playlist_list_state: ListState::new(0, ListAlignment::Top, px(PLAYLIST_ROW_HEIGHT)),
            session: None,
            load_generation: 0,
            loading_source_id: None,
            target_paused: false,
            error: None,
            unsupported_format: None,
            pending_seek_fraction: None,
            pending_seek_seconds: None,
            progress_bar_bounds: None,
        }
    }

    pub fn new_media_only() -> Self {
        Self::new(VideoPlayerConfig {
            show_playlist: false,
            show_controls: false,
            audio_enabled: false,
            muted: true,
            ..VideoPlayerConfig::default()
        })
    }

    pub fn open_source(&mut self, source: VideoPlayerSource, cx: &mut Context<Self>) {
        self.open_sources(vec![source], None, cx);
    }

    pub fn open_sources(
        &mut self,
        sources: Vec<VideoPlayerSource>,
        selected_source_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.open_playlist(sources, selected_source_id, cx);
    }

    pub fn open_playlist(
        &mut self,
        sources: Vec<VideoPlayerSource>,
        selected_source_id: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.sources = sources;
        self.playlist_list_state.reset(self.sources.len());
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = None;
        self.selected_source_id = self
            .selected_source_from_id(selected_source_id)
            .or_else(|| self.sources.first())
            .map(|source| source.id.clone());
        self.pending_seek_seconds = self
            .selected_source()
            .and_then(VideoPlayerSource::playback_start_seconds);
        self.target_paused = false;
        self.load_selected_source(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.retire_current_session();
        self.sources.clear();
        self.selected_source_id = None;
        self.playlist_list_state.reset(0);
        self.load_generation = self.load_generation.wrapping_add(1);
        self.loading_source_id = None;
        self.target_paused = false;
        self.error = None;
        self.unsupported_format = None;
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = None;
        cx.notify();
    }

    pub fn selected_source_id(&self) -> Option<&str> {
        self.selected_source_id.as_ref().map(SharedString::as_ref)
    }

    pub fn has_sources(&self) -> bool {
        !self.sources.is_empty()
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let changed = session.sync();
        if let Some(error) = session.error_message().filter(|_| !self.config.muted) {
            self.error = Some(error.into());
        }
        let should_poll_for_frames = session.should_poll_for_frames();
        if should_poll_for_frames {
            window.request_animation_frame();
        }
        if changed || should_poll_for_frames {
            cx.notify();
        }
        if !self.target_paused && self.selected_source_reached_end() {
            self.advance_playlist_or_stop(cx);
        }
    }

    pub(crate) fn selected_source(&self) -> Option<&VideoPlayerSource> {
        let selected_id = self.selected_source_id.as_ref()?;
        self.sources
            .iter()
            .find(|source| source.id.as_ref() == selected_id.as_ref())
    }

    pub(crate) fn select_source(&mut self, source_id: SharedString, cx: &mut Context<Self>) {
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = None;
        if self.selected_source_id.as_ref() == Some(&source_id) {
            let restart_seconds = self
                .selected_source()
                .and_then(VideoPlayerSource::playback_start_seconds)
                .unwrap_or(0.0);
            if let Some(session) = self.session.as_mut() {
                if session.is_finished() {
                    session.seek_to_seconds(restart_seconds);
                }
                session.play();
                self.target_paused = false;
            } else if self.loading_source_id.as_ref() != Some(&source_id) {
                self.load_selected_source(cx);
                return;
            }
            cx.notify();
            return;
        }
        self.selected_source_id = Some(source_id);
        self.pending_seek_seconds = self
            .selected_source()
            .and_then(VideoPlayerSource::playback_start_seconds);
        self.target_paused = false;
        self.load_selected_source(cx);
    }

    fn retire_current_session(&mut self) {
        if let Some(session) = self.session.take() {
            session.shutdown_in_background();
        }
    }

    fn selected_source_reached_end(&self) -> bool {
        let Some(session) = self.session.as_ref() else {
            return false;
        };
        session.is_finished()
            || self
                .selected_source()
                .and_then(VideoPlayerSource::playback_end_seconds)
                .is_some_and(|end_seconds| session.current_time_seconds() >= end_seconds)
    }

    fn advance_playlist_or_stop(&mut self, cx: &mut Context<Self>) {
        if self.advance_playlist(cx) {
            return;
        }
        self.target_paused = true;
        if let Some(session) = self.session.as_mut() {
            session.pause();
        }
        cx.notify();
    }

    fn advance_playlist(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(selected_id) = self.selected_source_id.as_ref() else {
            return false;
        };
        let Some(index) = self
            .sources
            .iter()
            .position(|source| source.id.as_ref() == selected_id.as_ref())
        else {
            return false;
        };
        let Some(next) = self.sources.get(index + 1) else {
            return false;
        };
        let next_id = next.id.clone();
        let next_start_seconds = next.playback_start_seconds();
        self.selected_source_id = Some(next_id);
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = next_start_seconds;
        self.target_paused = false;
        self.load_selected_source(cx);
        true
    }

    fn restart_playlist(&mut self, cx: &mut Context<Self>) {
        let Some(first) = self.sources.first() else {
            return;
        };
        let first_id = first.id.clone();
        let first_start_seconds = first.playback_start_seconds().unwrap_or(0.0);
        self.selected_source_id = Some(first_id);
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = Some(first_start_seconds);
        self.target_paused = false;
        self.load_selected_source(cx);
    }

    fn clamp_seek_seconds(&self, source_id: &SharedString, seconds: f64) -> f64 {
        let Some(source) = self
            .sources
            .iter()
            .find(|source| source.id.as_ref() == source_id.as_ref())
        else {
            return seconds;
        };
        let seconds = source
            .playback_start_seconds()
            .map_or(seconds, |start| seconds.max(start));
        source
            .playback_end_seconds()
            .map_or(seconds, |end| seconds.min(end))
    }

    fn selected_source_from_id(
        &self,
        selected_source_id: Option<&str>,
    ) -> Option<&VideoPlayerSource> {
        selected_source_id
            .and_then(|id| self.sources.iter().find(|source| source.id.as_ref() == id))
    }
}

pub(crate) const PLAYLIST_ROW_HEIGHT_PX: f32 = PLAYLIST_ROW_HEIGHT;
