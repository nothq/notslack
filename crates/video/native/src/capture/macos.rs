use std::{ffi::CString, path::Path, ptr::NonNull, sync::Arc, time::Duration};

use core_video::pixel_buffer::CVPixelBuffer;

use super::{
    CaptureShared, NativeVideoCaptureError, NativeVideoCaptureMetadata, VIDEO_CAPTURE_HEIGHT,
    VIDEO_CAPTURE_WIDTH,
};

mod callback;

use callback::{
    allocate_callback_context, callback_error, error_callback, preview_callback,
    release_callback_context, CallbackContext,
};

const NATIVE_STATE_READY: i32 = 1;
const NATIVE_STATE_PREVIEWING: i32 = 2;
const NATIVE_STATE_RECORDING: i32 = 3;
const NATIVE_STATE_DURATION_LIMIT_REACHED: i32 = 4;
const NATIVE_STATE_FAILED: i32 = 5;

#[derive(Clone)]
pub struct VideoCapturePreviewSurface {
    pixel_buffer: CVPixelBuffer,
    width: u32,
    height: u32,
}

// SAFETY: AVCapture delivers immutable completed sample buffers. The bridge
// transfers a +1 retained CVPixelBuffer to this wrapper, and retaining the
// buffer prevents its pool from reusing the storage until the wrapper drops.
// CoreVideo buffers may be transferred between threads; consumers still use
// CVPixelBuffer's lock API before accessing plane storage.
unsafe impl Send for VideoCapturePreviewSurface {}

impl VideoCapturePreviewSurface {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn clone_pixel_buffer(&self) -> CVPixelBuffer {
        self.pixel_buffer.clone()
    }
}

impl std::fmt::Debug for VideoCapturePreviewSurface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoCapturePreviewSurface")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub(super) const fn capture_supported() -> bool {
    true
}

pub(super) enum PlatformCaptureState {
    Ready,
    Previewing,
    Recording,
    DurationLimitReached,
    Failed(NativeVideoCaptureError),
}

pub(super) struct PlatformCaptureStatus {
    pub(super) state: PlatformCaptureState,
    pub(super) duration: Duration,
}

pub(super) struct PlatformVideoCapture {
    session: Option<NonNull<ffi::NotslackVideoCaptureSession>>,
    callback_context: Option<NonNull<CallbackContext>>,
}

impl PlatformVideoCapture {
    pub(super) fn prepare(shared: Arc<CaptureShared>) -> Result<Self, NativeVideoCaptureError> {
        let callback_context = allocate_callback_context(shared);
        let session = NonNull::new(unsafe {
            ffi::notslack_video_capture_session_prepare(
                callback_context.as_ptr().cast(),
                preview_callback,
                error_callback,
            )
        });
        let Some(session) = session else {
            let error = callback_error(callback_context).unwrap_or_else(|| {
                NativeVideoCaptureError::Configuration(
                    "AVFoundation did not create a capture session".to_string(),
                )
            });
            release_callback_context(callback_context);
            return Err(error);
        };
        Ok(Self {
            session: Some(session),
            callback_context: Some(callback_context),
        })
    }

    pub(super) fn begin_recording(
        &mut self,
        scratch_movie_path: &Path,
    ) -> Result<(), NativeVideoCaptureError> {
        match self.status().state {
            PlatformCaptureState::Previewing => {}
            PlatformCaptureState::Ready => return Err(NativeVideoCaptureError::NotReady),
            PlatformCaptureState::Recording | PlatformCaptureState::DurationLimitReached => {
                return Err(NativeVideoCaptureError::AlreadyRecording);
            }
            PlatformCaptureState::Failed(error) => return Err(error),
        }
        let session = self.session.ok_or(NativeVideoCaptureError::WorkerStopped)?;
        let path = capture_path(scratch_movie_path, "scratch movie")?;
        let started = unsafe {
            ffi::notslack_video_capture_session_begin_recording(session.as_ptr(), path.as_ptr())
        };
        if started {
            Ok(())
        } else {
            Err(self.last_error().unwrap_or_else(|| {
                NativeVideoCaptureError::Recording(
                    "AVFoundation did not begin recording the movie file".to_string(),
                )
            }))
        }
    }

    pub(super) fn status(&self) -> PlatformCaptureStatus {
        let Some(session) = self.session else {
            return PlatformCaptureStatus {
                state: PlatformCaptureState::Failed(NativeVideoCaptureError::WorkerStopped),
                duration: Duration::ZERO,
            };
        };
        let mut duration_micros = 0_u64;
        let state = unsafe {
            ffi::notslack_video_capture_session_status(session.as_ptr(), &mut duration_micros)
        };
        let duration = Duration::from_micros(duration_micros);
        let state = match state {
            NATIVE_STATE_READY => PlatformCaptureState::Ready,
            NATIVE_STATE_PREVIEWING => PlatformCaptureState::Previewing,
            NATIVE_STATE_RECORDING => PlatformCaptureState::Recording,
            NATIVE_STATE_DURATION_LIMIT_REACHED => PlatformCaptureState::DurationLimitReached,
            NATIVE_STATE_FAILED => {
                PlatformCaptureState::Failed(self.last_error().unwrap_or_else(|| {
                    NativeVideoCaptureError::Recording(
                        "AVFoundation ended the recording without a diagnostic".to_string(),
                    )
                }))
            }
            state => PlatformCaptureState::Failed(NativeVideoCaptureError::Recording(format!(
                "AVFoundation returned unknown capture state {state}"
            ))),
        };
        PlatformCaptureStatus { state, duration }
    }

