use std::{
    num::NonZeroU64,
    path::Path,
    sync::{
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod stub;
mod worker;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(target_os = "macos"))]
use stub as platform;
use worker::{run_worker, WorkerCommand};

pub use platform::VideoCapturePreviewSurface;

pub const VIDEO_CAPTURE_WIDTH: u32 = 1280;
pub const VIDEO_CAPTURE_HEIGHT: u32 = 720;
pub const VIDEO_CAPTURE_FRAMES_PER_SECOND: u32 = 30;
pub const VIDEO_CAPTURE_MAX_DURATION: Duration = Duration::from_secs(5 * 60);
pub const VIDEO_CAPTURE_MIMETYPE: &str = "video/mp4";

const COMMAND_CAPACITY: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VideoCapturePreviewGeneration(NonZeroU64);

impl VideoCapturePreviewGeneration {
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[cfg(target_os = "macos")]
    fn next(previous: Option<Self>) -> Option<Self> {
        let value = match previous {
            Some(generation) => generation.get().checked_add(1)?,
            None => 1,
        };
        NonZeroU64::new(value).map(Self)
    }
}

#[derive(Clone, Debug)]
pub struct NativeVideoCapturePreview {
    generation: VideoCapturePreviewGeneration,
    surface: VideoCapturePreviewSurface,
}

impl NativeVideoCapturePreview {
    pub const fn generation(&self) -> VideoCapturePreviewGeneration {
        self.generation
    }

