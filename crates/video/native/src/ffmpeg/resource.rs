use std::{
    ffi::{c_int, CString},
    ptr,
};

use rsmpeg::ffi;

mod scaler;
mod stream;

pub(super) use scaler::{FfmpegScaler, FfmpegScalerConfig};
pub(super) use stream::{find_audio_stream, find_video_stream};

use super::{
    api::{check_ffmpeg, FfmpegApi},
    channel_layout::FfmpegChannelLayout,
    FfmpegError,
};

pub(super) struct FfmpegInput {
    api: &'static FfmpegApi,
    pub(super) ptr: *mut ffi::AVFormatContext,
}

impl FfmpegInput {
    pub(super) fn open(api: &'static FfmpegApi, source: &str) -> Result<Self, FfmpegError> {
        let source = CString::new(source)?;
        let mut input = ptr::null_mut();
        // SAFETY: `source` is a valid C string and FFmpeg initializes `input` on success.
        let status = unsafe {
            (api.avformat_open_input)(&mut input, source.as_ptr(), ptr::null(), ptr::null_mut())
        };
        check_ffmpeg(api, "avformat_open_input", status)?;
        Ok(Self { api, ptr: input })
    }

    pub(super) fn find_stream_info(&self) -> Result<(), FfmpegError> {
        // SAFETY: `self.ptr` is a valid format context after `avformat_open_input`.
        let status = unsafe { (self.api.avformat_find_stream_info)(self.ptr, ptr::null_mut()) };
        check_ffmpeg(self.api, "avformat_find_stream_info", status)
    }
    pub(super) fn stream(&self, stream_index: usize) -> Result<*mut ffi::AVStream, FfmpegError> {
        // SAFETY: `self.ptr` is a valid open format context.
        let stream_count = unsafe { (*self.ptr).nb_streams as usize };
        if stream_index >= stream_count {
            return Err(FfmpegError::Stream);
        }
        // SAFETY: `self.ptr` is a valid open format context.
        let streams = unsafe { (*self.ptr).streams };
        if streams.is_null() {
            return Err(FfmpegError::Stream);
        }
        // SAFETY: bounds were checked against `nb_streams`.
        let stream = unsafe { *streams.add(stream_index) };
        if stream.is_null() {
            return Err(FfmpegError::Stream);
        }
        Ok(stream)
    }
    pub(super) fn discard_other_streams(
        &self,
        keep_stream_index: usize,
    ) -> Result<(), FfmpegError> {
        // SAFETY: `self.ptr` is a valid open format context.
        let stream_count = unsafe { (*self.ptr).nb_streams as usize };
        if keep_stream_index >= stream_count {
            return Err(FfmpegError::Stream);
        }
        // SAFETY: `self.ptr` is a valid open format context.
        let streams = unsafe { (*self.ptr).streams };
        if streams.is_null() {
            return Err(FfmpegError::Stream);
        }
        for stream_index in 0..stream_count {
            // SAFETY: bounds are checked against `nb_streams`.
            let stream = unsafe { *streams.add(stream_index) };
            if stream.is_null() {
                return Err(FfmpegError::Stream);
            }
            // SAFETY: `stream` is a live stream pointer owned by the format context.
            unsafe {
                (*stream).discard = if stream_index == keep_stream_index {
                    ffi::AVDISCARD_DEFAULT
                } else {
                    ffi::AVDISCARD_ALL
                };
            }
        }
        Ok(())
    }
}

impl Drop for FfmpegInput {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: `ptr` was opened by FFmpeg and is closed exactly once here.
        unsafe {
            (self.api.avformat_close_input)(&mut self.ptr);
        }
    }
}

pub(super) struct FfmpegCodecContext {
    api: &'static FfmpegApi,
    pub(super) ptr: *mut ffi::AVCodecContext,
}

