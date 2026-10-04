use std::{path::Path, time::Duration};

use gpui::{
    div, prelude::FluentBuilder, px, App, Div, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Styled, Window,
};
use gpui_components::alpha;

use crate::ffmpeg_player::audio::{AudioPlayback, AudioPlaybackSnapshot, AudioPlaybackState};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioPreviewState {
    #[default]
    Idle,
    Playing,
    Paused,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioPreviewSnapshot {
    pub state: AudioPreviewState,
    pub position: Duration,
    pub duration: Duration,
    pub muted: bool,
    pub volume: f64,
}

impl AudioPreviewSnapshot {
    pub fn progress_fraction(self) -> f64 {
        if self.duration.is_zero() {
            return 0.0;
        }
        (self.position.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0)
    }
}

impl Default for AudioPreviewSnapshot {
    fn default() -> Self {
        Self {
            state: AudioPreviewState::Idle,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            muted: false,
            volume: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioPreviewControlStyle {
    pub border: u32,
    pub background: u32,
    pub enabled_border_alpha: f32,
    pub disabled_border_alpha: f32,
    pub enabled_background_alpha: f32,
    pub disabled_background_alpha: f32,
}

#[derive(Default)]
pub struct AudioPreviewPlayer {
    source: Option<String>,
    playback: Option<AudioPlayback>,
}

impl AudioPreviewPlayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play_file(&mut self, path: &Path) -> Result<(), String> {
        let source = path.to_string_lossy().into_owned();
        self.play_source(&source)
    }

    pub fn play_source(&mut self, source: &str) -> Result<(), String> {
        if self.source.as_deref() == Some(source) {
            if let Some(playback) = self.playback.as_ref() {
                playback.restart();
                return Ok(());
            }
        }
        let Some(playback) = AudioPlayback::start(source.to_string(), None)? else {
            return Err("audio source has no playable audio stream".to_string());
        };
        playback.set_volume(1.0);
        playback.set_muted(false);
        playback.set_looping(false);
        self.source = Some(source.to_string());
        self.playback = Some(playback);
        Ok(())
    }

    pub fn toggle(&mut self) -> AudioPreviewState {
        match self.state() {
            AudioPreviewState::Idle => AudioPreviewState::Idle,
            AudioPreviewState::Playing => self.pause(),
            AudioPreviewState::Paused | AudioPreviewState::Finished => self.play(),
        }
    }

    pub fn play(&self) -> AudioPreviewState {
        match self.state() {
            AudioPreviewState::Idle | AudioPreviewState::Playing => self.state(),
            AudioPreviewState::Paused => {
                self.playback
                    .as_ref()
                    .expect("paused audio preview must have playback")
                    .set_paused(false);
                self.state()
            }
            AudioPreviewState::Finished => self.replay(),
        }
    }

    pub fn pause(&self) -> AudioPreviewState {
        if self.state() == AudioPreviewState::Playing {
            self.playback
                .as_ref()
                .expect("playing audio preview must have playback")
                .set_paused(true);
        }
        self.state()
    }

    pub fn replay(&self) -> AudioPreviewState {
        if let Some(playback) = self.playback.as_ref() {
            playback.restart();
        }
        self.state()
    }

    pub fn seek(&self, position: Duration) -> AudioPreviewSnapshot {
        if let Some(playback) = self.playback.as_ref() {
            let duration = playback.snapshot().duration;
            let position = if duration.is_zero() {
                position
            } else {
                position.min(duration)
            };
            playback.seek(position);
        }
        self.snapshot()
    }

    pub fn seek_fraction(&self, fraction: f64) -> AudioPreviewSnapshot {
        assert!(
            fraction.is_finite(),
            "audio preview seek fraction must be finite"
        );
        let duration = self.duration();
        self.seek(duration.mul_f64(fraction.clamp(0.0, 1.0)))
    }

    pub fn set_muted(&self, muted: bool) {
        if let Some(playback) = self.playback.as_ref() {
            playback.set_muted(muted);
        }
    }

    pub fn toggle_muted(&self) -> bool {
        let muted = !self.muted();
        self.set_muted(muted);
        self.muted()
    }

    pub fn set_volume(&self, volume: f64) {
        assert!(volume.is_finite(), "audio preview volume must be finite");
        if let Some(playback) = self.playback.as_ref() {
            playback.set_volume(volume);
        }
    }

    pub fn position(&self) -> Duration {
        self.snapshot().position
    }

    pub fn duration(&self) -> Duration {
        self.snapshot().duration
    }

    pub fn muted(&self) -> bool {
        self.snapshot().muted
    }

    pub fn volume(&self) -> f64 {
        self.snapshot().volume
    }

    pub fn state(&self) -> AudioPreviewState {
        self.snapshot().state
    }

    pub fn snapshot(&self) -> AudioPreviewSnapshot {
        self.playback
            .as_ref()
            .map(AudioPlayback::snapshot)
            .map(audio_preview_snapshot)
            .unwrap_or_default()
    }

    pub fn stop(&mut self) {
        self.source = None;
        self.playback = None;
    }

    pub fn error_message(&self) -> Option<String> {
        self.playback
            .as_ref()
            .and_then(AudioPlayback::error_message)
    }
}

fn audio_preview_snapshot(snapshot: AudioPlaybackSnapshot) -> AudioPreviewSnapshot {
    AudioPreviewSnapshot {
        state: match snapshot.state {
            AudioPlaybackState::Playing => AudioPreviewState::Playing,
            AudioPlaybackState::Paused => AudioPreviewState::Paused,
            AudioPlaybackState::Finished => AudioPreviewState::Finished,
        },
        position: snapshot.position,
        duration: snapshot.duration,
        muted: snapshot.muted,
        volume: snapshot.volume,
    }
}

pub fn audio_preview_control(
    icon: impl IntoElement,
    enabled: bool,
    style: AudioPreviewControlStyle,
    listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex_none()
        .w(px(32.0))
        .h(px(32.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(alpha(
            style.border,
            if enabled {
                style.enabled_border_alpha
            } else {
                style.disabled_border_alpha
            },
        ))
        .bg(alpha(
            style.background,
            if enabled {
                style.enabled_background_alpha
            } else {
                style.disabled_background_alpha
            },
        ))
        .flex()
        .items_center()
        .justify_center()
        .when(enabled, |this| {
            this.cursor_pointer()
                .on_mouse_down(MouseButton::Left, listener)
        })
        .child(icon)
}
