use std::{
    any::Any,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use cpal::{
    traits::{HostTrait, StreamTrait},
    Stream,
};
use parking_lot::Mutex;
use video_native::{FfmpegAudioMetadata, FfmpegAudioOutputConfig, FfmpegError};

use super::{ffmpeg_decoder_lock, worker::join_worker_in_background};
use output::{build_output_stream, resolve_output_config, AudioSampleQueue};
use worker::{spawn_audio_worker, AudioCommand, AudioWorkerInput};

mod output;
mod seek;
mod worker;

const OUTPUT_QUEUE_MS: usize = 250;
const SEEK_PREFILL_TIMEOUT_MS: u64 = 250;

pub(crate) struct AudioPlayback {
    command_tx: mpsc::Sender<AudioCommand>,
    worker: Option<std::thread::JoinHandle<()>>,
    _stream: Stream,
    queue: Arc<AudioSampleQueue>,
    volume: Arc<AtomicU64>,
    muted: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    state: Arc<AtomicU8>,
    duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AudioPlaybackState {
    Playing = 0,
    Paused = 1,
    Finished = 2,
}

impl AudioPlaybackState {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Playing,
            1 => Self::Paused,
            2 => Self::Finished,
            _ => unreachable!("invalid audio playback state"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AudioPlaybackSnapshot {
    pub(crate) state: AudioPlaybackState,
    pub(crate) position: Duration,
    pub(crate) duration: Duration,
    pub(crate) muted: bool,
    pub(crate) volume: f64,
}

impl std::fmt::Debug for AudioPlayback {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioPlayback")
            .field("queued_samples", &self.queue.len())
            .field("position", &self.queue.position())
            .field("duration", &self.duration)
            .field("muted", &self.muted.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl AudioPlayback {
    pub(crate) fn start(
        source: String,
        retained_source: Option<Arc<dyn Any + Send + Sync>>,
    ) -> Result<Option<Self>, String> {
        let metadata = match probe_audio_stream(&source) {
            Ok(metadata) => metadata,
            Err(FfmpegError::AudioStream) => return Ok(None),
            Err(error) => return Err(format!("failed to inspect audio stream: {error}")),
        };
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no output audio device is available for video".to_string())?;
        let config = resolve_output_config(&device)?;
        let output = FfmpegAudioOutputConfig {
            sample_rate: config.sample_rate().0,
            channels: config.channels(),
        };
        let queue = Arc::new(AudioSampleQueue::for_output(
            output.sample_rate as usize * usize::from(output.channels) * OUTPUT_QUEUE_MS / 1_000,
            output,
        ));
        let volume = Arc::new(AtomicU64::new(1.0f64.to_bits()));
        let muted = Arc::new(AtomicBool::new(false));
        let stream = build_output_stream(
            &device,
            &config,
            queue.clone(),
            volume.clone(),
            muted.clone(),
        )?;
        stream
            .play()
            .map_err(|error| format!("failed to start video audio stream: {error}"))?;
        let (command_tx, control_rx) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let state = Arc::new(AtomicU8::new(AudioPlaybackState::Playing as u8));
        let worker = spawn_audio_worker(AudioWorkerInput {
            source,
            _retained_source: retained_source,
            output,
            queue: queue.clone(),
            control_rx,
            error: error.clone(),
            state: state.clone(),
        });
        Ok(Some(Self {
            command_tx,
            worker: Some(worker),
            _stream: stream,
            queue,
            volume,
            muted,
            error,
            state,
            duration: metadata.duration,
        }))
    }

    pub(crate) fn set_volume(&self, volume: f64) {
        assert!(volume.is_finite(), "audio volume must be finite");
        self.volume
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::SeqCst);
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        if paused {
            self.queue.hold_output();
        } else {
            self.queue.release_output();
        }
        if self
            .command_tx
            .send(AudioCommand::SetPaused(paused))
            .is_ok()
        {
            self.set_state(if paused {
                AudioPlaybackState::Paused
            } else {
                AudioPlaybackState::Playing
            });
        } else {
            self.set_state(AudioPlaybackState::Finished);
        }
    }

    pub(crate) fn seek_and_hold(&self, position: Duration) {
        self.queue.hold_output();
        self.queue.reset_position(position);
        let (ready_tx, ready_rx) = mpsc::channel();
        if self
            .command_tx
            .send(AudioCommand::Seek { position, ready_tx })
            .is_err()
        {
            self.queue.release_output();
            self.set_state(AudioPlaybackState::Finished);
            return;
        }
        if ready_rx
            .recv_timeout(Duration::from_millis(SEEK_PREFILL_TIMEOUT_MS))
            .is_err()
        {
            log::warn!("audio seek prebuffer did not complete before timeout");
        }
    }

    pub(crate) fn release_after_seek(&self) {
        self.queue.release_output();
    }

    pub(crate) fn seek(&self, position: Duration) {
        let state = self.state();
        self.seek_and_hold(position);
        match state {
            AudioPlaybackState::Playing => self.release_after_seek(),
            AudioPlaybackState::Paused => {}
            AudioPlaybackState::Finished => self.set_paused(true),
        }
    }

    pub(crate) fn restart(&self) {
        self.queue.reset_position(Duration::ZERO);
        self.queue.release_output();
        if self.command_tx.send(AudioCommand::Restart).is_ok() {
            self.set_state(AudioPlaybackState::Playing);
        } else {
            self.set_state(AudioPlaybackState::Finished);
        }
    }

    pub(crate) fn set_looping(&self, looping: bool) {
        let _ = self.command_tx.send(AudioCommand::SetLooping(looping));
    }

    pub(crate) fn set_speed(&self, speed: f64, position: Duration) {
        self.queue.reset_position(position);
        let _ = self
            .command_tx
            .send(AudioCommand::SetSpeed { speed, position });
    }

    pub(crate) fn error_message(&self) -> Option<String> {
        self.error.lock().clone()
    }

    pub(crate) fn state(&self) -> AudioPlaybackState {
        AudioPlaybackState::from_u8(self.state.load(Ordering::SeqCst))
    }

    pub(crate) fn snapshot(&self) -> AudioPlaybackSnapshot {
        AudioPlaybackSnapshot {
            state: self.state(),
            position: self.queue.position(),
            duration: self.duration,
            muted: self.muted.load(Ordering::SeqCst),
            volume: f64::from_bits(self.volume.load(Ordering::SeqCst)),
        }
    }

    fn set_state(&self, state: AudioPlaybackState) {
        self.state.store(state as u8, Ordering::SeqCst);
    }
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        let _ = self.command_tx.send(AudioCommand::Shutdown);
        join_worker_in_background("audio", self.worker.take());
    }
}

fn probe_audio_stream(source: &str) -> Result<FfmpegAudioMetadata, FfmpegError> {
    let _guard = ffmpeg_decoder_lock();
    video_native::probe_audio_metadata(source)
}

#[cfg(test)]
mod audio_tests;
