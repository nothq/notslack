//! The microphone and speaker of a huddle, as 48 kHz mono sample queues.
//!
//! cpal streams cannot move between threads, so they live on a thread of
//! their own; samples cross to the call through lock-free ring buffers.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
};

/// The rate the call runs at: Opus in WebRTC is always 48 kHz.
pub(crate) const CALL_SAMPLE_RATE: u32 = 48_000;

/// About two seconds of audio either way, so a stall never blocks a device.
const QUEUE_SAMPLES: usize = CALL_SAMPLE_RATE as usize * 2;

/// The open devices; dropping it closes them.
pub(crate) struct AudioDevices {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioDevices {
    /// No devices: for a test that feeds and drains the queues itself.
    #[cfg(test)]
    pub(crate) fn none() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}

impl Drop for AudioDevices {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The call's view of the devices.
pub(crate) struct AudioIo {
    /// What the microphone hears; `None` when there is no microphone, and
    /// the call is listen-only.
    pub(crate) microphone: Option<rtrb::Consumer<f32>>,
    /// What the speaker plays.
    pub(crate) speaker: rtrb::Producer<f32>,
    pub(crate) devices: AudioDevices,
}

/// Opens the default microphone and speaker.
pub(crate) fn open_audio() -> Result<AudioIo, String> {
    let (microphone_in, microphone_out) = rtrb::RingBuffer::new(QUEUE_SAMPLES);
    let (speaker_in, speaker_out) = rtrb::RingBuffer::new(QUEUE_SAMPLES);
    let stop = Arc::new(AtomicBool::new(false));
    let (opened_sender, opened) = mpsc::channel();
    let thread_stop = stop.clone();
    let thread = thread::Builder::new()
        .name("notslack-huddle-audio".into())
        .spawn(move || {
            let streams = open_streams(microphone_in, speaker_out);
            let has_microphone = streams.as_ref().map(|streams| streams.microphone.is_some());
            let _ = opened_sender.send(has_microphone.map_err(Clone::clone));
            // Hold the streams open until the call ends.
            let _streams = streams;
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(50));
            }
        })
        .map_err(|error| format!("could not start the huddle audio thread: {error}"))?;
    let devices = AudioDevices {
        stop,
        thread: Some(thread),
    };
    let has_microphone = opened
        .recv()
        .map_err(|_| "the huddle audio thread stopped while opening devices".to_string())??;
    Ok(AudioIo {
        microphone: has_microphone.then_some(microphone_out),
        speaker: speaker_in,
        devices,
    })
}

struct Streams {
    microphone: Option<Stream>,
    _speaker: Stream,
}

fn open_streams(
    microphone: rtrb::Producer<f32>,
    speaker: rtrb::Consumer<f32>,
) -> Result<Streams, String> {
    let host = cpal::default_host();
    let output = host
        .default_output_device()
        .ok_or("there is no speaker or headphones to play the huddle")?;
    let speaker = build_speaker(&output, speaker)?;
    speaker
        .play()
        .map_err(|error| format!("could not start the speaker: {error}"))?;
    // A missing or failing microphone leaves the huddle listen-only.
    let microphone = host
        .default_input_device()
        .and_then(|input| build_microphone(&input, microphone).ok())
        .filter(|stream| stream.play().is_ok());
    Ok(Streams {
        microphone,
        _speaker: speaker,
    })
}

fn build_microphone(device: &cpal::Device, queue: rtrb::Producer<f32>) -> Result<Stream, String> {
    let supported = device
        .default_input_config()
        .map_err(|error| format!("the microphone has no usable format: {error}"))?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    match format {
        SampleFormat::F32 => microphone_stream::<f32>(device, &config, queue),
        SampleFormat::I16 => microphone_stream::<i16>(device, &config, queue),
        SampleFormat::U16 => microphone_stream::<u16>(device, &config, queue),
        SampleFormat::I32 => microphone_stream::<i32>(device, &config, queue),
        other => Err(format!("the microphone uses an unsupported format {other}")),
    }
}

