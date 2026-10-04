use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Ffmpeg(#[from] video_native::FfmpegError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid URI")]
    Uri,
    #[error("failed to sync with playback")]
    Sync,
    #[error("failed to lock internal sync primitive")]
    Lock,
    #[error("invalid playback speed: {0}")]
    PlaybackSpeed(f64),
    #[error("video decode worker stopped")]
    WorkerStopped,
}
