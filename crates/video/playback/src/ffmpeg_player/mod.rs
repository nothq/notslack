pub(crate) mod audio;
mod controls;
mod element;
mod error;
#[cfg(target_os = "linux")]
mod render_image;
mod state;
mod video;
mod worker;

use std::sync::{Mutex as StdMutex, MutexGuard as StdMutexGuard, OnceLock};

pub use element::{video, VideoElement};
pub use error::Error;
pub use state::VideoFrameData;
pub use url::Url;
pub(crate) use video::probe_video_metadata;
pub use video::{Position, Video, VideoOptions};
pub(crate) use worker::VideoMetadata;

pub(crate) fn ffmpeg_decoder_lock() -> StdMutexGuard<'static, ()> {
    static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| StdMutex::new(()))
        .lock()
        .expect("FFmpeg decoder lock poisoned")
}
