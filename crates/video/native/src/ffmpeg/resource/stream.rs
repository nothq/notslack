use std::{ffi::CString, ptr};

use rsmpeg::ffi;

use crate::ffmpeg::{api::FfmpegApi, FfmpegError};

pub(in crate::ffmpeg) fn find_video_stream(
    api: &'static FfmpegApi,
    input: *mut ffi::AVFormatContext,
) -> Result<(usize, *const ffi::AVCodec), FfmpegError> {
    let (stream_index, codec) =
        find_media_stream(api, input, ffi::AVMEDIA_TYPE_VIDEO, FfmpegError::Stream)?;
    let codec = if stream_codec_id(input, stream_index)? == ffi::AV_CODEC_ID_VP8 {
        required_decoder_by_name(api, "libvpx")?
    } else {
        codec
    };
    Ok((stream_index, codec))
}

pub(in crate::ffmpeg) fn find_audio_stream(
    api: &'static FfmpegApi,
    input: *mut ffi::AVFormatContext,
) -> Result<(usize, *const ffi::AVCodec), FfmpegError> {
    find_media_stream(
        api,
        input,
        ffi::AVMEDIA_TYPE_AUDIO,
        FfmpegError::AudioStream,
    )
}

fn stream_codec_id(
    input: *mut ffi::AVFormatContext,
    stream_index: usize,
) -> Result<ffi::AVCodecID, FfmpegError> {
    if input.is_null() {
        return Err(FfmpegError::Stream);
    }
    // SAFETY: `input` is a live format context owned by the caller.
    let stream_count = unsafe { (*input).nb_streams as usize };
    if stream_index >= stream_count {
        return Err(FfmpegError::Stream);
    }
    // SAFETY: `input` is a live format context.
    let streams = unsafe { (*input).streams };
    if streams.is_null() {
        return Err(FfmpegError::Stream);
    }
    // SAFETY: `stream_index` was checked against `nb_streams`.
    let stream = unsafe { *streams.add(stream_index) };
    if stream.is_null() {
        return Err(FfmpegError::Stream);
    }
    // SAFETY: `stream` is a live stream pointer owned by the format context.
    let codecpar = unsafe { (*stream).codecpar };
    if codecpar.is_null() {
        return Err(FfmpegError::Stream);
    }
    // SAFETY: `codecpar` is live for the stream lifetime.
    Ok(unsafe { (*codecpar).codec_id })
}

fn required_decoder_by_name(
    api: &'static FfmpegApi,
    name: &str,
) -> Result<*const ffi::AVCodec, FfmpegError> {
    let name = CString::new(name)?;
    // SAFETY: `name` is a valid C string and FFmpeg returns a static decoder descriptor.
    let codec = unsafe { (api.avcodec_find_decoder_by_name)(name.as_ptr()) };
    if codec.is_null() {
        return Err(FfmpegError::Stream);
    }
    Ok(codec)
}

fn find_media_stream(
    api: &'static FfmpegApi,
    input: *mut ffi::AVFormatContext,
    media_type: ffi::AVMediaType,
    missing_error: FfmpegError,
) -> Result<(usize, *const ffi::AVCodec), FfmpegError> {
    let mut codec = ptr::null();
    // SAFETY: `input` is a live format context and FFmpeg writes the codec pointer on success.
    let stream_index =
        unsafe { (api.av_find_best_stream)(input, media_type, -1, -1, &mut codec, 0) };
    if stream_index < 0 || codec.is_null() {
        return Err(missing_error);
    }
    let stream_index = usize::try_from(stream_index).map_err(|_| FfmpegError::Stream)?;
    Ok((stream_index, codec))
}
