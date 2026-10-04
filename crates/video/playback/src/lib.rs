pub mod ffmpeg_player;

mod audio_preview;
mod audio_waveform;
mod player;
mod render;
mod render_playlist;
mod responsive_element;
mod session;
mod source;

#[cfg(test)]
mod tests;

pub use audio_preview::{
    audio_preview_control, AudioPreviewControlStyle, AudioPreviewPlayer, AudioPreviewSnapshot,
    AudioPreviewState,
};
pub use audio_waveform::audio_waveform;
pub use player::{LoadedVideoSource, VideoPlaybackFrameState, VideoPlayer};
pub use source::{video_clock_label, VideoFrameFit, VideoPlayerConfig, VideoPlayerSource};

pub fn validate_native_runtime() -> Result<(), video_native::FfmpegError> {
    video_native::validate_ffmpeg_runtime()
}

pub fn validate_native_media(source: &str) -> Result<(), video_native::FfmpegError> {
    video_native::validate_ffmpeg_media(source)
}

#[cfg(test)]
pub(crate) use source::{prepare_video_source, video_source_kind, VideoSourceKind};
