mod error;
mod file;
mod state;

use video_native::NativeVideoCapture;

use crate::{
    CapturedVideoClip, MediaCaptureError, VideoClipCaptureStatus, VideoClipPreview,
    VideoClipSessionId, VideoClipSnapshot,
};

use error::{capture_status, media_capture_error};
pub use file::CapturedVideoFile;
use file::PendingVideoFile;
pub(crate) use state::VideoActorState;

pub(crate) struct PreparedVideoClip {
    pub(super) session_id: VideoClipSessionId,
    capture: NativeVideoCapture,
}

pub(crate) struct RecordingVideoClip {
    pub(super) session_id: VideoClipSessionId,
    file: PendingVideoFile,
    capture: NativeVideoCapture,
}

pub(super) struct BeginVideoClipError {
    pub(super) prepared: PreparedVideoClip,
    pub(super) error: MediaCaptureError,
}

impl PreparedVideoClip {
    pub(super) fn prepare(session_id: VideoClipSessionId) -> Result<Self, MediaCaptureError> {
        let capture = NativeVideoCapture::prepare()
            .map_err(|error| media_capture_error(session_id, error))?;
        Ok(Self {
            session_id,
            capture,
        })
    }

    pub(super) fn snapshot(&self) -> VideoClipSnapshot {
        capture_snapshot(&self.capture)
    }

    pub(super) fn begin_recording(self) -> Result<RecordingVideoClip, BeginVideoClipError> {
        let session_id = self.session_id;
        match self.snapshot().status() {
            VideoClipCaptureStatus::Previewing => {}
            VideoClipCaptureStatus::Failed(failure) => {
                return Err(
                    self.begin_error(MediaCaptureError::VideoCaptureFailed(failure.clone()))
                );
            }
            VideoClipCaptureStatus::Recording | VideoClipCaptureStatus::DurationLimitReached => {
                return Err(
                    self.begin_error(MediaCaptureError::VideoClipAlreadyRecording(session_id))
                );
            }
            VideoClipCaptureStatus::RequestingPermissions | VideoClipCaptureStatus::Ready => {
                return Err(self.begin_error(MediaCaptureError::VideoClipNotReady(session_id)));
            }
        }
        let file = match PendingVideoFile::create() {
            Ok(file) => file,
            Err(error) => return Err(self.begin_error(error)),
        };
        if let Err(error) = self
            .capture
            .begin_recording(file.scratch_movie_path(), file.mp4_output_path())
        {
            return Err(self.begin_error(media_capture_error(session_id, error)));
        }
        Ok(RecordingVideoClip {
            session_id,
            file,
            capture: self.capture,
        })
    }

    pub(super) fn cancel(self) -> Result<(), MediaCaptureError> {
        self.capture
            .cancel()
            .map_err(|error| media_capture_error(self.session_id, error))
    }

    fn begin_error(self, error: MediaCaptureError) -> BeginVideoClipError {
        BeginVideoClipError {
            prepared: self,
            error,
        }
    }
}

impl RecordingVideoClip {
    pub(super) fn snapshot(&self) -> VideoClipSnapshot {
        capture_snapshot(&self.capture)
    }

    pub(super) fn finalize(self) -> Result<CapturedVideoClip, MediaCaptureError> {
        let metadata = self
            .capture
            .stop()
            .map_err(|error| media_capture_error(self.session_id, error))?;
        let file = self.file.finalize()?;
        Ok(CapturedVideoClip::new(
            metadata.duration,
            metadata.width,
            metadata.height,
            file,
        ))
    }

    pub(super) fn cancel(self) -> Result<(), MediaCaptureError> {
        self.capture
            .cancel()
            .map_err(|error| media_capture_error(self.session_id, error))
    }
}

fn capture_snapshot(capture: &NativeVideoCapture) -> VideoClipSnapshot {
    let snapshot = capture.snapshot();
    VideoClipSnapshot::new(
        capture_status(snapshot.status),
        snapshot.duration,
        snapshot.latest_preview.map(VideoClipPreview::from),
    )
}
