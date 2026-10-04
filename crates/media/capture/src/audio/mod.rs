mod input;
mod output;
mod session;
mod state;
mod writer;

use std::time::Duration;

use crate::AudioClipFailure;

pub(crate) use session::ActiveAudioClip;
pub(crate) use state::AudioActorState;

pub(super) const WRITER_IDLE_WAIT: Duration = Duration::from_millis(5);

pub(super) const CAPTURE_EVENT_NONE: u8 = 0;
pub(super) const CAPTURE_EVENT_BUFFER_OVERFLOW: u8 = 1;
pub(super) const CAPTURE_EVENT_DEVICE_STREAM: u8 = 2;
pub(super) const CAPTURE_EVENT_WRITER: u8 = 3;
pub(super) const CAPTURE_EVENT_DURATION_LIMIT: u8 = 4;

pub(super) fn audio_clip_failure(event: u8) -> AudioClipFailure {
    match event {
        CAPTURE_EVENT_BUFFER_OVERFLOW => AudioClipFailure::BufferOverflow,
        CAPTURE_EVENT_DEVICE_STREAM => AudioClipFailure::DeviceStream,
        CAPTURE_EVENT_WRITER => AudioClipFailure::Writer,
        event => panic!("unknown audio clip capture event {event}"),
    }
}
