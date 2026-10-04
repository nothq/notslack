use std::{ffi::c_int, ptr, time::Duration};

use rsmpeg::ffi;

use super::{
    api::{api, av_error, check_ffmpeg, FfmpegApi},
    frame_data::{nv12_frame_data_with_visible_size, yuv420p_frame_data},
    resource::{
        find_video_stream, FfmpegCodecContext, FfmpegFrame, FfmpegInput, FfmpegPacket,
        FfmpegScaler, FfmpegScalerConfig,
    },
    FfmpegDecodedFrame, FfmpegError, FfmpegVideoMetadata, Nv12FrameData,
};

pub struct FfmpegVideoDecoder {
    api: &'static FfmpegApi,
    // FFmpeg resources are declared dependent-first so Rust drops scaler,
    // packet/frame, and codec state before closing the owning format input.
    scaler: Option<FfmpegScaler>,
    packet: FfmpegPacket,
    decoded_frame: FfmpegFrame,
    decoder: FfmpegCodecContext,
    input: FfmpegInput,
    stream_index: usize,
    time_base: ffi::AVRational,
    framerate: f64,
    duration: Duration,
    frame_index: u64,
    last_frame_position: Option<Duration>,
    eof_sent: bool,
}

impl FfmpegVideoDecoder {
    pub fn open(source: &str) -> Result<Self, FfmpegError> {
        let api = api()?;
        let input = FfmpegInput::open(api, source)?;
        input.find_stream_info()?;
        let (stream_index, codec) = find_video_stream(api, input.ptr)?;
        input.discard_other_streams(stream_index)?;
        let stream = input.stream(stream_index)?;
        // SAFETY: `stream` was validated by `FfmpegInput::stream`.
        let time_base = unsafe { (*stream).time_base };
        let framerate = stream_framerate(api, input.ptr, stream)?;
        let duration = stream_duration(input.ptr, stream);
        let decoder = FfmpegCodecContext::open(api, stream, codec)?;
        Ok(Self {
            api,
            scaler: None,
            packet: FfmpegPacket::new(api)?,
            decoded_frame: FfmpegFrame::new(api)?,
            decoder,
            input,
            stream_index,
            time_base,
            framerate,
            duration,
            frame_index: 0,
            last_frame_position: None,
            eof_sent: false,
        })
    }

    pub fn metadata(&self) -> FfmpegVideoMetadata {
        FfmpegVideoMetadata {
            width: self.width(),
            height: self.height(),
            framerate: self.framerate,
            duration: self.duration,
        }
    }

    pub fn width(&self) -> i32 {
        self.decoder.width()
    }

    pub fn height(&self) -> i32 {
        self.decoder.height()
    }

    pub const fn framerate(&self) -> f64 {
        self.framerate
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }

    pub fn next_frame(&mut self) -> Result<Option<FfmpegDecodedFrame>, FfmpegError> {
        loop {
            if let Some(frame) = self.receive_frame()? {
                return Ok(Some(frame));
            }
            if self.eof_sent {
                return Ok(None);
            }
            self.read_next_packet()?;
        }
    }

    pub fn seek(&mut self, target: Duration) -> Result<(), FfmpegError> {
        let target_ts = duration_to_timestamp(target, self.time_base);
        self.packet.unref();
        self.decoded_frame.unref();
        // SAFETY: `input` and decoder stream index are owned by this decoder.
        let status = unsafe {
            (self.api.av_seek_frame)(
                self.input.ptr,
                self.stream_index as c_int,
                target_ts,
                ffi::AVSEEK_FLAG_BACKWARD as c_int,
            )
        };
        check_ffmpeg(self.api, "av_seek_frame", status)?;
        // SAFETY: The codec context is valid while `self.decoder` is alive.
        unsafe {
            (self.api.avcodec_flush_buffers)(self.decoder.ptr);
        }
        self.frame_index = if self.framerate > 0.0 {
            (target.as_secs_f64() * self.framerate).max(0.0) as u64
        } else {
            0
        };
        self.last_frame_position = None;
        self.eof_sent = false;
        Ok(())
    }

