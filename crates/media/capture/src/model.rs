use std::{num::NonZeroU64, sync::Arc, time::Duration};

use cpal::SampleFormat;
use video_native::{
    NativeVideoCapturePreview, VideoCapturePreviewGeneration, VideoCapturePreviewSurface,
};

use crate::CapturedVideoFile;

pub const AUDIO_CLIP_MIMETYPE: &str = "audio/wav";
pub const MAX_AUDIO_CLIP_DURATION: Duration = Duration::from_secs(5 * 60);
pub const VIDEO_CLIP_MIMETYPE: &str = video_native::VIDEO_CAPTURE_MIMETYPE;
pub const MAX_VIDEO_CLIP_DURATION: Duration = video_native::VIDEO_CAPTURE_MAX_DURATION;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AudioClipSessionId(NonZeroU64);

impl AudioClipSessionId {
    pub(crate) const fn from_raw(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub(crate) const fn raw(self) -> NonZeroU64 {
        self.0
    }
}

impl std::fmt::Display for AudioClipSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "audio-clip-session-{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VideoClipSessionId(NonZeroU64);

impl VideoClipSessionId {
    pub(crate) const fn from_raw(value: NonZeroU64) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for VideoClipSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "video-clip-session-{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaCaptureCapabilities {
    pub audio_clip: bool,
    pub video_clip: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioClipFailure {
    BufferOverflow,
    DeviceStream,
    Writer,
}

impl std::fmt::Display for AudioClipFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::BufferOverflow => "microphone input exceeded the bounded audio buffer",
            Self::DeviceStream => "the microphone input stream stopped unexpectedly",
            Self::Writer => "the audio clip could not be written",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioClipCaptureStatus {
    Recording,
    DurationLimitReached,
    Failed(AudioClipFailure),
}

#[derive(Clone, Debug)]
pub struct CapturedAudioClip {
    pub filename: String,
    pub mimetype: &'static str,
    pub duration: Duration,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum VideoClipFailure {
    #[error("camera access was denied")]
    CameraPermissionDenied,
    #[error("microphone access was denied")]
    MicrophonePermissionDenied,
    #[error("no camera device is available")]
    NoCameraDevice,
    #[error("no microphone device is available")]
    NoMicrophoneDevice,
    #[error("the camera does not support 1280x720 video at 30 fps")]
    UnsupportedCameraFormat,
    #[error("video capture configuration failed: {0}")]
    Configuration(String),
    #[error("video recording failed: {0}")]
    Recording(String),
    #[error("video finalization failed: {0}")]
    Finalization(String),
}

#[derive(Clone, Debug)]
pub enum VideoClipCaptureStatus {
    RequestingPermissions,
    Ready,
    Previewing,
    Recording,
    DurationLimitReached,
    Failed(VideoClipFailure),
}

#[derive(Clone, Debug)]
pub struct VideoClipPreview {
    generation: VideoCapturePreviewGeneration,
    surface: VideoCapturePreviewSurface,
}

impl VideoClipPreview {
    pub const fn generation(&self) -> VideoCapturePreviewGeneration {
        self.generation
    }

