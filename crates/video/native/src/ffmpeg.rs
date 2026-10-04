mod api;
mod audio;
mod channel_layout;
mod decoder;
mod frame_data;
mod resource;

use std::{ffi::NulError, time::Duration};

use thiserror::Error;

pub use audio::{probe_audio, probe_audio_metadata, FfmpegAudioDecoder};
pub use decoder::FfmpegVideoDecoder;

pub fn validate_ffmpeg_runtime() -> Result<(), FfmpegError> {
    api::api().map(|_| ())
}

pub fn validate_ffmpeg_media(source: &str) -> Result<(), FfmpegError> {
    let mut video = FfmpegVideoDecoder::open(source)?;
    video.next_frame()?.ok_or(FfmpegError::Stream)?;

    let mut audio = FfmpegAudioDecoder::open(
        source,
        FfmpegAudioOutputConfig {
            sample_rate: 48_000,
            channels: 2,
        },
    )?;
    audio.next_frame()?.ok_or(FfmpegError::AudioStream)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum FfmpegError {
    #[error("{operation} failed: {message} ({code})")]
    FfmpegCall {
        operation: &'static str,
        code: i32,
        message: String,
    },
    #[error("failed to load FFmpeg library {library}: {error}")]
    FfmpegLibrary {
        library: &'static str,
        error: String,
    },
    #[error("failed to load FFmpeg symbol {symbol}: {error}")]
    FfmpegSymbol { symbol: &'static str, error: String },
    #[error("FFmpeg allocation failed for {0}")]
    FfmpegAllocation(&'static str),
    #[error("media source contains an interior nul byte")]
    Nul(#[from] NulError),
    #[error("failed to find a video stream")]
    Stream,
    #[error("failed to find an audio stream")]
    AudioStream,
    #[error("failed to get media capabilities")]
    Caps,
    #[error("invalid framerate: {0}")]
    Framerate(f64),
    #[error("audio resampler failed: {0}")]
    Resampler(String),
}

#[derive(Debug, Clone)]
pub struct Nv12FrameData {
    pub nv12_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub uv_row_width: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct FfmpegVideoMetadata {
    pub width: i32,
    pub height: i32,
    pub framerate: f64,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfmpegAudioMetadata {
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct FfmpegDecodedFrame {
    pub frame: Nv12FrameData,
    pub position: Duration,
    pub interval: Duration,
}

#[derive(Debug, Clone, Copy)]
pub struct FfmpegAudioOutputConfig {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone)]
pub struct FfmpegAudioFrame {
    pub samples: Vec<i16>,
    pub position: Duration,
}
