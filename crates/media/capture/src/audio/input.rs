use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    thread,
};

use cpal::{
    traits::{DeviceTrait, HostTrait},
    FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedStreamConfig,
};
use rtrb::Producer;

use super::{
    CAPTURE_EVENT_BUFFER_OVERFLOW, CAPTURE_EVENT_DEVICE_STREAM, CAPTURE_EVENT_DURATION_LIMIT,
    CAPTURE_EVENT_NONE,
};
use crate::{MediaCaptureError, MAX_AUDIO_CLIP_DURATION};

const AUDIO_RING_BUFFER_SECONDS: usize = 2;

pub(super) struct AudioInputConfiguration {
    pub(super) device: cpal::Device,
    pub(super) supported: SupportedStreamConfig,
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) ring_capacity: usize,
    pub(super) maximum_samples: u64,
}

pub(super) fn default_input_configuration() -> Result<AudioInputConfiguration, MediaCaptureError> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or(MediaCaptureError::NoInputDevice)?;
    let supported = device
        .default_input_config()
        .map_err(|error| MediaCaptureError::InputConfiguration(error.to_string()))?;
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    if channels == 0 || sample_rate == 0 {
        return Err(MediaCaptureError::InputConfiguration(
            "the default microphone input configuration is invalid".to_string(),
        ));
    }
    let ring_capacity = usize::try_from(sample_rate)
        .ok()
        .and_then(|rate| rate.checked_mul(AUDIO_RING_BUFFER_SECONDS))
        .ok_or_else(|| {
            MediaCaptureError::InputConfiguration(
                "the microphone sample rate exceeds the bounded buffer capacity".to_string(),
            )
        })?;
    let maximum_samples = u64::from(sample_rate)
        .checked_mul(MAX_AUDIO_CLIP_DURATION.as_secs())
        .ok_or_else(|| {
            MediaCaptureError::InputConfiguration(
                "the microphone sample rate exceeds the clip duration limit".to_string(),
            )
        })?;
    Ok(AudioInputConfiguration {
        device,
        supported,
        sample_rate,
        channels,
        ring_capacity,
        maximum_samples,
    })
}

pub(super) struct AudioInputCallbackContext {
    pub(super) producer: Producer<i16>,
    pub(super) channels: u16,
    pub(super) maximum_samples: u64,
    pub(super) accepted_samples: u64,
    pub(super) stop_requested: Arc<AtomicBool>,
    pub(super) capture_event: Arc<AtomicU8>,
    pub(super) writer_thread: thread::Thread,
}

impl AudioInputCallbackContext {
    fn consume<T>(&mut self, data: &[T])
    where
        T: SizedSample + Copy,
        i16: FromSample<T>,
    {
        if self.stop_requested.load(Ordering::Acquire) {
            return;
        }
        for frame in data.chunks_exact(usize::from(self.channels)) {
            if self.accepted_samples == self.maximum_samples {
                self.record_event(CAPTURE_EVENT_DURATION_LIMIT);
                break;
            }
            let sum = frame
                .iter()
                .map(|sample| i64::from(i16::from_sample(*sample)))
                .sum::<i64>();
            let mono = (sum / i64::from(self.channels)) as i16;
            if self.producer.push(mono).is_err() {
                self.record_event(CAPTURE_EVENT_BUFFER_OVERFLOW);
                break;
            }
            self.accepted_samples += 1;
        }
        self.writer_thread.unpark();
    }

    fn record_event(&self, event: u8) {
        record_capture_event(
            &self.capture_event,
            event,
            &self.stop_requested,
            &self.writer_thread,
        );
    }
}

pub(super) fn build_input_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    context: AudioInputCallbackContext,
) -> Result<Stream, MediaCaptureError> {
    match config.sample_format() {
        SampleFormat::I8 => build_typed_input_stream::<i8>(device, config, context),
        SampleFormat::I16 => build_typed_input_stream::<i16>(device, config, context),
        SampleFormat::I32 => build_typed_input_stream::<i32>(device, config, context),
        SampleFormat::I64 => build_typed_input_stream::<i64>(device, config, context),
        SampleFormat::U8 => build_typed_input_stream::<u8>(device, config, context),
        SampleFormat::U16 => build_typed_input_stream::<u16>(device, config, context),
        SampleFormat::U32 => build_typed_input_stream::<u32>(device, config, context),
        SampleFormat::U64 => build_typed_input_stream::<u64>(device, config, context),
        SampleFormat::F32 => build_typed_input_stream::<f32>(device, config, context),
        SampleFormat::F64 => build_typed_input_stream::<f64>(device, config, context),
        format => Err(MediaCaptureError::UnsupportedSampleFormat(format)),
    }
}

fn build_typed_input_stream<T>(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    mut context: AudioInputCallbackContext,
) -> Result<Stream, MediaCaptureError>
where
    T: SizedSample + Copy,
    i16: FromSample<T>,
{
    let error_stop = Arc::clone(&context.stop_requested);
    let error_event = Arc::clone(&context.capture_event);
    let error_writer = context.writer_thread.clone();
    device
        .build_input_stream(
            &config.config(),
            move |data: &[T], _| context.consume(data),
            move |_error| {
                record_capture_event(
                    &error_event,
                    CAPTURE_EVENT_DEVICE_STREAM,
                    &error_stop,
                    &error_writer,
                );
            },
            None,
        )
        .map_err(|error| MediaCaptureError::BuildStream(error.to_string()))
}

fn record_capture_event(
    capture_event: &AtomicU8,
    event: u8,
    stop_requested: &AtomicBool,
    writer_thread: &thread::Thread,
) {
    let _ = capture_event.compare_exchange(
        CAPTURE_EVENT_NONE,
        event,
        Ordering::AcqRel,
        Ordering::Acquire,
    );
    stop_requested.store(true, Ordering::Release);
    writer_thread.unpark();
}
