use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use cpal::{traits::DeviceTrait, Device, SampleFormat, SizedSample, Stream};
use parking_lot::Mutex;
use video_native::FfmpegAudioOutputConfig;

#[derive(Debug)]
pub(super) struct AudioSampleQueue {
    state: Mutex<AudioSampleQueueState>,
    max_samples: usize,
    interleaved_samples_per_second: u64,
}

#[derive(Debug)]
struct AudioSampleQueueState {
    samples: VecDeque<i16>,
    position_base: Duration,
    consumed_interleaved_samples: u64,
    output_held: bool,
}

impl AudioSampleQueue {
    #[cfg(test)]
    pub(super) fn new(max_samples: usize) -> Self {
        Self::for_output(
            max_samples,
            FfmpegAudioOutputConfig {
                sample_rate: 1,
                channels: 1,
            },
        )
    }

    pub(super) fn for_output(max_samples: usize, output: FfmpegAudioOutputConfig) -> Self {
        let interleaved_samples_per_second =
            u64::from(output.sample_rate) * u64::from(output.channels);
        assert!(
            interleaved_samples_per_second > 0,
            "audio output sample rate and channel count must be nonzero"
        );
        Self {
            state: Mutex::new(AudioSampleQueueState {
                samples: VecDeque::new(),
                position_base: Duration::ZERO,
                consumed_interleaved_samples: 0,
                output_held: false,
            }),
            max_samples: max_samples.max(1),
            interleaved_samples_per_second,
        }
    }

    pub(super) fn push_interleaved(&self, samples: &[i16]) {
        let mut state = self.state.lock();
        let overflow = state
            .samples
            .len()
            .saturating_add(samples.len())
            .saturating_sub(self.max_samples);
        let queued_drop_count = overflow.min(state.samples.len());
        if queued_drop_count > 0 {
            let _ = state.samples.drain(..queued_drop_count);
        }
        let incoming_start = overflow.saturating_sub(queued_drop_count);
        state
            .samples
            .extend(samples.iter().skip(incoming_start).copied());
    }

    pub(super) fn pop_output_sample(&self, muted: bool, volume: f64) -> i16 {
        let sample = {
            let mut state = self.state.lock();
            if state.output_held {
                return 0;
            }
            let Some(sample) = state.samples.pop_front() else {
                return 0;
            };
            state.consumed_interleaved_samples =
                state.consumed_interleaved_samples.saturating_add(1);
            sample
        };
        if muted {
            return 0;
        }
        scale_sample(sample, volume)
    }

    pub(super) fn push_silence(&self, sample_count: usize) {
        if sample_count == 0 {
            return;
        }
        let silence = vec![0; sample_count.min(self.max_samples)];
        self.push_interleaved(&silence);
    }

    pub(super) fn hold_output(&self) {
        self.state.lock().output_held = true;
    }

    pub(super) fn release_output(&self) {
        self.state.lock().output_held = false;
    }

    pub(super) fn reset_position(&self, position: Duration) {
        let mut state = self.state.lock();
        state.samples.clear();
        state.position_base = position;
        state.consumed_interleaved_samples = 0;
    }

    pub(super) fn len(&self) -> usize {
        self.state.lock().samples.len()
    }

    pub(super) fn position(&self) -> Duration {
        let state = self.state.lock();
        state.position_base.saturating_add(sample_duration(
            state.consumed_interleaved_samples,
            self.interleaved_samples_per_second,
        ))
    }
}

fn sample_duration(interleaved_samples: u64, interleaved_samples_per_second: u64) -> Duration {
    let seconds = interleaved_samples / interleaved_samples_per_second;
    let remaining_samples = interleaved_samples % interleaved_samples_per_second;
    let nanoseconds = (u128::from(remaining_samples) * 1_000_000_000)
        / u128::from(interleaved_samples_per_second);
    Duration::new(seconds, nanoseconds as u32)
}

pub(super) fn resolve_output_config(
    device: &Device,
) -> Result<cpal::SupportedStreamConfig, String> {
    device
        .default_output_config()
        .map_err(|error| format!("failed to read video speaker config: {error}"))
}

pub(super) fn build_output_stream(
    device: &Device,
    config: &cpal::SupportedStreamConfig,
    queue: Arc<AudioSampleQueue>,
    volume: Arc<AtomicU64>,
    muted: Arc<AtomicBool>,
) -> Result<Stream, String> {
    match config.sample_format() {
        SampleFormat::I16 => build_typed_output_stream::<i16>(device, config, queue, volume, muted),
        SampleFormat::U16 => build_typed_output_stream::<u16>(device, config, queue, volume, muted),
        SampleFormat::F32 => build_typed_output_stream::<f32>(device, config, queue, volume, muted),
        SampleFormat::I32 => build_typed_output_stream::<i32>(device, config, queue, volume, muted),
        SampleFormat::U32 => build_typed_output_stream::<u32>(device, config, queue, volume, muted),
        SampleFormat::F64 => build_typed_output_stream::<f64>(device, config, queue, volume, muted),
        other => Err(format!(
            "unsupported video speaker sample format: {other:?}"
        )),
    }
}

fn build_typed_output_stream<T>(
    device: &Device,
    config: &cpal::SupportedStreamConfig,
    queue: Arc<AudioSampleQueue>,
    volume: Arc<AtomicU64>,
    muted: Arc<AtomicBool>,
) -> Result<Stream, String>
where
    T: SizedSample + cpal::FromSample<i16>,
{
    device
        .build_output_stream(
            &config.config(),
            move |data: &mut [T], _| {
                let muted = muted.load(Ordering::SeqCst);
                let volume = f64::from_bits(volume.load(Ordering::SeqCst)).clamp(0.0, 1.0);
                for sample in data.iter_mut() {
                    *sample = T::from_sample(queue.pop_output_sample(muted, volume));
                }
            },
            move |error| {
                log::error!("video audio stream error: {error}");
            },
            None,
        )
        .map_err(|error| format!("failed to build video audio stream: {error}"))
}

pub(super) fn scale_sample(sample: i16, volume: f64) -> i16 {
    let scaled = sample as f64 * volume.clamp(0.0, 1.0);
    scaled.clamp(i16::MIN as f64, i16::MAX as f64).round() as i16
}