    pub const fn surface(&self) -> &VideoCapturePreviewSurface {
        &self.surface
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum NativeVideoCaptureError {
    #[error("native video capture is unsupported on this platform")]
    UnsupportedPlatform,
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
    #[error("failed to configure native video capture: {0}")]
    Configuration(String),
    #[error("native video recording failed: {0}")]
    Recording(String),
    #[error("native video finalization failed: {0}")]
    Finalization(String),
    #[error("failed to create the native video capture worker: {0}")]
    WorkerSpawn(String),
    #[error("the native video capture worker stopped unexpectedly")]
    WorkerStopped,
    #[error("native video capture is not ready to begin recording")]
    NotReady,
    #[error("native video capture is already recording")]
    AlreadyRecording,
    #[error("native video capture has not begun recording")]
    NotRecording,
    #[error("the native preview generation counter is exhausted")]
    PreviewGenerationExhausted,
}

#[derive(Clone, Debug)]
pub enum NativeVideoCaptureStatus {
    RequestingPermissions,
    Ready,
    Previewing,
    Recording,
    DurationLimitReached,
    Failed(NativeVideoCaptureError),
}

#[derive(Clone, Debug)]
pub struct NativeVideoCaptureSnapshot {
    pub status: NativeVideoCaptureStatus,
    pub duration: Duration,
    pub latest_preview: Option<NativeVideoCapturePreview>,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeVideoCaptureMetadata {
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
}

pub struct NativeVideoCapture {
    commands: SyncSender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
    shared: Arc<CaptureShared>,
}

impl NativeVideoCapture {
    pub fn prepare() -> Result<Self, NativeVideoCaptureError> {
        if !platform::capture_supported() {
            return Err(NativeVideoCaptureError::UnsupportedPlatform);
        }
        let shared = Arc::new(CaptureShared::new());
        let worker_shared = Arc::clone(&shared);
        let (commands, command_rx) = mpsc::sync_channel(COMMAND_CAPACITY);
        let worker = thread::Builder::new()
            .name("notslack-native-video-capture".to_string())
            .spawn(move || run_worker(worker_shared, command_rx))
            .map_err(|error| NativeVideoCaptureError::WorkerSpawn(error.to_string()))?;
        Ok(Self {
            commands,
            worker: Some(worker),
            shared,
        })
    }

    pub fn snapshot(&self) -> NativeVideoCaptureSnapshot {
        self.shared.snapshot()
    }

    pub fn begin_recording(
        &self,
        scratch_movie_path: &Path,
        mp4_output_path: &Path,
    ) -> Result<(), NativeVideoCaptureError> {
        let scratch_movie_path = scratch_movie_path.to_path_buf();
        let mp4_output_path = mp4_output_path.to_path_buf();
        self.request(|reply| WorkerCommand::BeginRecording {
            scratch_movie_path,
            mp4_output_path,
            reply,
        })
    }

    pub fn stop(mut self) -> Result<NativeVideoCaptureMetadata, NativeVideoCaptureError> {
        let result = self.request(WorkerCommand::Stop);
        self.join_worker();
        result
    }

    pub fn cancel(mut self) -> Result<(), NativeVideoCaptureError> {
        let result = self.request(WorkerCommand::Cancel);
        self.join_worker();
        result
    }

    fn request<Reply>(
        &self,
        command: impl FnOnce(SyncSender<Result<Reply, NativeVideoCaptureError>>) -> WorkerCommand,
    ) -> Result<Reply, NativeVideoCaptureError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(0);
        self.commands
            .send(command(reply_tx))
            .map_err(|_| NativeVideoCaptureError::WorkerStopped)?;
        reply_rx
            .recv()
            .map_err(|_| NativeVideoCaptureError::WorkerStopped)?
    }

    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for NativeVideoCapture {
    fn drop(&mut self) {
        if self.worker.is_none() {
            return;
        }
        let (reply_tx, reply_rx) = mpsc::sync_channel(0);
        if self.commands.send(WorkerCommand::Cancel(reply_tx)).is_ok() {
            let _ = reply_rx.recv();
        }
        self.join_worker();
    }
}

struct CaptureShared {
    state: Mutex<CaptureSharedState>,
}

struct CaptureSharedState {
    status: NativeVideoCaptureStatus,
    duration: Duration,
    latest_preview: Option<NativeVideoCapturePreview>,
}

impl CaptureShared {
    fn new() -> Self {
        Self {
            state: Mutex::new(CaptureSharedState {
                status: NativeVideoCaptureStatus::RequestingPermissions,
                duration: Duration::ZERO,
                latest_preview: None,
            }),
        }
    }

    fn snapshot(&self) -> NativeVideoCaptureSnapshot {
        let state = self
            .state
            .lock()
            .expect("video capture state mutex poisoned");
        NativeVideoCaptureSnapshot {
            status: state.status.clone(),
            duration: state.duration,
            latest_preview: state.latest_preview.clone(),
        }
    }

    fn set_status(&self, status: NativeVideoCaptureStatus) {
        self.state
            .lock()
            .expect("video capture state mutex poisoned")
            .status = status;
    }

    #[cfg(target_os = "macos")]
    fn try_set_status(&self, status: NativeVideoCaptureStatus) {
        if let Ok(mut state) = self.state.lock() {
            state.status = status;
        }
    }

    fn require_previewing(&self) -> Result<(), NativeVideoCaptureError> {
        let state = self
            .state
            .lock()
            .expect("video capture state mutex poisoned");
        match &state.status {
            NativeVideoCaptureStatus::Previewing => Ok(()),
            NativeVideoCaptureStatus::RequestingPermissions | NativeVideoCaptureStatus::Ready => {
                Err(NativeVideoCaptureError::NotReady)
            }
            NativeVideoCaptureStatus::Recording
            | NativeVideoCaptureStatus::DurationLimitReached => {
                Err(NativeVideoCaptureError::AlreadyRecording)
            }
            NativeVideoCaptureStatus::Failed(error) => Err(error.clone()),
        }
    }

    fn update_native_status(&self, status: platform::PlatformCaptureStatus) {
        let mut state = self
            .state
            .lock()
            .expect("video capture state mutex poisoned");
        if matches!(state.status, NativeVideoCaptureStatus::Failed(_)) {
            return;
        }
        state.duration = status.duration;
        state.status = match status.state {
            #[cfg(target_os = "macos")]
            platform::PlatformCaptureState::Ready => NativeVideoCaptureStatus::Ready,
            #[cfg(target_os = "macos")]
            platform::PlatformCaptureState::Previewing => NativeVideoCaptureStatus::Previewing,
            #[cfg(target_os = "macos")]
            platform::PlatformCaptureState::Recording => NativeVideoCaptureStatus::Recording,
            #[cfg(target_os = "macos")]
            platform::PlatformCaptureState::DurationLimitReached => {
                NativeVideoCaptureStatus::DurationLimitReached
            }
            platform::PlatformCaptureState::Failed(error) => {
                NativeVideoCaptureStatus::Failed(error)
            }
        };
    }

    #[cfg(target_os = "macos")]
    fn replace_preview(&self, surface: VideoCapturePreviewSurface) {
        let mut state = self
            .state
            .lock()
            .expect("video capture state mutex poisoned");
        let previous = state
            .latest_preview
            .as_ref()
            .map(NativeVideoCapturePreview::generation);
        let Some(generation) = VideoCapturePreviewGeneration::next(previous) else {
            state.status = NativeVideoCaptureStatus::Failed(
                NativeVideoCaptureError::PreviewGenerationExhausted,
            );
            return;
        };
        state.latest_preview = Some(NativeVideoCapturePreview {
            generation,
            surface,
        });
        if matches!(state.status, NativeVideoCaptureStatus::Ready) {
            state.status = NativeVideoCaptureStatus::Previewing;
        }
    }
}
