use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;

use super::{audio::AudioPlayback, error::Error};

/// Position in the media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Position {
    /// Position based on time.
    Time(Duration),
    /// Position based on nth frame.
    Frame(u64),
}

impl From<Duration> for Position {
    fn from(t: Duration) -> Self {
        Position::Time(t)
    }
}

impl From<u64> for Position {
    fn from(f: u64) -> Self {
        Position::Frame(f)
    }
}

#[derive(Debug, Clone)]
pub struct VideoFrameData {
    pub nv12_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub uv_row_width: u32,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
pub(crate) struct RenderedVideoFrame {
    pub(crate) image: Arc<gpui::RenderImage>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Options for initializing a `Video` without post-construction locking.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Optional initial frame buffer capacity (0 disables buffering). Defaults to 3.
    pub frame_buffer_capacity: Option<usize>,
    /// Optional initial looping flag. Defaults to false.
    pub looping: Option<bool>,
    /// Optional audio playback flag. Defaults to true.
    pub audio_enabled: Option<bool>,
    /// Optional initial playback speed. Defaults to 1.0.
    pub speed: Option<f64>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            frame_buffer_capacity: Some(3),
            looping: Some(false),
            audio_enabled: Some(true),
            speed: Some(1.0),
        }
    }
}

#[derive(Debug)]
pub(crate) enum PlaybackCommand {
    SetPaused(bool),
    Seek { target: Duration, generation: u64 },
    SetSpeed(f64),
    Restart,
    Shutdown,
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct Internal {
    pub(crate) id: u64,
    pub(crate) source: String,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) control_tx: Sender<PlaybackCommand>,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    pub(crate) duration: Duration,
    pub(crate) speed: Arc<AtomicU64>,
    pub(crate) current_position_ns: Arc<AtomicU64>,
    pub(crate) seek_generation: Arc<AtomicU64>,
    pub(crate) pending_seek_generation: Arc<AtomicU64>,

    pub(crate) frame: Arc<Mutex<Option<VideoFrameData>>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    pub(crate) render_surface_pool: Option<video_native::Nv12PixelBufferPool>,
    #[cfg(target_os = "macos")]
    pub(crate) render_surfaces: VecDeque<video_native::Nv12PixelBuffer>,
    #[cfg(target_os = "linux")]
    pub(crate) rendered_frame: Arc<Mutex<Option<RenderedVideoFrame>>>,
    pub(crate) frame_buffer: Arc<Mutex<VecDeque<VideoFrameData>>>,
    pub(crate) frame_buffer_capacity: Arc<AtomicUsize>,
    pub(crate) last_frame_time: Arc<Mutex<Instant>>,
    pub(crate) looping: Arc<AtomicBool>,
    pub(crate) is_eos: Arc<AtomicBool>,
    pub(crate) paused: Arc<AtomicBool>,

    pub(crate) subtitle_text: Arc<Mutex<Option<String>>>,
    pub(crate) upload_text: Arc<AtomicBool>,

    pub(crate) display_width_override: Option<u32>,
    pub(crate) display_height_override: Option<u32>,
    pub(crate) volume: f64,
    pub(crate) muted: bool,
    pub(crate) audio: Option<AudioPlayback>,
    pub(crate) audio_start_error: Option<String>,
}

impl Internal {
    pub(crate) fn seek(&self, position: impl Into<Position>, _accurate: bool) -> Result<(), Error> {
        let target = position_to_duration(position.into(), self.framerate);
        let generation = self
            .seek_generation
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        let resume_audio = !self.paused.load(Ordering::SeqCst);
        self.is_eos.store(false, Ordering::SeqCst);
        self.frame_buffer.lock().clear();
        self.upload_frame.store(false, Ordering::SeqCst);
        self.pending_seek_generation
            .store(generation, Ordering::SeqCst);
        self.current_position_ns
            .store(target.as_nanos() as u64, Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.seek_and_hold(target);
        }
        let result = self.send_command(PlaybackCommand::Seek { target, generation });
        if result.is_err() {
            self.pending_seek_generation.store(0, Ordering::SeqCst);
        }
        if resume_audio {
            if let Some(audio) = &self.audio {
                audio.release_after_seek();
            }
        }
        result
    }

    pub(crate) fn set_speed(&mut self, speed: f64) -> Result<(), Error> {
        validate_speed(speed)?;
        self.speed.store(speed.to_bits(), Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.set_speed(
                speed,
                Duration::from_nanos(self.current_position_ns.load(Ordering::SeqCst)),
            );
        }
        self.send_command(PlaybackCommand::SetSpeed(speed))
    }

    pub(crate) fn restart_stream(&mut self) -> Result<(), Error> {
        self.is_eos.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.current_position_ns.store(0, Ordering::SeqCst);
        self.pending_seek_generation.store(0, Ordering::SeqCst);
        self.frame_buffer.lock().clear();
        self.upload_frame.store(false, Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.restart();
        }
        self.send_command(PlaybackCommand::Restart)
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        let command = if self.is_eos.load(Ordering::Acquire) && !paused {
            PlaybackCommand::Restart
        } else {
            PlaybackCommand::SetPaused(paused)
        };
        if let Some(audio) = &self.audio {
            match command {
                PlaybackCommand::Restart => audio.restart(),
                PlaybackCommand::SetPaused(paused) => audio.set_paused(paused),
                _ => {}
            }
        }
        if let Err(err) = self.send_command(command) {
            log::error!("failed to update video pause state: {err}");
        }
    }

    pub(crate) fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    fn send_command(&self, command: PlaybackCommand) -> Result<(), Error> {
        self.control_tx
            .send(command)
            .map_err(|_| Error::WorkerStopped)
    }
}

pub(crate) fn validate_speed(speed: f64) -> Result<(), Error> {
    if speed.is_finite() && speed > 0.0 {
        return Ok(());
    }
    Err(Error::PlaybackSpeed(speed))
}

pub(crate) fn position_to_duration(position: Position, framerate: f64) -> Duration {
    match position {
        Position::Time(duration) => duration,
        Position::Frame(frame) => {
            if !framerate.is_finite() || framerate <= 0.0 {
                return Duration::ZERO;
            }
            Duration::from_secs_f64(frame as f64 / framerate)
        }
    }
}
