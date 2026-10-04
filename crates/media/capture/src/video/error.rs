use video_native::{NativeVideoCaptureError, NativeVideoCaptureStatus};

use crate::{MediaCaptureError, VideoClipCaptureStatus, VideoClipFailure, VideoClipSessionId};

pub(super) fn capture_status(status: NativeVideoCaptureStatus) -> VideoClipCaptureStatus {
    match status {
        NativeVideoCaptureStatus::RequestingPermissions => {
            VideoClipCaptureStatus::RequestingPermissions
        }
        NativeVideoCaptureStatus::Ready => VideoClipCaptureStatus::Ready,
        NativeVideoCaptureStatus::Previewing => VideoClipCaptureStatus::Previewing,
        NativeVideoCaptureStatus::Recording => VideoClipCaptureStatus::Recording,
        NativeVideoCaptureStatus::DurationLimitReached => {
            VideoClipCaptureStatus::DurationLimitReached
        }
        NativeVideoCaptureStatus::Failed(error) => {
            VideoClipCaptureStatus::Failed(video_failure(error))
        }
    }
}

pub(super) fn media_capture_error(
    session_id: VideoClipSessionId,
    error: NativeVideoCaptureError,
) -> MediaCaptureError {
    match error {
        NativeVideoCaptureError::UnsupportedPlatform => MediaCaptureError::VideoCaptureUnsupported,
        NativeVideoCaptureError::NotReady => MediaCaptureError::VideoClipNotReady(session_id),
        NativeVideoCaptureError::AlreadyRecording => {
            MediaCaptureError::VideoClipAlreadyRecording(session_id)
        }
        NativeVideoCaptureError::NotRecording => {
            MediaCaptureError::VideoClipNotRecording(session_id)
        }
        error => MediaCaptureError::VideoCaptureFailed(video_failure(error)),
    }
}

fn video_failure(error: NativeVideoCaptureError) -> VideoClipFailure {
    match error {
        NativeVideoCaptureError::CameraPermissionDenied => VideoClipFailure::CameraPermissionDenied,
        NativeVideoCaptureError::MicrophonePermissionDenied => {
            VideoClipFailure::MicrophonePermissionDenied
        }
        NativeVideoCaptureError::NoCameraDevice => VideoClipFailure::NoCameraDevice,
        NativeVideoCaptureError::NoMicrophoneDevice => VideoClipFailure::NoMicrophoneDevice,
        NativeVideoCaptureError::UnsupportedCameraFormat => {
            VideoClipFailure::UnsupportedCameraFormat
        }
        NativeVideoCaptureError::Configuration(message)
        | NativeVideoCaptureError::WorkerSpawn(message) => VideoClipFailure::Configuration(message),
        NativeVideoCaptureError::Recording(message) => VideoClipFailure::Recording(message),
        NativeVideoCaptureError::Finalization(message) => VideoClipFailure::Finalization(message),
        NativeVideoCaptureError::WorkerStopped => VideoClipFailure::Recording(
            "the native video capture worker stopped unexpectedly".to_string(),
        ),
        NativeVideoCaptureError::PreviewGenerationExhausted => VideoClipFailure::Recording(
            "the native preview generation counter is exhausted".to_string(),
        ),
        NativeVideoCaptureError::UnsupportedPlatform
        | NativeVideoCaptureError::NotReady
        | NativeVideoCaptureError::AlreadyRecording
        | NativeVideoCaptureError::NotRecording => {
            unreachable!("lifecycle and platform errors are mapped before video failures")
        }
    }
}
