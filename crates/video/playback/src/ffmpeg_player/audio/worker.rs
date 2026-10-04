use std::{
    any::Any,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    time::Duration,
};

use parking_lot::Mutex;
use video_native::{FfmpegAudioDecoder, FfmpegAudioOutputConfig, FfmpegError};

use super::{output::AudioSampleQueue, seek::prefill_audio_queue, AudioPlaybackState};
use crate::ffmpeg_player::ffmpeg_decoder_lock;

const OUTPUT_QUEUE_TARGET_MS: usize = 125;

#[derive(Debug)]
pub(super) enum AudioCommand {
    SetPaused(bool),
    Seek {
        position: Duration,
        ready_tx: mpsc::Sender<()>,
    },
    Restart,
    SetLooping(bool),
    SetSpeed {
        speed: f64,
        position: Duration,
    },
    Shutdown,
}

pub(super) struct AudioWorkerInput {
    pub(super) source: String,
    pub(super) _retained_source: Option<Arc<dyn Any + Send + Sync>>,
    pub(super) output: FfmpegAudioOutputConfig,
    pub(super) queue: Arc<AudioSampleQueue>,
    pub(super) control_rx: Receiver<AudioCommand>,
    pub(super) error: Arc<Mutex<Option<String>>>,
    pub(super) state: Arc<AtomicU8>,
}

struct AudioWorkerState {
    paused: bool,
    looping: bool,
    speed_supported: bool,
    decoder_exhausted: bool,
    target_samples: usize,
}

impl AudioWorkerState {
    fn new(output: FfmpegAudioOutputConfig) -> Self {
        Self {
            paused: false,
            looping: false,
            speed_supported: true,
            decoder_exhausted: false,
            target_samples: output_target_samples(output),
        }
    }
}

pub(super) fn spawn_audio_worker(input: AudioWorkerInput) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || run_audio_worker(input))
}

fn run_audio_worker(input: AudioWorkerInput) {
    let Some(mut decoder) = open_worker_decoder(&input) else {
        return;
    };
    let mut state = AudioWorkerState::new(input.output);
    loop {
        let keep_running = with_audio_decoder(&mut decoder, |decoder| {
            drain_audio_commands(&input, decoder, &mut state)
        });
        if !keep_running {
            break;
        }
        if state.paused || !state.speed_supported {
            std::thread::sleep(Duration::from_millis(16));
        } else if state.decoder_exhausted {
            mark_finished_when_drained(&input);
            std::thread::sleep(Duration::from_millis(16));
        } else if input.queue.len() > state.target_samples {
            std::thread::sleep(Duration::from_millis(8));
        } else {
            decode_next_frame(&input, &mut decoder, &mut state);
        }
    }
    drop_audio_decoder(decoder);
}

fn open_worker_decoder(input: &AudioWorkerInput) -> Option<FfmpegAudioDecoder> {
    match open_audio_decoder(&input.source, input.output) {
        Ok(decoder) => Some(decoder),
        Err(error) => {
            *input.error.lock() = Some(format!("failed to open audio decoder: {error}"));
            input
                .state
                .store(AudioPlaybackState::Finished as u8, Ordering::SeqCst);
            None
        }
    }
}

fn mark_finished_when_drained(input: &AudioWorkerInput) {
    if input.queue.len() == 0 {
        input
            .state
            .store(AudioPlaybackState::Finished as u8, Ordering::SeqCst);
    }
}

fn decode_next_frame(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
) {
    match with_audio_decoder(decoder, FfmpegAudioDecoder::next_frame) {
        Ok(Some(frame)) if !frame.samples.is_empty() => {
            input.queue.push_interleaved(&frame.samples)
        }
        Ok(Some(_)) => {}
        Ok(None) if state.looping => restart_looping_decoder(input, decoder),
        Ok(None) => state.decoder_exhausted = true,
        Err(error) => {
            *input.error.lock() = Some(format!("error decoding audio frame: {error}"));
            state.decoder_exhausted = true;
        }
    }
}

