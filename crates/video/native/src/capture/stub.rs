use std::{path::Path, sync::Arc, time::Duration};

use super::{
    CaptureShared, NativeVideoCaptureError, NativeVideoCaptureMetadata, VIDEO_CAPTURE_HEIGHT,
    VIDEO_CAPTURE_WIDTH,
};

#[derive(Clone, Debug)]
pub struct VideoCapturePreviewSurface {
    _private: (),
}

impl VideoCapturePreviewSurface {
    pub const fn width(&self) -> u32 {
        VIDEO_CAPTURE_WIDTH
    }

    pub const fn height(&self) -> u32 {
        VIDEO_CAPTURE_HEIGHT
    }
}

pub(super) const fn capture_supported() -> bool {
    false
}

pub(super) enum PlatformCaptureState {
    Failed(NativeVideoCaptureError),
}

pub(super) struct PlatformCaptureStatus {
    pub(super) state: PlatformCaptureState,
    pub(super) duration: Duration,
}

pub(super) struct PlatformVideoCapture;

impl PlatformVideoCapture {
    pub(super) fn prepare(_shared: Arc<CaptureShared>) -> Result<Self, NativeVideoCaptureError> {
        Err(NativeVideoCaptureError::UnsupportedPlatform)
    }

    pub(super) fn begin_recording(
        &mut self,
        _scratch_movie_path: &Path,
    ) -> Result<(), NativeVideoCaptureError> {
        Err(NativeVideoCaptureError::UnsupportedPlatform)
    }

    pub(super) fn status(&self) -> PlatformCaptureStatus {
        PlatformCaptureStatus {
            state: PlatformCaptureState::Failed(NativeVideoCaptureError::UnsupportedPlatform),
            duration: Duration::ZERO,
        }
    }

    pub(super) fn stop(
        &mut self,
        _mp4_output_path: &Path,
    ) -> Result<NativeVideoCaptureMetadata, NativeVideoCaptureError> {
        Err(NativeVideoCaptureError::UnsupportedPlatform)
    }

    pub(super) fn cancel(&mut self) -> Result<(), NativeVideoCaptureError> {
        Err(NativeVideoCaptureError::UnsupportedPlatform)
    }
}