    fn read_next_packet(&mut self) -> Result<(), FfmpegError> {
        // SAFETY: `input` and `packet` are owned by this decoder.
        let status = unsafe { (self.api.av_read_frame)(self.input.ptr, self.packet.ptr) };
        if status == ffi::AVERROR_EOF {
            self.send_flush_packet()?;
            self.eof_sent = true;
            return Ok(());
        }
        check_ffmpeg(self.api, "av_read_frame", status)?;
        if self.packet.stream_index() == self.stream_index as c_int {
            // SAFETY: The packet belongs to this decoder and targets the selected stream.
            let send_status =
                unsafe { (self.api.avcodec_send_packet)(self.decoder.ptr, self.packet.ptr) };
            self.packet.unref();
            check_ffmpeg(self.api, "avcodec_send_packet", send_status)?;
            return Ok(());
        }
        self.packet.unref();
        Ok(())
    }

    fn send_flush_packet(&mut self) -> Result<(), FfmpegError> {
        // SAFETY: Passing a null packet is FFmpeg's documented decoder drain signal.
        let status = unsafe { (self.api.avcodec_send_packet)(self.decoder.ptr, ptr::null()) };
        if status == ffi::AVERROR_EOF {
            return Ok(());
        }
        check_ffmpeg(self.api, "avcodec_send_packet", status)
    }

    fn receive_frame(&mut self) -> Result<Option<FfmpegDecodedFrame>, FfmpegError> {
        // SAFETY: `decoded_frame` is allocated and owned by this decoder.
        let status =
            unsafe { (self.api.avcodec_receive_frame)(self.decoder.ptr, self.decoded_frame.ptr) };
        if status == av_error(ffi::EAGAIN) || status == ffi::AVERROR_EOF {
            return Ok(None);
        }
        check_ffmpeg(self.api, "avcodec_receive_frame", status)?;
        let position = self.frame_position(self.decoded_frame.ptr);
        let frame = self.convert_frame();
        self.decoded_frame.unref();
        let frame = frame?;
        let interval = self.frame_interval(position);
        self.frame_index = self.frame_index.saturating_add(1);
        Ok(Some(FfmpegDecodedFrame {
            frame,
            position,
            interval,
        }))
    }

    fn convert_frame(&mut self) -> Result<Nv12FrameData, FfmpegError> {
        let decoded = self.decoded_frame.ptr;
        // SAFETY: `decoded` contains a frame returned by `avcodec_receive_frame`.
        let width = unsafe { (*decoded).width };
        // SAFETY: `decoded` contains a frame returned by `avcodec_receive_frame`.
        let height = unsafe { (*decoded).height };
        // SAFETY: `decoded` contains a frame returned by `avcodec_receive_frame`.
        let pix_fmt = unsafe { (*decoded).format as ffi::AVPixelFormat };
        if pix_fmt == ffi::AV_PIX_FMT_YUV420P {
            return yuv420p_frame_data(decoded);
        }
        let output_width = Self::nv12_surface_dimension(width)?;
        let output_height = Self::nv12_surface_dimension(height)?;
        let scaler_config = FfmpegScalerConfig {
            src_width: width,
            src_height: height,
            pix_fmt,
            dst_width: output_width,
            dst_height: output_height,
        };
        let needs_scaler = match &self.scaler {
            Some(scaler) => !scaler.matches(scaler_config),
            None => true,
        };
        if needs_scaler {
            self.scaler = Some(FfmpegScaler::new(self.api, scaler_config)?);
        }
        let nv12_frame = FfmpegFrame::new(self.api)?;
        nv12_frame.configure_nv12(output_width, output_height);
        // SAFETY: `nv12_frame` has format and dimensions set before allocation.
        let status = unsafe { (self.api.av_frame_get_buffer)(nv12_frame.ptr, 0) };
        check_ffmpeg(self.api, "av_frame_get_buffer", status)?;
        let scaler = self.scaler.as_ref().ok_or(FfmpegError::Caps)?;
        scaler.scale(decoded, nv12_frame.ptr)?;
        nv12_frame_data_with_visible_size(nv12_frame.ptr, width, height)
    }

    fn nv12_surface_dimension(value: i32) -> Result<i32, FfmpegError> {
        if value <= 0 {
            return Err(FfmpegError::Caps);
        }
        value.checked_add(value % 2).ok_or(FfmpegError::Caps)
    }