    pub(super) fn stop(
        &mut self,
        mp4_output_path: &Path,
    ) -> Result<NativeVideoCaptureMetadata, NativeVideoCaptureError> {
        match self.status().state {
            PlatformCaptureState::Recording | PlatformCaptureState::DurationLimitReached => {}
            PlatformCaptureState::Ready | PlatformCaptureState::Previewing => {
                return Err(NativeVideoCaptureError::NotRecording);
            }
            PlatformCaptureState::Failed(error) => return Err(error),
        }
        let output_path = capture_path(mp4_output_path, "MP4 output")?;
        let session = self
            .session
            .take()
            .ok_or(NativeVideoCaptureError::WorkerStopped)?;
        let mut duration_micros = 0_u64;
        let stopped = unsafe {
            ffi::notslack_video_capture_session_stop(
                session.as_ptr(),
                output_path.as_ptr(),
                &mut duration_micros,
            )
        };
        let destroyed = unsafe { ffi::notslack_video_capture_session_destroy(session.as_ptr()) };
        let error = self.last_error();
        if destroyed {
            self.release_context();
        } else {
            self.session = Some(session);
        }
        if !stopped || !destroyed {
            return Err(error.unwrap_or_else(|| {
                NativeVideoCaptureError::Finalization(
                    "AVFoundation did not finalize the MP4 output".to_string(),
                )
            }));
        }
        Ok(NativeVideoCaptureMetadata {
            duration: Duration::from_micros(duration_micros),
            width: VIDEO_CAPTURE_WIDTH,
            height: VIDEO_CAPTURE_HEIGHT,
        })
    }

    pub(super) fn cancel(&mut self) -> Result<(), NativeVideoCaptureError> {
        let Some(session) = self.session.take() else {
            return Ok(());
        };
        let cancelled = unsafe { ffi::notslack_video_capture_session_cancel(session.as_ptr()) };
        let destroyed = unsafe { ffi::notslack_video_capture_session_destroy(session.as_ptr()) };
        let error = self.last_error();
        if destroyed {
            self.release_context();
        } else {
            self.session = Some(session);
        }
        if cancelled && destroyed {
            Ok(())
        } else {
            Err(error.unwrap_or_else(|| {
                NativeVideoCaptureError::Finalization(
                    "AVFoundation did not stop the cancelled recording".to_string(),
                )
            }))
        }
    }

    fn last_error(&self) -> Option<NativeVideoCaptureError> {
        self.callback_context.and_then(callback_error)
    }

    fn release_context(&mut self) {
        if let Some(context) = self.callback_context.take() {
            release_callback_context(context);
        }
    }
}

fn capture_path(path: &Path, purpose: &str) -> Result<CString, NativeVideoCaptureError> {
    let path = path.to_str().ok_or_else(|| {
        NativeVideoCaptureError::Configuration(format!(
            "the native {purpose} path is not valid UTF-8"
        ))
    })?;
    CString::new(path).map_err(|_| {
        NativeVideoCaptureError::Configuration(format!(
            "the native {purpose} path contained an interior NUL byte"
        ))
    })
}

impl Drop for PlatformVideoCapture {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

mod ffi {
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    pub struct NotslackVideoCaptureSession {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn notslack_video_capture_session_prepare(
            context: *mut c_void,
            preview_callback: unsafe extern "C" fn(*mut c_void, *mut c_void),
            error_callback: unsafe extern "C" fn(*mut c_void, i32, *const c_char),
        ) -> *mut NotslackVideoCaptureSession;
        pub fn notslack_video_capture_session_begin_recording(
            session: *mut NotslackVideoCaptureSession,
            scratch_movie_path: *const c_char,
        ) -> bool;
        pub fn notslack_video_capture_session_status(
            session: *mut NotslackVideoCaptureSession,
            duration_micros: *mut u64,
        ) -> i32;
        pub fn notslack_video_capture_session_stop(
            session: *mut NotslackVideoCaptureSession,
            mp4_output_path: *const c_char,
            duration_micros: *mut u64,
        ) -> bool;
        pub fn notslack_video_capture_session_cancel(session: *mut NotslackVideoCaptureSession) -> bool;
        pub fn notslack_video_capture_session_destroy(session: *mut NotslackVideoCaptureSession) -> bool;
    }
}
