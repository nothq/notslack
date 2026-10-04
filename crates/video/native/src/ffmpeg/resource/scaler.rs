use std::{ffi::c_int, ptr};

use rsmpeg::ffi;

use crate::ffmpeg::{api::FfmpegApi, FfmpegError};

pub(in crate::ffmpeg) struct FfmpegScaler {
    api: &'static FfmpegApi,
    ptr: *mut ffi::SwsContext,
    config: FfmpegScalerConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ffmpeg) struct FfmpegScalerConfig {
    pub(in crate::ffmpeg) src_width: i32,
    pub(in crate::ffmpeg) src_height: i32,
    pub(in crate::ffmpeg) pix_fmt: ffi::AVPixelFormat,
    pub(in crate::ffmpeg) dst_width: i32,
    pub(in crate::ffmpeg) dst_height: i32,
}

impl FfmpegScaler {
    pub(in crate::ffmpeg) fn new(
        api: &'static FfmpegApi,
        config: FfmpegScalerConfig,
    ) -> Result<Self, FfmpegError> {
        if config.src_width <= 0
            || config.src_height <= 0
            || config.dst_width <= 0
            || config.dst_height <= 0
        {
            return Err(FfmpegError::Caps);
        }
        // SAFETY: Dimensions and pixel formats are validated before creating the scaling context.
        let ptr = unsafe {
            (api.sws_get_context)(
                config.src_width,
                config.src_height,
                config.pix_fmt,
                config.dst_width,
                config.dst_height,
                ffi::AV_PIX_FMT_NV12,
                ffi::SWS_BILINEAR as c_int,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if ptr.is_null() {
            return Err(FfmpegError::Caps);
        }
        Ok(Self { api, ptr, config })
    }

    pub(in crate::ffmpeg) fn matches(&self, config: FfmpegScalerConfig) -> bool {
        self.config == config
    }

    pub(in crate::ffmpeg) fn scale(
        &self,
        decoded: *const ffi::AVFrame,
        nv12: *mut ffi::AVFrame,
    ) -> Result<(), FfmpegError> {
        // SAFETY: Both frames are live FFmpeg frames and swscale reads only the data/linesize arrays.
        let src_data = unsafe {
            [
                (*decoded).data[0].cast_const(),
                (*decoded).data[1].cast_const(),
                (*decoded).data[2].cast_const(),
                (*decoded).data[3].cast_const(),
                (*decoded).data[4].cast_const(),
                (*decoded).data[5].cast_const(),
                (*decoded).data[6].cast_const(),
                (*decoded).data[7].cast_const(),
            ]
        };
        // SAFETY: Both frames are live FFmpeg frames.
        let src_linesize = unsafe { (*decoded).linesize };
        // SAFETY: Both frames are live FFmpeg frames.
        let dst_data = unsafe { (*nv12).data };
        // SAFETY: Both frames are live FFmpeg frames.
        let dst_linesize = unsafe { (*nv12).linesize };
        // SAFETY: Frame planes, strides, and scaler context are owned by this decoder.
        let scaled = unsafe {
            (self.api.sws_scale)(
                self.ptr,
                src_data.as_ptr(),
                src_linesize.as_ptr(),
                0,
                self.config.src_height,
                dst_data.as_ptr(),
                dst_linesize.as_ptr(),
            )
        };
        if scaled <= 0 {
            return Err(FfmpegError::Caps);
        }
        Ok(())
    }
}

impl Drop for FfmpegScaler {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: `ptr` was allocated by swscale and is freed exactly once here.
        unsafe {
            (self.api.sws_free_context)(self.ptr);
        }
    }
}
