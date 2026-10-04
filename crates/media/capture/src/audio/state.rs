use super::ActiveAudioClip;
use crate::transition::CaptureTransition;
use crate::{AudioClipCaptureStatus, AudioClipSessionId, CapturedAudioClip, MediaCaptureError};

pub(crate) enum AudioActorState {
    Recording(ActiveAudioClip),
    Completed(CompletedAudioClip),
}

pub(crate) struct CompletedAudioClip {
    session_id: AudioClipSessionId,
    status: AudioClipCaptureStatus,
    stop_result: Option<Result<CapturedAudioClip, MediaCaptureError>>,
    cancel_result: Option<Result<(), MediaCaptureError>>,
}

impl AudioActorState {
    pub(crate) fn start(session_id: AudioClipSessionId) -> Result<Self, MediaCaptureError> {
        ActiveAudioClip::start(session_id).map(Self::Recording)
    }

    pub(crate) fn session_id(&self) -> AudioClipSessionId {
        match self {
            Self::Recording(active) => active.session_id,
            Self::Completed(completed) => completed.session_id,
        }
    }

    pub(crate) const fn needs_poll(&self) -> bool {
        matches!(self, Self::Recording(_))
    }

    pub(crate) fn status(
        &self,
        session_id: AudioClipSessionId,
    ) -> Result<AudioClipCaptureStatus, MediaCaptureError> {
        let expected = self.session_id();
        if expected != session_id {
            return Err(MediaCaptureError::SessionMismatch {
                expected,
                received: session_id,
            });
        }
        match self {
            Self::Recording(active) => Ok(active.status()),
            Self::Completed(completed) => Ok(completed.status),
        }
    }

    pub(crate) fn stop(
        self,
        session_id: AudioClipSessionId,
    ) -> CaptureTransition<Self, CapturedAudioClip> {
        let expected = self.session_id();
        if expected != session_id {
            return CaptureTransition::retained(
                self,
                MediaCaptureError::SessionMismatch {
                    expected,
                    received: session_id,
                },
            );
        }
        let result = match self {
            Self::Completed(mut completed) => completed
                .stop_result
                .take()
                .expect("a completed audio clip must retain its stop result"),
            Self::Recording(active) => active.finalize(),
        };
        CaptureTransition::completed(result)
    }

    pub(crate) fn cancel(self, session_id: AudioClipSessionId) -> CaptureTransition<Self, ()> {
        let expected = self.session_id();
        if expected != session_id {
            return CaptureTransition::retained(
                self,
                MediaCaptureError::SessionMismatch {
                    expected,
                    received: session_id,
                },
            );
        }
        let result = match self {
            Self::Completed(mut completed) => completed
                .cancel_result
                .take()
                .expect("a completed audio clip must retain its cancel result"),
            Self::Recording(active) => active.cancel(),
        };
        CaptureTransition::completed(result)
    }

    pub(crate) fn poll(self) -> Self {
        let Self::Recording(active) = self else {
            return self;
        };
        match active.status() {
            AudioClipCaptureStatus::Recording => Self::Recording(active),
            AudioClipCaptureStatus::DurationLimitReached => {
                let session_id = active.session_id;
                let stop_result = active.finalize();
                Self::Completed(CompletedAudioClip {
                    session_id,
                    status: AudioClipCaptureStatus::DurationLimitReached,
                    stop_result: Some(stop_result),
                    cancel_result: Some(Ok(())),
                })
            }
            AudioClipCaptureStatus::Failed(failure) => {
                let session_id = active.session_id;
                let cancel_result = active.cancel();
                Self::Completed(CompletedAudioClip {
                    session_id,
                    status: AudioClipCaptureStatus::Failed(failure),
                    stop_result: Some(Err(MediaCaptureError::CaptureFailed(failure))),
                    cancel_result: Some(cancel_result),
                })
            }
        }
    }

    pub(crate) fn shutdown(self) {
        if let Self::Recording(active) = self {
            let _ = active.cancel();
        }
    }
}
