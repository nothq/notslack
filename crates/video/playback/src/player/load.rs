use gpui::{Context, SharedString};
use gpui_components::spawn_background_task_for_entity;

use crate::{
    session::{VideoSession, VideoSessionOptions},
    source::{
        prepare_player_source, prepare_video_source_for_playback, video_unsupported_label,
        PreparedVideoSource, VideoPlayerSource,
    },
};

use super::VideoPlayer;

pub struct LoadedVideoSource {
    pub(super) source: VideoPlayerSource,
    pub(super) session: VideoSession,
}

type VideoSourceLoadResult = (u64, SharedString, Result<PreparedVideoSource, String>);

impl VideoPlayer {
    pub fn load_source(source: VideoPlayerSource) -> Result<LoadedVideoSource, String> {
        Self::load_source_with_options(source, VideoSessionOptions::default())
    }

    fn load_source_with_options(
        source: VideoPlayerSource,
        options: VideoSessionOptions,
    ) -> Result<LoadedVideoSource, String> {
        if let Some(label) = video_unsupported_label(source.url.as_str()) {
            return Err(format!("{label} is not a video log source."));
        }
        let prepared_source = prepare_player_source(&source)?;
        let session = VideoSession::from_prepared_source_with_options(prepared_source, options)?;
        Ok(LoadedVideoSource { source, session })
    }

    pub fn open_loaded_source(&mut self, mut loaded: LoadedVideoSource, cx: &mut Context<Self>) {
        self.retire_current_session();
        self.load_generation = self.load_generation.wrapping_add(1);
        self.loading_source_id = None;
        self.target_paused = false;
        self.sources = vec![loaded.source];
        if let Some(start_seconds) = self.sources[0].playback_start_seconds() {
            loaded.session.seek_to_seconds(start_seconds);
        }
        self.playlist_list_state.reset(self.sources.len());
        self.selected_source_id = self.sources.first().map(|source| source.id.clone());
        self.session = Some(loaded.session);
        self.error = None;
        self.unsupported_format = None;
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = None;
        cx.notify();
    }

    pub(super) fn load_selected_source(&mut self, cx: &mut Context<Self>) {
        self.retire_current_session();
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        self.error = None;
        self.unsupported_format = None;
        self.loading_source_id = None;

        let Some(source) = self.selected_source().cloned() else {
            cx.notify();
            return;
        };
        if let Some(label) = video_unsupported_label(source.url.as_str()) {
            self.unsupported_format = Some(label.into());
            cx.notify();
            return;
        }
        self.loading_source_id = Some(source.id.clone());
        spawn_background_task_for_entity(
            (generation, source),
            cx,
            |(generation, source)| {
                let source_id = source.id.clone();
                let result = prepare_video_source_for_playback(&source);
                (generation, source_id, result)
            },
            Self::apply_prepared_source,
        );
        cx.notify();
    }

    fn apply_prepared_source(
        this: &mut Self,
        (generation, source_id, result): VideoSourceLoadResult,
        cx: &mut Context<Self>,
    ) {
        if generation != this.load_generation
            || this.selected_source_id.as_ref() != Some(&source_id)
            || this.loading_source_id.as_ref() != Some(&source_id)
        {
            cx.notify();
            return;
        }
        this.loading_source_id = None;
        match result {
            Ok(prepared_source) => this.open_prepared_source(prepared_source),
            Err(error) => this.record_source_error(error),
        }
        cx.notify();
    }

    fn open_prepared_source(&mut self, prepared_source: PreparedVideoSource) {
        self.retire_current_session();
        match VideoSession::from_prepared_source_with_options(
            prepared_source,
            self.session_options(),
        ) {
            Ok(mut session) => {
                self.apply_pending_playback_state(&mut session);
                self.session = Some(session);
                self.error = None;
                self.unsupported_format = None;
            }
            Err(error) => self.record_source_error(error),
        }
    }

    fn apply_pending_playback_state(&mut self, session: &mut VideoSession) {
        if self.target_paused {
            session.pause();
        }
        if let Some(seconds) = self.pending_seek_seconds.take() {
            self.pending_seek_fraction = None;
            session.seek_to_seconds(seconds);
        } else if let Some(fraction) = self.pending_seek_fraction.take() {
            session.seek_to_fraction(fraction);
        }
        if !self.target_paused {
            session.play();
        }
    }

    fn record_source_error(&mut self, error: String) {
        self.session = None;
        self.error = Some(error.into());
        self.unsupported_format = None;
        self.pending_seek_fraction = None;
        self.pending_seek_seconds = None;
    }

    fn session_options(&self) -> VideoSessionOptions {
        VideoSessionOptions {
            muted: self.config.muted,
            looping: self.config.looping,
            audio_enabled: self.config.audio_enabled,
        }
    }
}
