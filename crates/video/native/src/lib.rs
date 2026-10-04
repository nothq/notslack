mod capture;
mod ffmpeg;

#[cfg(target_os = "macos")]
mod macos;

pub use capture::{
    NativeVideoCapture, NativeVideoCaptureError, NativeVideoCaptureMetadata,
    NativeVideoCapturePreview, NativeVideoCaptureSnapshot, NativeVideoCaptureStatus,
    VideoCapturePreviewGeneration, VideoCapturePreviewSurface, VIDEO_CAPTURE_FRAMES_PER_SECOND,
    VIDEO_CAPTURE_HEIGHT, VIDEO_CAPTURE_MAX_DURATION, VIDEO_CAPTURE_MIMETYPE, VIDEO_CAPTURE_WIDTH,
};
pub use ffmpeg::{
    probe_audio, probe_audio_metadata, validate_ffmpeg_media, validate_ffmpeg_runtime,
    FfmpegAudioDecoder, FfmpegAudioFrame, FfmpegAudioMetadata, FfmpegAudioOutputConfig,
    FfmpegDecodedFrame, FfmpegError, FfmpegVideoDecoder, FfmpegVideoMetadata, Nv12FrameData,
};

#[cfg(target_os = "macos")]
pub use macos::{nv12_frame_to_pixel_buffer, Nv12PixelBuffer, Nv12PixelBufferPool};