    pub const fn surface(&self) -> &VideoCapturePreviewSurface {
        &self.surface
    }
}

impl From<NativeVideoCapturePreview> for VideoClipPreview {
    fn from(preview: NativeVideoCapturePreview) -> Self {
        Self {
            generation: preview.generation(),
            surface: preview.surface().clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VideoClipSnapshot {
    status: VideoClipCaptureStatus,
    duration: Duration,
    latest_preview: Option<VideoClipPreview>,
}

impl VideoClipSnapshot {
    pub(crate) fn new(
        status: VideoClipCaptureStatus,
        duration: Duration,
        latest_preview: Option<VideoClipPreview>,
    ) -> Self {
        Self {
            status,
            duration,
            latest_preview,
        }
    }

    pub const fn status(&self) -> &VideoClipCaptureStatus {
        &self.status
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }

    pub const fn latest_preview(&self) -> Option<&VideoClipPreview> {
        self.latest_preview.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct CapturedVideoClip {
    duration: Duration,
    width: u32,
    height: u32,
    file: CapturedVideoFile,
}

impl CapturedVideoClip {
    pub(crate) fn new(
        duration: Duration,
        width: u32,
        height: u32,
        file: CapturedVideoFile,
    ) -> Self {
        Self {
            duration,
            width,
            height,
            file,
        }
    }

    pub const fn mimetype(&self) -> &'static str {
        VIDEO_CLIP_MIMETYPE
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn file(&self) -> &CapturedVideoFile {
        &self.file
    }

    pub fn into_file(self) -> CapturedVideoFile {
        self.file
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MediaCaptureError {
    #[error("an audio clip recording is already active ({0})")]
    SessionAlreadyActive(AudioClipSessionId),
    #[error("a video clip capture session is already active ({0})")]
    VideoSessionAlreadyActive(VideoClipSessionId),
    #[error("audio capture {0} prevents starting or controlling video capture")]
    AudioSessionActive(AudioClipSessionId),
    #[error("video capture {0} prevents starting or controlling audio capture")]
    VideoSessionActive(VideoClipSessionId),
    #[error("no audio clip recording is active")]
    NoActiveSession,
    #[error("no video clip capture session is active")]
    NoActiveVideoSession,
    #[error("audio clip session mismatch: expected {expected}, received {received}")]
    SessionMismatch {
        expected: AudioClipSessionId,
        received: AudioClipSessionId,
    },
    #[error("video clip session mismatch: expected {expected}, received {received}")]
    VideoSessionMismatch {
        expected: VideoClipSessionId,
        received: VideoClipSessionId,
    },
    #[error("video clip {0} is not ready to begin recording")]
    VideoClipNotReady(VideoClipSessionId),
    #[error("video clip {0} is already recording")]
    VideoClipAlreadyRecording(VideoClipSessionId),
    #[error("video clip {0} has not begun recording")]
    VideoClipNotRecording(VideoClipSessionId),
    #[error("media capture session ids are exhausted")]
    SessionIdExhausted,
    #[error("no microphone input device is available")]
    NoInputDevice,
    #[error("failed to read the microphone input configuration: {0}")]
    InputConfiguration(String),
    #[error("unsupported microphone input sample format: {0:?}")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("failed to create the audio clip output: {0}")]
    Output(String),
    #[error("failed to build the microphone input stream: {0}")]
    BuildStream(String),
    #[error("failed to start the microphone input stream: {0}")]
    StartStream(String),
    #[error("audio clip capture failed: {0}")]
    CaptureFailed(AudioClipFailure),
    #[error("video clip capture failed: {0}")]
    VideoCaptureFailed(VideoClipFailure),
    #[error("native video capture is unsupported on this platform")]
    VideoCaptureUnsupported,
    #[error("the audio clip writer thread stopped unexpectedly")]
    WriterThreadStopped,
    #[error("the media capture actor stopped unexpectedly")]
    ActorStopped,
    #[error("failed to spawn the media capture actor: {0}")]
    ActorSpawn(String),
}

pub trait MediaCaptureService: Send + Sync {
    fn capabilities(&self) -> MediaCaptureCapabilities;
    fn start_audio_clip(&self) -> Result<AudioClipSessionId, MediaCaptureError>;
    fn audio_clip_status(
        &self,
        session_id: AudioClipSessionId,
    ) -> Result<AudioClipCaptureStatus, MediaCaptureError>;
    fn stop_audio_clip(
        &self,
        session_id: AudioClipSessionId,
    ) -> Result<CapturedAudioClip, MediaCaptureError>;
    fn cancel_audio_clip(&self, session_id: AudioClipSessionId) -> Result<(), MediaCaptureError>;
    fn prepare_video_clip(&self) -> Result<VideoClipSessionId, MediaCaptureError>;
    fn begin_video_clip_recording(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<(), MediaCaptureError>;
    fn video_clip_status(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<VideoClipSnapshot, MediaCaptureError>;
    fn stop_video_clip(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<CapturedVideoClip, MediaCaptureError>;
    fn cancel_video_clip(&self, session_id: VideoClipSessionId) -> Result<(), MediaCaptureError>;
}