fn restart_looping_decoder(input: &AudioWorkerInput, decoder: &mut FfmpegAudioDecoder) {
    input.queue.reset_position(Duration::ZERO);
    if let Err(error) = with_audio_decoder(decoder, |decoder| decoder.seek(Duration::ZERO)) {
        *input.error.lock() = Some(format!("failed to restart audio: {error}"));
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn drain_audio_commands(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
) -> bool {
    loop {
        match input.control_rx.try_recv() {
            Ok(command) => {
                if !apply_audio_command(command, input, decoder, state) {
                    return false;
                }
            }
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
}

fn apply_audio_command(
    command: AudioCommand,
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
) -> bool {
    match command {
        AudioCommand::SetPaused(paused) => apply_pause(input, state, paused),
        AudioCommand::Seek { position, ready_tx } => {
            apply_seek(input, decoder, state, position);
            let _ = ready_tx.send(());
        }
        AudioCommand::Restart => apply_restart(input, decoder, state),
        AudioCommand::SetLooping(looping) => state.looping = looping,
        AudioCommand::SetSpeed { speed, position } => {
            apply_speed(input, decoder, state, speed, position)
        }
        AudioCommand::Shutdown => return false,
    }
    true
}

fn apply_pause(input: &AudioWorkerInput, state: &mut AudioWorkerState, paused: bool) {
    state.paused = paused;
    if paused {
        input.queue.hold_output();
    } else {
        input.queue.release_output();
    }
    input.state.store(
        if paused {
            AudioPlaybackState::Paused
        } else {
            AudioPlaybackState::Playing
        } as u8,
        Ordering::SeqCst,
    );
}

fn apply_seek(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
    position: Duration,
) {
    state.decoder_exhausted = false;
    input.queue.reset_position(position);
    if let Err(error) = decoder.seek(position) {
        *input.error.lock() = Some(format!("failed to seek audio: {error}"));
    } else if state.speed_supported {
        prefill_audio_queue(input, decoder, position, state.target_samples);
    }
}

fn apply_restart(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
) {
    state.decoder_exhausted = false;
    input.queue.reset_position(Duration::ZERO);
    input.queue.release_output();
    if let Err(error) = decoder.seek(Duration::ZERO) {
        *input.error.lock() = Some(format!("failed to restart audio: {error}"));
        state.decoder_exhausted = true;
    } else {
        *input.error.lock() = None;
    }
    state.paused = false;
    state.speed_supported = true;
    if !state.decoder_exhausted {
        input
            .state
            .store(AudioPlaybackState::Playing as u8, Ordering::SeqCst);
    }
}

fn apply_speed(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    state: &mut AudioWorkerState,
    speed: f64,
    position: Duration,
) {
    state.decoder_exhausted = false;
    state.speed_supported = (speed - 1.0).abs() <= f64::EPSILON;
    input.queue.reset_position(position);
    if state.speed_supported {
        if let Err(error) = decoder.seek(position) {
            *input.error.lock() = Some(format!("failed to resync audio: {error}"));
        }
    }
}

fn output_target_samples(output: FfmpegAudioOutputConfig) -> usize {
    output.sample_rate as usize * usize::from(output.channels) * OUTPUT_QUEUE_TARGET_MS / 1_000
}

fn open_audio_decoder(
    source: &str,
    output: FfmpegAudioOutputConfig,
) -> Result<FfmpegAudioDecoder, FfmpegError> {
    let _guard = ffmpeg_decoder_lock();
    FfmpegAudioDecoder::open(source, output)
}

fn with_audio_decoder<T>(
    decoder: &mut FfmpegAudioDecoder,
    operation: impl FnOnce(&mut FfmpegAudioDecoder) -> T,
) -> T {
    let _guard = ffmpeg_decoder_lock();
    operation(decoder)
}

fn drop_audio_decoder(decoder: FfmpegAudioDecoder) {
    let _guard = ffmpeg_decoder_lock();
    drop(decoder);
}
