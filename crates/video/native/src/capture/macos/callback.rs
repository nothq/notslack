use std::{
    ffi::{c_char, c_void, CStr},
    panic::{self, AssertUnwindSafe},
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use core_foundation::base::TCFType;
use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef, CVPixelBufferRelease};

use super::super::{CaptureShared, NativeVideoCaptureError, NativeVideoCaptureStatus};
use super::VideoCapturePreviewSurface;

const ERROR_CAMERA_PERMISSION_DENIED: i32 = 1;
const ERROR_MICROPHONE_PERMISSION_DENIED: i32 = 2;
const ERROR_NO_CAMERA_DEVICE: i32 = 3;
const ERROR_NO_MICROPHONE_DEVICE: i32 = 4;
const ERROR_UNSUPPORTED_CAMERA_FORMAT: i32 = 5;
const ERROR_CONFIGURATION: i32 = 6;
const ERROR_RECORDING: i32 = 7;
const ERROR_FINALIZATION: i32 = 8;

pub(super) struct CallbackContext {
    shared: Arc<CaptureShared>,
    last_error: Mutex<Option<NativeVideoCaptureError>>,
}

pub(super) fn allocate_callback_context(shared: Arc<CaptureShared>) -> NonNull<CallbackContext> {
    let context = Box::new(CallbackContext {
        shared,
        last_error: Mutex::new(None),
    });
    NonNull::new(Box::into_raw(context))
        .expect("allocating the native video callback context must yield a non-null pointer")
}

pub(super) fn callback_error(context: NonNull<CallbackContext>) -> Option<NativeVideoCaptureError> {
    unsafe { context.as_ref() }
        .last_error
        .lock()
        .expect("native video callback error mutex poisoned")
        .clone()
}

pub(super) fn release_callback_context(context: NonNull<CallbackContext>) {
    unsafe { drop(Box::from_raw(context.as_ptr())) };
}

pub(super) unsafe extern "C" fn preview_callback(context: *mut c_void, pixel_buffer: *mut c_void) {
    let dispatch = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(context) = NonNull::new(context.cast::<CallbackContext>()) else {
            release_pixel_buffer(pixel_buffer);
            return;
        };
        let pixel_buffer = pixel_buffer.cast::<core::ffi::c_void>() as CVPixelBufferRef;
        let Some(pixel_buffer) = NonNull::new(pixel_buffer) else {
            return;
        };
        let pixel_buffer = unsafe { CVPixelBuffer::wrap_under_create_rule(pixel_buffer.as_ptr()) };
        let width = match u32::try_from(pixel_buffer.get_width()) {
            Ok(width) => width,
            Err(error) => {
                record_callback_error(
                    context,
                    NativeVideoCaptureError::Recording(format!(
                        "native preview width is invalid: {error}"
                    )),
                );
                return;
            }
        };
        let height = match u32::try_from(pixel_buffer.get_height()) {
            Ok(height) => height,
            Err(error) => {
                record_callback_error(
                    context,
                    NativeVideoCaptureError::Recording(format!(
                        "native preview height is invalid: {error}"
                    )),
                );
                return;
            }
        };
        unsafe { context.as_ref() }
            .shared
            .replace_preview(VideoCapturePreviewSurface {
                pixel_buffer,
                width,
                height,
            });
    }));
    if dispatch.is_err() {
        record_callback_panic(context);
    }
}

pub(super) unsafe extern "C" fn error_callback(
    context: *mut c_void,
    code: i32,
    message: *const c_char,
) {
    let dispatch = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(context) = NonNull::new(context.cast::<CallbackContext>()) else {
            return;
        };
        let message = if message.is_null() {
            "AVFoundation capture failed".to_string()
        } else {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        record_callback_error(context, native_error(code, message));
    }));
    if dispatch.is_err() {
        record_callback_panic(context);
    }
}

fn record_callback_error(context: NonNull<CallbackContext>, error: NativeVideoCaptureError) {
    let context = unsafe { context.as_ref() };
    *context
        .last_error
        .lock()
        .expect("native video callback error mutex poisoned") = Some(error.clone());
    context
        .shared
        .set_status(NativeVideoCaptureStatus::Failed(error));
}

fn record_callback_panic(context: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<CallbackContext>()) else {
        return;
    };
    let error =
        NativeVideoCaptureError::Recording("native video capture callback panicked".to_string());
    let context = unsafe { context.as_ref() };
    if let Ok(mut last_error) = context.last_error.lock() {
        *last_error = Some(error.clone());
    }
    context
        .shared
        .try_set_status(NativeVideoCaptureStatus::Failed(error));
}

fn native_error(code: i32, message: String) -> NativeVideoCaptureError {
    match code {
        ERROR_CAMERA_PERMISSION_DENIED => NativeVideoCaptureError::CameraPermissionDenied,
        ERROR_MICROPHONE_PERMISSION_DENIED => NativeVideoCaptureError::MicrophonePermissionDenied,
        ERROR_NO_CAMERA_DEVICE => NativeVideoCaptureError::NoCameraDevice,
        ERROR_NO_MICROPHONE_DEVICE => NativeVideoCaptureError::NoMicrophoneDevice,
        ERROR_UNSUPPORTED_CAMERA_FORMAT => NativeVideoCaptureError::UnsupportedCameraFormat,
        ERROR_CONFIGURATION => NativeVideoCaptureError::Configuration(message),
        ERROR_RECORDING => NativeVideoCaptureError::Recording(message),
        ERROR_FINALIZATION => NativeVideoCaptureError::Finalization(message),
        code => NativeVideoCaptureError::Recording(format!("AVFoundation error {code}: {message}")),
    }
}

fn release_pixel_buffer(pixel_buffer: *mut c_void) {
    let pixel_buffer = pixel_buffer.cast::<core::ffi::c_void>() as CVPixelBufferRef;
    if let Some(pixel_buffer) = NonNull::new(pixel_buffer) {
        unsafe { CVPixelBufferRelease(pixel_buffer.as_ptr()) };
    }
}
