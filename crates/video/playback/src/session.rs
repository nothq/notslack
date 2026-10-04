use std::time::Duration;

use crate::{
    ffmpeg_player::{Video as NativeVideo, VideoOptions as NativeVideoOptions},
    source::PreparedVideoSource,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoSessionOptions {
    pub(crate) muted: bool,
    pub(crate) looping: bool,
    pub(crate) audio_enabled: bool,
}

impl Default for VideoSessionOptions {
    fn default() -> Self {
        Self {
            muted: false,
            looping: false,
            audio_enabled: true,
        }
    }
}

pub(crate) struct VideoSession {
    video: NativeVideo,
    _prepared_source: PreparedVideoSource,
    latest_error: Option<String>,
    current_position: Duration,
    volume: f32,
    muted: bool,
    paused: bool,
    has_seen_frame: bool,
}

impl VideoSession {
    #[cfg(test)]
    pub(crate) fn new(source: &str) -> Result<Self, String> {
        Self::new_with_options(source, VideoSessionOptions::default())
    }

    #[cfg(test)]
    pub(crate) fn new_with_options(
        source: &str,
        options: VideoSessionOptions,
    ) -> Result<Self, String> {
        let prepared_source = crate::source::prepare_video_source(source)?;
        Self::from_prepared_source_with_options(prepared_source, options)
    }

    pub(crate) fn from_prepared_source_with_options(
        prepared_source: PreparedVideoSource,
        options: VideoSessionOptions,
    ) -> Result<Self, String> {
        let video = NativeVideo::new_with_metadata(
            prepared_source.uri(),
            prepared_source.metadata(),
            prepared_source.retained(),
            NativeVideoOptions {
                looping: Some(options.looping),
                audio_enabled: Some(options.audio_enabled),
                ..NativeVideoOptions::default()
            },
        )
        .map_err(|error| {
            format!(
                "failed to open video source {}: {error}",
                prepared_source.uri()
            )
        })?;
        video.set_muted(options.muted);
        Ok(Self {
            video,
            _prepared_source: prepared_source,
            latest_error: None,
            current_position: Duration::ZERO,
            volume: 1.0,
            muted: options.muted,
            paused: false,
            has_seen_frame: false,
        })
    }

    pub(crate) fn video(&self) -> NativeVideo {
        self.video.clone()
    }

    pub(crate) fn play(&mut self) {
        self.paused = false;
        self.video.set_paused(false);
    }

    pub(crate) fn pause(&mut self) {
        self.paused = true;
        self.video.set_paused(true);
    }

    pub(crate) fn shutdown_in_background(mut self) {
        self.paused = true;
        self.video.shutdown_in_background();
    }

    pub(crate) fn toggle_playback(&mut self) -> bool {
        if self.is_playing() {
            self.pause();
            false
        } else {
            self.play();
            true
        }
    }

    pub(crate) fn seek_to_fraction(&mut self, fraction: f64) {
        let Some(duration) = self.duration_seconds().filter(|duration| *duration > 0.0) else {
            return;
        };
        self.seek_to_seconds(duration * fraction.clamp(0.0, 1.0));
    }

    pub(crate) fn has_audio(&self) -> bool {
        self.video.has_audio()
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        if !self.has_audio() {
            return;
        }
        self.volume = volume.clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.muted = false;
        }
        self.video.set_volume(self.volume as f64);
        self.video.set_muted(self.muted);
    }

    pub(crate) fn toggle_muted(&mut self) {
        if !self.has_audio() {
            return;
        }
        self.muted = !self.muted;
        self.video.set_muted(self.muted);
    }

    pub(crate) fn sync(&mut self) -> bool {
        let has_renderable_frame = self.video.has_current_frame() || self.video.buffered_len() > 0;
        let current_position = self.video.position();
        let position_changed = current_position != self.current_position;
        let audio_error = self.video.audio_error();
        let audio_error_changed = audio_error.is_some() && audio_error != self.latest_error;
        let changed = (has_renderable_frame && !self.has_seen_frame)
            || position_changed
            || audio_error_changed;
        self.current_position = current_position;
        if has_renderable_frame {
            self.has_seen_frame = true;
        }
        if let Some(error) = audio_error {
            self.latest_error = Some(error);
        }
        changed
    }

    pub(crate) fn should_poll_for_frames(&self) -> bool {
        self.is_playing() || !self.has_seen_frame || self.video.has_pending_seek()
    }

    pub(crate) fn current_time_seconds(&self) -> f64 {
        self.video.position().as_secs_f64()
    }

    pub(crate) fn duration_seconds(&self) -> Option<f64> {
        let duration = self.video.duration();
        if duration.is_zero() {
            None
        } else {
            Some(duration.as_secs_f64())
        }
    }

    pub(crate) fn progress_fraction(&self) -> f32 {
        let Some(duration) = self.duration_seconds().filter(|duration| *duration > 0.0) else {
            return 0.0;
        };
        (self.current_time_seconds() / duration).clamp(0.0, 1.0) as f32
    }

    pub(crate) fn volume(&self) -> f32 {
        self.volume
    }

    pub(crate) fn muted(&self) -> bool {
        self.muted || self.video.muted()
    }

    pub(crate) fn is_playing(&self) -> bool {
        !self.paused && !self.video.eos()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.video.eos()
    }

    pub(crate) fn has_seen_frame(&self) -> bool {
        self.has_seen_frame
    }

    pub(crate) fn seek_pending(&self) -> bool {
        self.video.has_pending_seek()
    }

    pub(crate) fn has_decoded_frame(&self) -> bool {
        self.video.has_current_frame()
    }

    pub(crate) fn error_message(&self) -> Option<String> {
        self.latest_error.clone()
    }

    pub(crate) fn seek_to_seconds(&mut self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        let seconds = match self.duration_seconds() {
            Some(duration) if duration.is_finite() && duration > 0.0 => {
                seconds.clamp(0.0, duration)
            }
            _ => seconds.max(0.0),
        };
        match self
            .video
            .seek(Duration::from_secs_f64(seconds), false)
            .map_err(|error| format!("failed to seek video: {error}"))
        {
            Ok(()) => {
                self.current_position = Duration::from_secs_f64(seconds);
                if !self.paused {
                    self.video.set_paused(false);
                }
                self.latest_error = None;
            }
            Err(error) => {
                self.latest_error = Some(error);
            }
        }
    }
}
