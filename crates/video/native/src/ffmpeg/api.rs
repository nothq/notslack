use std::{
    ffi::{c_char, c_int, CStr},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use libloading::Library;
use rsmpeg::ffi;

use super::FfmpegError;

type AvformatOpenInput = unsafe extern "C" fn(
    *mut *mut ffi::AVFormatContext,
    *const c_char,
    *const ffi::AVInputFormat,
    *mut *mut ffi::AVDictionary,
) -> c_int;
type AvformatFindStreamInfo =
    unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut *mut ffi::AVDictionary) -> c_int;
type AvFindBestStream = unsafe extern "C" fn(
    *mut ffi::AVFormatContext,
    ffi::AVMediaType,
    c_int,
    c_int,
    *mut *const ffi::AVCodec,
    c_int,
) -> c_int;
type AvcodecFindDecoderByName = unsafe extern "C" fn(*const c_char) -> *const ffi::AVCodec;
type AvReadFrame = unsafe extern "C" fn(*mut ffi::AVFormatContext, *mut ffi::AVPacket) -> c_int;
type AvSeekFrame = unsafe extern "C" fn(*mut ffi::AVFormatContext, c_int, i64, c_int) -> c_int;
type AvformatCloseInput = unsafe extern "C" fn(*mut *mut ffi::AVFormatContext);
type AvGuessFrameRate = unsafe extern "C" fn(
    *mut ffi::AVFormatContext,
    *mut ffi::AVStream,
    *mut ffi::AVFrame,
) -> ffi::AVRational;
type AvcodecAllocContext3 = unsafe extern "C" fn(*const ffi::AVCodec) -> *mut ffi::AVCodecContext;
type AvcodecParametersToContext =
    unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVCodecParameters) -> c_int;
type AvcodecOpen2 = unsafe extern "C" fn(
    *mut ffi::AVCodecContext,
    *const ffi::AVCodec,
    *mut *mut ffi::AVDictionary,
) -> c_int;
type AvcodecFreeContext = unsafe extern "C" fn(*mut *mut ffi::AVCodecContext);
type AvcodecSendPacket =
    unsafe extern "C" fn(*mut ffi::AVCodecContext, *const ffi::AVPacket) -> c_int;
type AvcodecReceiveFrame =
    unsafe extern "C" fn(*mut ffi::AVCodecContext, *mut ffi::AVFrame) -> c_int;
type AvcodecFlushBuffers = unsafe extern "C" fn(*mut ffi::AVCodecContext);
type AvFrameAlloc = unsafe extern "C" fn() -> *mut ffi::AVFrame;
type AvFrameFree = unsafe extern "C" fn(*mut *mut ffi::AVFrame);
type AvFrameUnref = unsafe extern "C" fn(*mut ffi::AVFrame);
type AvFrameGetBuffer = unsafe extern "C" fn(*mut ffi::AVFrame, c_int) -> c_int;
type AvPacketAlloc = unsafe extern "C" fn() -> *mut ffi::AVPacket;
type AvPacketFree = unsafe extern "C" fn(*mut *mut ffi::AVPacket);
type AvPacketUnref = unsafe extern "C" fn(*mut ffi::AVPacket);
type AvSamplesGetBufferSize =
    unsafe extern "C" fn(*mut c_int, c_int, c_int, ffi::AVSampleFormat, c_int) -> c_int;
type AvChannelLayoutCopy =
    unsafe extern "C" fn(*mut ffi::AVChannelLayout, *const ffi::AVChannelLayout) -> c_int;
type AvChannelLayoutDefault = unsafe extern "C" fn(*mut ffi::AVChannelLayout, c_int);
type AvChannelLayoutUninit = unsafe extern "C" fn(*mut ffi::AVChannelLayout);
type AvStrerror = unsafe extern "C" fn(c_int, *mut c_char, usize) -> c_int;
type AvLogSetLevel = unsafe extern "C" fn(c_int);
type SwsGetContext = unsafe extern "C" fn(
    c_int,
    c_int,
    ffi::AVPixelFormat,
    c_int,
    c_int,
    ffi::AVPixelFormat,
    c_int,
    *mut ffi::SwsFilter,
    *mut ffi::SwsFilter,
    *const f64,
) -> *mut ffi::SwsContext;
type SwsScale = unsafe extern "C" fn(
    *mut ffi::SwsContext,
    *const *const u8,
    *const c_int,
    c_int,
    c_int,
    *const *mut u8,
    *const c_int,
) -> c_int;
type SwsFreeContext = unsafe extern "C" fn(*mut ffi::SwsContext);

