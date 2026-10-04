use std::path::Path;

use video_native::{FfmpegAudioDecoder, FfmpegAudioOutputConfig};

use crate::ffmpeg_player::ffmpeg_decoder_lock;

/// Loudness is measured on a narrow mono mix; a waveform needs no more detail.
const WAVEFORM_SAMPLE_RATE: u32 = 8_000;

/// The loudness of an audio file across `buckets` equal slices of its length:
/// each slice's RMS level, scaled so the loudest slice is 1.
pub fn audio_waveform(path: &Path, buckets: usize) -> Result<Vec<f32>, String> {
    assert!(buckets > 0, "an audio waveform needs at least one bucket");
    let source = path.to_string_lossy();
    let mut decoder = {
        let _guard = ffmpeg_decoder_lock();
        FfmpegAudioDecoder::open(
            &source,
            FfmpegAudioOutputConfig {
                sample_rate: WAVEFORM_SAMPLE_RATE,
                channels: 1,
            },
        )
        .map_err(|error| format!("could not open the recording audio: {error}"))?
    };
    let loudness = Loudness::measure(&mut decoder, buckets);
    {
        let _guard = ffmpeg_decoder_lock();
        drop(decoder);
    }
    Ok(loudness?.levels())
}

/// Squared samples summed per slice of the recording.
struct Loudness {
    sums: Vec<f64>,
    counts: Vec<u64>,
}

impl Loudness {
    /// Decodes the whole file, taking the decoder lock one frame at a time so
    /// a player decoding alongside is never held up for the whole file.
    fn measure(decoder: &mut FfmpegAudioDecoder, buckets: usize) -> Result<Self, String> {
        let total = decoder.duration().as_secs_f64();
        if total <= 0.0 {
            return Err("the recording audio reports no duration".to_string());
        }
        let mut loudness = Self {
            sums: vec![0.0; buckets],
            counts: vec![0; buckets],
        };
        loop {
            let frame = {
                let _guard = ffmpeg_decoder_lock();
                decoder.next_frame()
            }
            .map_err(|error| format!("could not decode the recording audio: {error}"))?;
            let Some(frame) = frame else {
                return Ok(loudness);
            };
            let start = frame.position.as_secs_f64();
            for (index, sample) in frame.samples.iter().enumerate() {
                let seconds = start + index as f64 / f64::from(WAVEFORM_SAMPLE_RATE);
                let bucket = (((seconds / total) * buckets as f64) as usize).min(buckets - 1);
                let level = f64::from(*sample) / f64::from(i16::MAX);
                loudness.sums[bucket] += level * level;
                loudness.counts[bucket] += 1;
            }
        }
    }

    fn levels(self) -> Vec<f32> {
        let levels = self
            .sums
            .iter()
            .zip(&self.counts)
            .map(|(sum, count)| {
                if *count == 0 {
                    0.0
                } else {
                    (sum / *count as f64).sqrt() as f32
                }
            })
            .collect::<Vec<_>>();
        let loudest = levels.iter().copied().fold(0.0_f32, f32::max);
        if loudest <= 0.0 {
            return levels;
        }
        levels.into_iter().map(|level| level / loudest).collect()
    }
}
