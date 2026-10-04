use super::{PreparedVideoClip, RecordingVideoClip};
use crate::transition::CaptureTransition;
use crate::{
    CapturedVideoClip, MediaCaptureError, VideoClipCaptureStatus, VideoClipFailure,
    VideoClipSessionId, VideoClipSnapshot,
};

pub(crate) enum VideoActorState {
    Prepared(PreparedVideoClip),
    Recording(RecordingVideoClip),
    Completed(CompletedVideoClip),
}

pub(crate) struct CompletedVideoClip {
    session_id: VideoClipSessionId,
    snapshot: VideoClipSnapshot,
    stop_result: Option<Result<CapturedVideoClip, MediaCaptureError>>,
    cancel_result: Option<Result<(), MediaCaptureError>>,
}

impl VideoActorState {
    pub(crate) fn prepare(session_id: VideoClipSessionId) -> Result<Self, MediaCaptureError> {
        PreparedVideoClip::prepare(session_id).map(Self::Prepared)
    }

    pub(crate) fn session_id(&self) -> VideoClipSessionId {
        match self {
            Self::Prepared(prepared) => prepared.session_id,
            Self::Recording(recording) => recording.session_id,
            Self::Completed(completed) => completed.session_id,
        }
    }

    pub(crate) const fn needs_poll(&self) -> bool {
        matches!(self, Self::Prepared(_) | Self::Recording(_))
    }