    fn frame_position(&self, frame: *const ffi::AVFrame) -> Duration {
        // SAFETY: `frame` is a decoded frame owned by this decoder.
        let timestamp = unsafe {
            let best_effort = (*frame).best_effort_timestamp;
            if best_effort != ffi::AV_NOPTS_VALUE {
                best_effort
            } else {
                (*frame).pts
            }
        };
        if timestamp != ffi::AV_NOPTS_VALUE {
            return timestamp_to_duration(timestamp, self.time_base);
        }
        Duration::from_secs_f64(self.frame_index as f64 / self.framerate.max(1.0))
    }

    fn frame_interval(&mut self, position: Duration) -> Duration {
        let interval = self
            .last_frame_position
            .and_then(|last_position| position.checked_sub(last_position))
            .unwrap_or(Duration::ZERO);
        self.last_frame_position = Some(position);
        interval
    }
}

fn stream_framerate(
    api: &'static FfmpegApi,
    input: *mut ffi::AVFormatContext,
    stream: *mut ffi::AVStream,
) -> Result<f64, FfmpegError> {
    // SAFETY: `stream` was validated by `FfmpegInput::stream`.
    let stream_ref = unsafe { &*stream };
    let framerate = if valid_rational(stream_ref.avg_frame_rate) {
        rational_to_f64(stream_ref.avg_frame_rate)
    } else if valid_rational(stream_ref.r_frame_rate) {
        rational_to_f64(stream_ref.r_frame_rate)
    } else {
        // SAFETY: FFmpeg owns the format context and stream for the decoder lifetime.
        let guessed = unsafe { (api.av_guess_frame_rate)(input, stream, ptr::null_mut()) };
        rational_to_f64(guessed)
    };
    if framerate.is_nan()
        || framerate.is_infinite()
        || framerate < 0.0
        || framerate.abs() < f64::EPSILON
    {
        return Err(FfmpegError::Framerate(framerate));
    }
    Ok(framerate)
}

pub(super) fn stream_duration(
    input: *mut ffi::AVFormatContext,
    stream: *mut ffi::AVStream,
) -> Duration {
    // SAFETY: `stream` was validated by `FfmpegInput::stream`.
    let stream_ref = unsafe { &*stream };
    if stream_ref.duration > 0 && stream_ref.duration != ffi::AV_NOPTS_VALUE {
        return timestamp_to_duration(stream_ref.duration, stream_ref.time_base);
    }
    // SAFETY: `input` is an open FFmpeg format context.
    let container_duration = unsafe { (*input).duration };
    if container_duration > 0 && container_duration != ffi::AV_NOPTS_VALUE {
        return timestamp_to_duration(container_duration, time_base_q());
    }
    Duration::ZERO
}

fn valid_rational(value: ffi::AVRational) -> bool {
    value.num > 0 && value.den > 0
}

fn rational_to_f64(value: ffi::AVRational) -> f64 {
    if value.den == 0 {
        return 0.0;
    }
    value.num as f64 / value.den as f64
}

pub(super) fn timestamp_to_duration(timestamp: i64, time_base: ffi::AVRational) -> Duration {
    let micros = rescale_q(timestamp, time_base, rational(1, 1_000_000));
    if micros <= 0 {
        return Duration::ZERO;
    }
    Duration::from_micros(micros as u64)
}

pub(super) fn duration_to_timestamp(duration: Duration, time_base: ffi::AVRational) -> i64 {
    let micros = i64::try_from(duration.as_micros()).unwrap_or(i64::MAX);
    rescale_q(micros, rational(1, 1_000_000), time_base)
}

fn rescale_q(value: i64, src: ffi::AVRational, dst: ffi::AVRational) -> i64 {
    if src.den == 0 || dst.num == 0 {
        return 0;
    }
    let numerator = value as i128 * src.num as i128 * dst.den as i128;
    let denominator = src.den as i128 * dst.num as i128;
    (numerator / denominator).clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn rational(num: i32, den: i32) -> ffi::AVRational {
    ffi::AVRational { num, den }
}

fn time_base_q() -> ffi::AVRational {
    rational(1, ffi::AV_TIME_BASE as i32)
}

#[cfg(test)]
mod tests;