fn microphone_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut queue: rtrb::Producer<f32>,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels.max(1));
    let mut resampler = Resampler::new(config.sample_rate.0, CALL_SAMPLE_RATE);
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                for frame in data.chunks(channels) {
                    let mono = frame
                        .iter()
                        .map(|sample| sample.to_sample::<f32>())
                        .sum::<f32>()
                        / frame.len() as f32;
                    resampler.push(mono, |sample| {
                        // A full queue means the call fell behind; drop.
                        let _ = queue.push(sample);
                    });
                }
            },
            |_| {},
            None,
        )
        .map_err(|error| format!("could not open the microphone: {error}"))
}

fn build_speaker(device: &cpal::Device, queue: rtrb::Consumer<f32>) -> Result<Stream, String> {
    let supported = device
        .default_output_config()
        .map_err(|error| format!("the speaker has no usable format: {error}"))?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    match format {
        SampleFormat::F32 => speaker_stream::<f32>(device, &config, queue),
        SampleFormat::I16 => speaker_stream::<i16>(device, &config, queue),
        SampleFormat::U16 => speaker_stream::<u16>(device, &config, queue),
        SampleFormat::I32 => speaker_stream::<i32>(device, &config, queue),
        other => Err(format!("the speaker uses an unsupported format {other}")),
    }
}

fn speaker_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut queue: rtrb::Consumer<f32>,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels.max(1));
    let mut resampler = Resampler::new(CALL_SAMPLE_RATE, config.sample_rate.0);
    let mut pending = std::collections::VecDeque::<f32>::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for frame in data.chunks_mut(channels) {
                    while pending.is_empty() {
                        // Silence when the call has nothing to play.
                        let next = queue.pop().unwrap_or(0.0);
                        resampler.push(next, |sample| pending.push_back(sample));
                    }
                    let sample = pending.pop_front().unwrap_or(0.0);
                    for channel in frame.iter_mut() {
                        *channel = T::from_sample(sample);
                    }
                }
            },
            |_| {},
            None,
        )
        .map_err(|error| format!("could not open the speaker: {error}"))
}

/// Linear interpolation from one sample rate to another; plenty for speech.
pub(crate) struct Resampler {
    /// Input samples per output sample.
    step: f64,
    /// Where the next output falls between the previous and next input.
    position: f64,
    previous: f32,
}

impl Resampler {
    pub(crate) fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            step: f64::from(input_rate) / f64::from(output_rate.max(1)),
            position: 0.0,
            previous: 0.0,
        }
    }

    /// Feeds one input sample, emitting the output samples it completes.
    pub(crate) fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
        while self.position < 1.0 {
            let t = self.position as f32;
            emit(self.previous + (sample - self.previous) * t);
            self.position += self.step;
        }
        self.position -= 1.0;
        self.previous = sample;
    }
}

#[cfg(test)]
mod tests {
    use super::Resampler;

    fn resample(input_rate: u32, output_rate: u32, input: usize) -> usize {
        let mut resampler = Resampler::new(input_rate, output_rate);
        let mut out = 0;
        for _ in 0..input {
            resampler.push(0.5, |_| out += 1);
        }
        out
    }

    #[test]
    fn resampling_keeps_the_duration() {
        assert_eq!(resample(48_000, 48_000, 960), 960);
        assert!((resample(44_100, 48_000, 44_100) as i64 - 48_000).abs() <= 1);
        assert!((resample(48_000, 16_000, 48_000) as i64 - 16_000).abs() <= 1);
    }

    #[test]
    fn resampling_interpolates_between_samples() {
        let mut resampler = Resampler::new(1, 2);
        let mut out = Vec::new();
        resampler.push(1.0, |sample| out.push(sample));
        resampler.push(3.0, |sample| out.push(sample));
        assert_eq!(out, [0.0, 0.5, 1.0, 2.0]);
    }
}