    pub(crate) fn status(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<VideoClipSnapshot, MediaCaptureError> {
        self.verify_session(session_id)?;
        Ok(match self {
            Self::Prepared(prepared) => prepared.snapshot(),
            Self::Recording(recording) => recording.snapshot(),
            Self::Completed(completed) => completed.snapshot.clone(),
        })
    }

    pub(crate) fn begin_recording(
        self,
        session_id: VideoClipSessionId,
    ) -> CaptureTransition<Self, ()> {
        if let Err(error) = self.verify_session(session_id) {
            return CaptureTransition::retained(self, error);
        }
        match self {
            Self::Prepared(prepared) => match prepared.begin_recording() {
                Ok(recording) => CaptureTransition::transitioned(Self::Recording(recording), ()),
                Err(error) => {
                    CaptureTransition::retained(Self::Prepared(error.prepared), error.error)
                }
            },
            state @ Self::Recording(_) => CaptureTransition::retained(
                state,
                MediaCaptureError::VideoClipAlreadyRecording(session_id),
            ),
            Self::Completed(completed) => {
                let error = completed.begin_recording_error();
                CaptureTransition::retained(Self::Completed(completed), error)
            }
        }
    }

    pub(crate) fn stop(
        self,
        session_id: VideoClipSessionId,
    ) -> CaptureTransition<Self, CapturedVideoClip> {
        if let Err(error) = self.verify_session(session_id) {
            return CaptureTransition::retained(self, error);
        }
        match self {
            state @ Self::Prepared(_) => CaptureTransition::retained(
                state,
                MediaCaptureError::VideoClipNotRecording(session_id),
            ),
            Self::Recording(recording) => CaptureTransition::completed(recording.finalize()),
            Self::Completed(mut completed) => CaptureTransition::completed(
                completed
                    .stop_result
                    .take()
                    .expect("a completed video clip must retain its stop result"),
            ),
        }
    }

    pub(crate) fn cancel(self, session_id: VideoClipSessionId) -> CaptureTransition<Self, ()> {
        if let Err(error) = self.verify_session(session_id) {
            return CaptureTransition::retained(self, error);
        }
        let result = match self {
            Self::Prepared(prepared) => prepared.cancel(),
            Self::Recording(recording) => recording.cancel(),
            Self::Completed(mut completed) => completed
                .cancel_result
                .take()
                .expect("a completed video clip must retain its cancel result"),
        };
        CaptureTransition::completed(result)
    }

    pub(crate) fn poll(self) -> Self {
        match self {
            Self::Prepared(prepared) => poll_prepared(prepared),
            Self::Recording(recording) => poll_recording(recording),
            Self::Completed(completed) => Self::Completed(completed),
        }
    }

    pub(crate) fn shutdown(self) {
        match self {
            Self::Prepared(prepared) => {
                let _ = prepared.cancel();
            }
            Self::Recording(recording) => {
                let _ = recording.cancel();
            }
            Self::Completed(_) => {}
        }
    }

    fn verify_session(&self, received: VideoClipSessionId) -> Result<(), MediaCaptureError> {
        let expected = self.session_id();
        if expected == received {
            Ok(())
        } else {
            Err(MediaCaptureError::VideoSessionMismatch { expected, received })
        }
    }
}

impl CompletedVideoClip {
    fn begin_recording_error(&self) -> MediaCaptureError {
        match self.snapshot.status() {
            VideoClipCaptureStatus::Failed(failure) => {
                MediaCaptureError::VideoCaptureFailed(failure.clone())
            }
            VideoClipCaptureStatus::DurationLimitReached => {
                MediaCaptureError::VideoClipAlreadyRecording(self.session_id)
            }
            VideoClipCaptureStatus::RequestingPermissions
            | VideoClipCaptureStatus::Ready
            | VideoClipCaptureStatus::Previewing
            | VideoClipCaptureStatus::Recording => {
                panic!("a completed video clip must have a terminal status")
            }
        }
    }
}

fn poll_prepared(prepared: PreparedVideoClip) -> VideoActorState {
    let snapshot = prepared.snapshot();
    let status = snapshot.status().clone();
    match status {
        VideoClipCaptureStatus::RequestingPermissions
        | VideoClipCaptureStatus::Ready
        | VideoClipCaptureStatus::Previewing => VideoActorState::Prepared(prepared),
        VideoClipCaptureStatus::Failed(failure) => {
            complete_prepared_failure(prepared, snapshot, failure)
        }
        VideoClipCaptureStatus::Recording | VideoClipCaptureStatus::DurationLimitReached => {
            panic!("a prepared video clip cannot report a recording state")
        }
    }
}

fn poll_recording(recording: RecordingVideoClip) -> VideoActorState {
    let snapshot = recording.snapshot();
    let status = snapshot.status().clone();
    match status {
        VideoClipCaptureStatus::Recording => VideoActorState::Recording(recording),
        VideoClipCaptureStatus::DurationLimitReached => {
            let session_id = recording.session_id;
            let stop_result = recording.finalize();
            VideoActorState::Completed(CompletedVideoClip {
                session_id,
                snapshot,
                stop_result: Some(stop_result),
                cancel_result: Some(Ok(())),
            })
        }
        VideoClipCaptureStatus::Failed(failure) => {
            complete_recording_failure(recording, snapshot, failure)
        }
        VideoClipCaptureStatus::RequestingPermissions
        | VideoClipCaptureStatus::Ready
        | VideoClipCaptureStatus::Previewing => {
            panic!("a recording video clip cannot return to preview preparation")
        }
    }
}

fn complete_prepared_failure(
    prepared: PreparedVideoClip,
    snapshot: VideoClipSnapshot,
    failure: VideoClipFailure,
) -> VideoActorState {
    let session_id = prepared.session_id;
    let cancel_result = prepared.cancel();
    failed_video_clip(session_id, snapshot, failure, cancel_result)
}

fn complete_recording_failure(
    recording: RecordingVideoClip,
    snapshot: VideoClipSnapshot,
    failure: VideoClipFailure,
) -> VideoActorState {
    let session_id = recording.session_id;
    let cancel_result = recording.cancel();
    failed_video_clip(session_id, snapshot, failure, cancel_result)
}

fn failed_video_clip(
    session_id: VideoClipSessionId,
    snapshot: VideoClipSnapshot,
    failure: VideoClipFailure,
    cancel_result: Result<(), MediaCaptureError>,
) -> VideoActorState {
    VideoActorState::Completed(CompletedVideoClip {
        session_id,
        snapshot,
        stop_result: Some(Err(MediaCaptureError::VideoCaptureFailed(failure))),
        cancel_result: Some(cancel_result),
    })
}
