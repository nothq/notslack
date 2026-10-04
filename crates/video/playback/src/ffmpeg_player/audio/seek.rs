use std::time::Duration;

use video_native::{FfmpegAudioDecoder, FfmpegAudioFrame, FfmpegAudioOutputConfig};

use super::worker::AudioWorkerInput;

pub(super) fn prefill_audio_queue(
    input: &AudioWorkerInput,
    decoder: &mut FfmpegAudioDecoder,
    position: Duration,
    target_samples: usize,
) {
    let mut sync_position = Some(position);
    while input.queue.len() < target_samples {
        match decoder.next_frame() {
            Ok(Some(frame)) => push_seek_aligned_frame(input, frame, &mut sync_position),
            Ok(None) => break,
            Err(error) => {
                *input.error.lock() = Some(format!("error decoding audio frame: {error}"));
                break;
            }
        }
    }
}

fn push_seek_aligned_frame(
    input: &AudioWorkerInput,
    frame: FfmpegAudioFrame,
    sync_position: &mut Option<Duration>,
) {
    if frame.samples.is_empty() {
        return;
    }
    let Some(position) = *sync_position else {
        input.queue.push_interleaved(&frame.samples);
        return;
    };
    let (leading_silence, samples, reached_position) =
        seek_aligned_samples(&frame, input.output, position);
    if leading_silence > 0 {
        input.queue.push_silence(leading_silence);
    }
    if !samples.is_empty() {
        input.queue.push_interleaved(samples);
    }
    if reached_position {
        *sync_position = None;
    }
}

fn seek_aligned_samples(
    frame: &FfmpegAudioFrame,
    output: FfmpegAudioOutputConfig,
    position: Duration,
) -> (usize, &[i16], bool) {
    if frame.position >= position {
        return (
            interleaved_sample_count_between(position, frame.position, output),
            &frame.samples,
            true,
        );
    }
    let skip_samples = interleaved_sample_count_between(frame.position, position, output);
    if skip_samples >= frame.samples.len() {
        return (0, &[], false);
    }
    (0, &frame.samples[skip_samples..], true)
}

fn interleaved_sample_count_between(
    start: Duration,
    end: Duration,
    output: FfmpegAudioOutputConfig,
) -> usize {
    let Some(duration) = end.checked_sub(start) else {
        return 0;
    };
    let sample_frames = duration.as_secs_f64() * f64::from(output.sample_rate);
    if !sample_frames.is_finite() || sample_frames <= 0.0 {
        return 0;
    }
    sample_frames.floor() as usize * usize::from(output.channels)
}