const FFMPEG_LOG_ERROR: c_int = 16;

pub(super) struct FfmpegApi {
    pub(super) avformat_open_input: AvformatOpenInput,
    pub(super) avformat_find_stream_info: AvformatFindStreamInfo,
    pub(super) av_find_best_stream: AvFindBestStream,
    pub(super) avcodec_find_decoder_by_name: AvcodecFindDecoderByName,
    pub(super) av_read_frame: AvReadFrame,
    pub(super) av_seek_frame: AvSeekFrame,
    pub(super) avformat_close_input: AvformatCloseInput,
    pub(super) av_guess_frame_rate: AvGuessFrameRate,
    pub(super) avcodec_alloc_context3: AvcodecAllocContext3,
    pub(super) avcodec_parameters_to_context: AvcodecParametersToContext,
    pub(super) avcodec_open2: AvcodecOpen2,
    pub(super) avcodec_free_context: AvcodecFreeContext,
    pub(super) avcodec_send_packet: AvcodecSendPacket,
    pub(super) avcodec_receive_frame: AvcodecReceiveFrame,
    pub(super) avcodec_flush_buffers: AvcodecFlushBuffers,
    pub(super) av_frame_alloc: AvFrameAlloc,
    pub(super) av_frame_free: AvFrameFree,
    pub(super) av_frame_unref: AvFrameUnref,
    pub(super) av_frame_get_buffer: AvFrameGetBuffer,
    pub(super) av_packet_alloc: AvPacketAlloc,
    pub(super) av_packet_free: AvPacketFree,
    pub(super) av_packet_unref: AvPacketUnref,
    pub(super) av_samples_get_buffer_size: AvSamplesGetBufferSize,
    pub(super) av_channel_layout_copy: AvChannelLayoutCopy,
    pub(super) av_channel_layout_default: AvChannelLayoutDefault,
    pub(super) av_channel_layout_uninit: AvChannelLayoutUninit,
    av_strerror: AvStrerror,
    pub(super) sws_get_context: SwsGetContext,
    pub(super) sws_scale: SwsScale,
    pub(super) sws_free_context: SwsFreeContext,
    _avformat: Library,
    _avcodec: Library,
    _avutil: Library,
    _swscale: Library,
    _swresample: Library,
}

