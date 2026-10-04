use std::mem::MaybeUninit;

use rsmpeg::ffi;

use super::{
    api::{check_ffmpeg, FfmpegApi},
    FfmpegError,
};

pub(super) struct FfmpegChannelLayout {
    api: &'static FfmpegApi,
    pub(super) inner: ffi::AVChannelLayout,
}

impl FfmpegChannelLayout {
    pub(super) fn copy_from_context(
        api: &'static FfmpegApi,
        decoder: *const ffi::AVCodecContext,
    ) -> Result<Self, FfmpegError> {
        if decoder.is_null() {
            return Err(FfmpegError::Caps);
        }
        // SAFETY: FFmpeg documents `{0}` as a valid initial channel-layout state.
        let mut layout = unsafe { std::mem::zeroed::<ffi::AVChannelLayout>() };
        // SAFETY: `decoder` is a live codec context and `layout` starts initialized.
        let status = unsafe { (api.av_channel_layout_copy)(&mut layout, &(*decoder).ch_layout) };
        if let Err(error) = check_ffmpeg(api, "av_channel_layout_copy", status) {
            // SAFETY: `layout` remains an initialized FFmpeg layout after copy failure.
            unsafe {
                (api.av_channel_layout_uninit)(&mut layout);
            }
            return Err(error);
        }
        Ok(Self::new(api, layout))
    }

    pub(super) fn default_for_channels(
        api: &'static FfmpegApi,
        channels: u16,
    ) -> Result<Self, FfmpegError> {
        let mut layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        // SAFETY: FFmpeg initializes `layout` for the requested channel count.
        unsafe {
            (api.av_channel_layout_default)(layout.as_mut_ptr(), i32::from(channels));
        }
        // SAFETY: `av_channel_layout_default` initializes the layout struct.
        let layout = unsafe { layout.assume_init() };
        if layout.nb_channels <= 0 {
            return Err(FfmpegError::Caps);
        }
        Ok(Self::new(api, layout))
    }

    fn new(api: &'static FfmpegApi, inner: ffi::AVChannelLayout) -> Self {
        Self { api, inner }
    }
}

impl Drop for FfmpegChannelLayout {
    fn drop(&mut self) {
        // SAFETY: `inner` is an initialized FFmpeg channel layout owned by this wrapper.
        unsafe {
            (self.api.av_channel_layout_uninit)(&mut self.inner);
        }
    }
}