impl FfmpegCodecContext {
    pub(super) fn open(
        api: &'static FfmpegApi,
        stream: *mut ffi::AVStream,
        codec: *const ffi::AVCodec,
    ) -> Result<Self, FfmpegError> {
        // SAFETY: `stream` was validated before constructing the decoder.
        let codecpar = unsafe { (*stream).codecpar };
        if codecpar.is_null() {
            return Err(FfmpegError::Caps);
        }
        if codec.is_null() {
            return Err(FfmpegError::Stream);
        }
        // SAFETY: `codec` is returned by FFmpeg stream discovery.
        let ptr = unsafe { (api.avcodec_alloc_context3)(codec) };
        if ptr.is_null() {
            return Err(FfmpegError::FfmpegAllocation("AVCodecContext"));
        }
        let decoder = Self { api, ptr };
        // SAFETY: `decoder.ptr` and `codecpar` are valid FFmpeg objects.
        let status = unsafe { (api.avcodec_parameters_to_context)(decoder.ptr, codecpar) };
        check_ffmpeg(api, "avcodec_parameters_to_context", status)?;
        // SAFETY: `decoder.ptr` and `stream` are valid FFmpeg objects.
        unsafe {
            (*decoder.ptr).pkt_timebase = (*stream).time_base;
            (*decoder.ptr).thread_count = 1;
            (*decoder.ptr).thread_type = 0;
        }
        // SAFETY: `decoder.ptr` and `codec` are valid FFmpeg objects.
        let status = unsafe { (api.avcodec_open2)(decoder.ptr, codec, ptr::null_mut()) };
        check_ffmpeg(api, "avcodec_open2", status)?;
        Ok(decoder)
    }

    pub(super) fn width(&self) -> i32 {
        // SAFETY: `ptr` is a live codec context.
        unsafe { (*self.ptr).width }
    }

    pub(super) fn height(&self) -> i32 {
        // SAFETY: `ptr` is a live codec context.
        unsafe { (*self.ptr).height }
    }
    pub(super) fn sample_rate(&self) -> i32 {
        // SAFETY: `ptr` is a live codec context.
        unsafe { (*self.ptr).sample_rate }
    }

    pub(super) fn sample_format(&self) -> ffi::AVSampleFormat {
        // SAFETY: `ptr` is a live codec context.
        unsafe { (*self.ptr).sample_fmt }
    }

    pub(super) fn channel_layout(&self) -> Result<FfmpegChannelLayout, FfmpegError> {
        FfmpegChannelLayout::copy_from_context(self.api, self.ptr)
    }
}

impl Drop for FfmpegCodecContext {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: `ptr` was allocated by FFmpeg and is freed exactly once here.
        unsafe {
            (self.api.avcodec_free_context)(&mut self.ptr);
        }
    }
}

pub(super) struct FfmpegFrame {
    api: &'static FfmpegApi,
    pub(super) ptr: *mut ffi::AVFrame,
}

impl FfmpegFrame {
    pub(super) fn new(api: &'static FfmpegApi) -> Result<Self, FfmpegError> {
        // SAFETY: FFmpeg allocates and returns an owned frame pointer.
        let ptr = unsafe { (api.av_frame_alloc)() };
        if ptr.is_null() {
            return Err(FfmpegError::FfmpegAllocation("AVFrame"));
        }
        Ok(Self { api, ptr })
    }

    pub(super) fn configure_nv12(&self, width: i32, height: i32) {
        // SAFETY: `ptr` is a live frame and only metadata fields are set before buffer allocation.
        unsafe {
            (*self.ptr).format = ffi::AV_PIX_FMT_NV12 as c_int;
            (*self.ptr).width = width;
            (*self.ptr).height = height;
        }
    }

    pub(super) fn unref(&self) {
        // SAFETY: `ptr` is a live frame and may be unreferenced after frame data is consumed.
        unsafe {
            (self.api.av_frame_unref)(self.ptr);
        }
    }
}

impl Drop for FfmpegFrame {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: `ptr` was allocated by FFmpeg and is freed exactly once here.
        unsafe {
            (self.api.av_frame_free)(&mut self.ptr);
        }
    }
}

pub(super) struct FfmpegPacket {
    api: &'static FfmpegApi,
    pub(super) ptr: *mut ffi::AVPacket,
}

impl FfmpegPacket {
    pub(super) fn new(api: &'static FfmpegApi) -> Result<Self, FfmpegError> {
        // SAFETY: FFmpeg allocates and returns an owned packet pointer.
        let ptr = unsafe { (api.av_packet_alloc)() };
        if ptr.is_null() {
            return Err(FfmpegError::FfmpegAllocation("AVPacket"));
        }
        Ok(Self { api, ptr })
    }

    pub(super) fn stream_index(&self) -> c_int {
        // SAFETY: `ptr` is a live packet.
        unsafe { (*self.ptr).stream_index }
    }

    pub(super) fn unref(&self) {
        // SAFETY: `ptr` is a live packet and may be unreferenced repeatedly after reads.
        unsafe {
            (self.api.av_packet_unref)(self.ptr);
        }
    }
}

impl Drop for FfmpegPacket {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: `ptr` was allocated by FFmpeg and is freed exactly once here.
        unsafe {
            (self.api.av_packet_free)(&mut self.ptr);
        }
    }
}
