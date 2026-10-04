use std::path::PathBuf;

use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use tempfile::TempDir;

use crate::{AudioClipSessionId, MediaCaptureError};

pub(super) type AudioOutputWriter = WavWriter<std::io::BufWriter<std::fs::File>>;

pub(super) struct AudioOutput {
    pub(super) temp_dir: TempDir,
    pub(super) path: PathBuf,
    pub(super) writer: AudioOutputWriter,
}

pub(super) fn create_audio_output(
    session_id: AudioClipSessionId,
    sample_rate: u32,
) -> Result<AudioOutput, MediaCaptureError> {
    let temp_dir = tempfile::Builder::new()
        .prefix("notslack-audio-clip-")
        .tempdir()
        .map_err(|error| MediaCaptureError::Output(error.to_string()))?;
    let path = temp_dir
        .path()
        .join(format!("audio-clip-{}.wav", session_id.raw()));
    let writer = WavWriter::create(
        &path,
        WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: WavSampleFormat::Int,
        },
    )
    .map_err(|error| MediaCaptureError::Output(error.to_string()))?;
    Ok(AudioOutput {
        temp_dir,
        path,
        writer,
    })
}