impl FfmpegApi {
    fn load() -> Result<Self, FfmpegError> {
        let avformat = load_ffmpeg_library("libavformat", ffmpeg_library_candidates("avformat"))?;
        let avcodec = load_ffmpeg_library("libavcodec", ffmpeg_library_candidates("avcodec"))?;
        let avutil = load_ffmpeg_library("libavutil", ffmpeg_library_candidates("avutil"))?;
        let swscale = load_ffmpeg_library("libswscale", ffmpeg_library_candidates("swscale"))?;
        let swresample =
            load_ffmpeg_library("libswresample", ffmpeg_library_candidates("swresample"))?;
        let av_log_set_level: AvLogSetLevel = load_symbol(&avutil, "av_log_set_level")?;
        // SAFETY: `av_log_set_level` updates FFmpeg's process-global log threshold.
        unsafe {
            av_log_set_level(FFMPEG_LOG_ERROR);
        }
        Ok(Self {
            avformat_open_input: load_symbol(&avformat, "avformat_open_input")?,
            avformat_find_stream_info: load_symbol(&avformat, "avformat_find_stream_info")?,
            av_find_best_stream: load_symbol(&avformat, "av_find_best_stream")?,
            avcodec_find_decoder_by_name: load_symbol(&avcodec, "avcodec_find_decoder_by_name")?,
            av_read_frame: load_symbol(&avformat, "av_read_frame")?,
            av_seek_frame: load_symbol(&avformat, "av_seek_frame")?,
            avformat_close_input: load_symbol(&avformat, "avformat_close_input")?,
            av_guess_frame_rate: load_symbol(&avformat, "av_guess_frame_rate")?,
            avcodec_alloc_context3: load_symbol(&avcodec, "avcodec_alloc_context3")?,
            avcodec_parameters_to_context: load_symbol(&avcodec, "avcodec_parameters_to_context")?,
            avcodec_open2: load_symbol(&avcodec, "avcodec_open2")?,
            avcodec_free_context: load_symbol(&avcodec, "avcodec_free_context")?,
            avcodec_send_packet: load_symbol(&avcodec, "avcodec_send_packet")?,
            avcodec_receive_frame: load_symbol(&avcodec, "avcodec_receive_frame")?,
            avcodec_flush_buffers: load_symbol(&avcodec, "avcodec_flush_buffers")?,
            av_frame_alloc: load_symbol(&avutil, "av_frame_alloc")?,
            av_frame_free: load_symbol(&avutil, "av_frame_free")?,
            av_frame_unref: load_symbol(&avutil, "av_frame_unref")?,
            av_frame_get_buffer: load_symbol(&avutil, "av_frame_get_buffer")?,
            av_packet_alloc: load_symbol(&avcodec, "av_packet_alloc")?,
            av_packet_free: load_symbol(&avcodec, "av_packet_free")?,
            av_packet_unref: load_symbol(&avcodec, "av_packet_unref")?,
            av_samples_get_buffer_size: load_symbol(&avutil, "av_samples_get_buffer_size")?,
            av_channel_layout_copy: load_symbol(&avutil, "av_channel_layout_copy")?,
            av_channel_layout_default: load_symbol(&avutil, "av_channel_layout_default")?,
            av_channel_layout_uninit: load_symbol(&avutil, "av_channel_layout_uninit")?,
            av_strerror: load_symbol(&avutil, "av_strerror")?,
            sws_get_context: load_symbol(&swscale, "sws_getContext")?,
            sws_scale: load_symbol(&swscale, "sws_scale")?,
            sws_free_context: load_symbol(&swscale, "sws_freeContext")?,
            _avformat: avformat,
            _avcodec: avcodec,
            _avutil: avutil,
            _swscale: swscale,
            _swresample: swresample,
        })
    }

    fn error_message(&self, code: c_int) -> String {
        let mut buffer = [0 as c_char; 128];
        // SAFETY: `buffer` is valid for writes and `av_strerror` writes a C string up to the given length.
        let status = unsafe { (self.av_strerror)(code, buffer.as_mut_ptr(), buffer.len()) };
        if status < 0 {
            return format!("FFmpeg error {code}");
        }
        // SAFETY: Successful `av_strerror` writes a nul-terminated string into `buffer`.
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }
}

pub(super) fn api() -> Result<&'static FfmpegApi, FfmpegError> {
    static API: OnceLock<FfmpegApi> = OnceLock::new();
    if let Some(api) = API.get() {
        return Ok(api);
    }
    let api = FfmpegApi::load()?;
    let _ = API.set(api);
    API.get().ok_or(FfmpegError::Caps)
}

pub(super) fn check_ffmpeg(
    api: &FfmpegApi,
    operation: &'static str,
    code: c_int,
) -> Result<(), FfmpegError> {
    if code >= 0 {
        return Ok(());
    }
    Err(FfmpegError::FfmpegCall {
        operation,
        code,
        message: api.error_message(code),
    })
}

pub(super) fn av_error(error: u32) -> c_int {
    -(error as c_int)
}

fn load_symbol<T: Copy>(library: &Library, symbol: &'static str) -> Result<T, FfmpegError> {
    let mut symbol_name = Vec::with_capacity(symbol.len() + 1);
    symbol_name.extend_from_slice(symbol.as_bytes());
    symbol_name.push(0);
    // SAFETY: The symbol name is nul-terminated and the requested type matches the FFmpeg C ABI.
    unsafe { library.get::<T>(&symbol_name) }
        .map(|symbol| *symbol)
        .map_err(|error| FfmpegError::FfmpegSymbol {
            symbol,
            error: error.to_string(),
        })
}

fn load_ffmpeg_library(
    library: &'static str,
    candidates: &'static [&'static str],
) -> Result<Library, FfmpegError> {
    for path in loaded_image_paths() {
        if path_matches_candidate(&path, candidates) {
            // SAFETY: Loading the already-present dynamic image pins the same FFmpeg dylib handle.
            return unsafe { Library::new(&path) }.map_err(|error| FfmpegError::FfmpegLibrary {
                library,
                error: error.to_string(),
            });
        }
    }
    let mut last_error = None;
    for candidate in candidates {
        // SAFETY: Candidate names are fixed FFmpeg library filenames; symbol validation follows immediately.
        match unsafe { Library::new(candidate) } {
            Ok(library) => return Ok(library),
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(FfmpegError::FfmpegLibrary {
        library,
        error: last_error.unwrap_or_else(|| "no library candidates configured".to_string()),
    })
}

fn path_matches_candidate(path: &Path, candidates: &[&str]) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    candidates
        .iter()
        .any(|candidate| candidate_matches_file_name(candidate, file_name))
}

fn candidate_matches_file_name(candidate: &str, file_name: &str) -> bool {
    if candidate == file_name {
        return true;
    }
    let Some(version_prefix) = candidate.strip_suffix(".dylib") else {
        return false;
    };
    let Some(major) = version_prefix.rsplit('.').next() else {
        return false;
    };
    if !major.as_bytes().iter().all(u8::is_ascii_digit) {
        return false;
    }
    file_name
        .strip_prefix(version_prefix)
        .is_some_and(|suffix| suffix.starts_with('.') && suffix.ends_with(".dylib"))
}

#[cfg(target_os = "macos")]
fn loaded_image_paths() -> Vec<PathBuf> {
    unsafe extern "C" {
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(image_index: u32) -> *const c_char;
    }

    // SAFETY: dyld returns the number of loaded images for this process.
    let count = unsafe { _dyld_image_count() };
    (0..count)
        .filter_map(|index| {
            // SAFETY: dyld owns the returned C string for the lifetime of the process image list.
            let name = unsafe { _dyld_get_image_name(index) };
            if name.is_null() {
                return None;
            }
            // SAFETY: Non-null dyld image names are nul-terminated C strings.
            let path = unsafe { CStr::from_ptr(name) }.to_string_lossy();
            Some(PathBuf::from(path.as_ref()))
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn loaded_image_paths() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn ffmpeg_library_candidates(component: &str) -> &'static [&'static str] {
    match component {
        "avformat" => &["libavformat.62.dylib", "libavformat.dylib"],
        "avcodec" => &["libavcodec.62.dylib", "libavcodec.dylib"],
        "avutil" => &["libavutil.60.dylib", "libavutil.dylib"],
        "swscale" => &["libswscale.9.dylib", "libswscale.dylib"],
        "swresample" => &["libswresample.6.dylib", "libswresample.dylib"],
        _ => &[],
    }
}

#[cfg(target_os = "windows")]
fn ffmpeg_library_candidates(component: &str) -> &'static [&'static str] {
    match component {
        "avformat" => &["avformat-63.dll", "avformat-62.dll", "avformat.dll"],
        "avcodec" => &["avcodec-63.dll", "avcodec-62.dll", "avcodec.dll"],
        "avutil" => &["avutil-61.dll", "avutil-60.dll", "avutil.dll"],
        "swscale" => &["swscale-10.dll", "swscale-9.dll", "swscale.dll"],
        "swresample" => &["swresample-7.dll", "swresample-6.dll", "swresample.dll"],
        _ => &[],
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn ffmpeg_library_candidates(component: &str) -> &'static [&'static str] {
    match component {
        "avformat" => &["libavformat.so.62", "libavformat.so"],
        "avcodec" => &["libavcodec.so.62", "libavcodec.so"],
        "avutil" => &["libavutil.so.60", "libavutil.so"],
        "swscale" => &["libswscale.so.9", "libswscale.so"],
        "swresample" => &["libswresample.so.6", "libswresample.so"],
        _ => &[],
    }
}
