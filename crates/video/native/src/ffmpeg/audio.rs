use std::{ffi::c_int, ptr, time::Duration};

use rsmpeg::{ffi, swresample::SwrContext};

use super::{
    api::{api, av_error, check_ffmpeg, FfmpegApi},
    channel_layout::FfmpegChannelLayout,
    decoder::{duration_to_timestamp, stream_duration, timestamp_to_duration},
    resource::{find_audio_stream, FfmpegCodecContext, FfmpegFrame, FfmpegInput, FfmpegPacket},
    FfmpegAudioFrame, FfmpegAudioMetadata, FfmpegAudioOutputConfig, FfmpegError,
};

pub fn probe_audio(source: &str) -> Result<(), FfmpegError> {
    probe_audio_metadata(source).map(|_| ())
}

pub fn probe_audio_metadata(source: &str) -> Result<FfmpegAudioMetadata, FfmpegError> {
    let api = api()?;
    let input = FfmpegInput::open(api, source)?;
    input.find_stream_info()?;
    let (stream_index, _) = find_audio_stream(api, input.ptr)?;
    let stream = input.stream(stream_index)?;
    Ok(FfmpegAudioMetadata {
        duration: stream_duration(input.ptr, stream),
    })
}

pub struct FfmpegAudioDecoder {
    api: &'static FfmpegApi,
    // FFmpeg resources are declared dependent-first so Rust drops resampler,
    // packet/frame, and codec state before closing the owning format input.
    resampler: SwrContext,
    packet: FfmpegPacket,
    decoded_frame: FfmpegFrame,
    decoder: FfmpegCodecContext,
    input: FfmpegInput,
    stream_index: usize,
    time_base: ffi::AVRational,
    duration: Duration,
    output_sample_rate: u32,
    output_channels: u16,
    eof_sent: bool,
}

impl FfmpegAudioDecoder {
    pub fn open(source: &str, output: FfmpegAudioOutputConfig) -> Result<Self, FfmpegError> {
        if output.sample_rate == 0 || output.channels == 0 {
            return Err(FfmpegError::Caps);
        }
        let api = api()?;
        let input = FfmpegInput::open(api, source)?;
        input.find_stream_info()?;
        let (stream_index, codec) = find_audio_stream(api, input.ptr)?;
        input.discard_other_streams(stream_index)?;
        let stream = input.stream(stream_index)?;
        // SAFETY: `stream` was validated by `FfmpegInput::stream`.
        let time_base = unsafe { (*stream).time_base };
        let duration = stream_duration(input.ptr, stream);
        let decoder = FfmpegCodecContext::open(api, stream, codec)?;
        let resampler = audio_resampler(api, &decoder, output)?;
        Ok(Self {
            api,
            resampler,
            packet: FfmpegPacket::new(api)?,
            decoded_frame: FfmpegFrame::new(api)?,
            decoder,
            input,
            stream_index,
            time_base,
            duration,
            output_sample_rate: output.sample_rate,
            output_channels: output.channels,
            eof_sent: false,
        })
    }

    pub const fn metadata(&self) -> FfmpegAudioMetadata {
        FfmpegAudioMetadata {
            duration: self.duration,
        }
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }

    pub fn next_frame(&mut self) -> Result<Option<FfmpegAudioFrame>, FfmpegError> {
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
        self.resampler = audio_resampler(
            self.api,
            &self.decoder,
            FfmpegAudioOutputConfig {
                sample_rate: self.output_sample_rate,
                channels: self.output_channels,
            },
        )?;
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

    fn receive_frame(&mut self) -> Result<Option<FfmpegAudioFrame>, FfmpegError> {
        // SAFETY: `decoded_frame` is allocated and owned by this decoder.
        let status =
            unsafe { (self.api.avcodec_receive_frame)(self.decoder.ptr, self.decoded_frame.ptr) };
        if status == av_error(ffi::EAGAIN) || status == ffi::AVERROR_EOF {
            return Ok(None);
        }
        check_ffmpeg(self.api, "avcodec_receive_frame", status)?;
        let position = audio_frame_position(self.decoded_frame.ptr, self.time_base);
        let samples = self.convert_frame();
        self.decoded_frame.unref();
        let samples = samples?;
        Ok(Some(FfmpegAudioFrame { samples, position }))
    }

    fn convert_frame(&mut self) -> Result<Vec<i16>, FfmpegError> {
        let decoded = self.decoded_frame.ptr;
        // SAFETY: `decoded` contains a frame returned by `avcodec_receive_frame`.
        let input_samples = unsafe { (*decoded).nb_samples };
        if input_samples <= 0 {
            return Ok(Vec::new());
        }
        let output_samples = self
            .resampler
            .get_out_samples(input_samples)
            .max(input_samples);
        let mut output_bytes = vec![
            0u8;
            audio_buffer_size(
                self.api,
                i32::from(self.output_channels),
                output_samples,
                ffi::AV_SAMPLE_FMT_S16
            )?
        ];
        let mut output_ptr = output_bytes.as_mut_ptr();
        // SAFETY: `decoded` is a live decoded audio frame. `extended_data` is FFmpeg's
        // canonical audio plane pointer array when present.
        let input_data = unsafe {
            if (*decoded).extended_data.is_null() {
                (*decoded).data.as_ptr() as *const *const u8
            } else {
                (*decoded).extended_data as *const *const u8
            }
        };
        // SAFETY: Output storage is sized for `output_samples` per channel and input
        // plane pointers come from a live FFmpeg frame.
        let written = unsafe {
            self.resampler
                .convert(&mut output_ptr, output_samples, input_data, input_samples)
        }
        .map_err(|error| FfmpegError::Resampler(error.to_string()))?;
        let actual_bytes = audio_buffer_size(
            self.api,
            i32::from(self.output_channels),
            written,
            ffi::AV_SAMPLE_FMT_S16,
        )?;
        output_bytes.truncate(actual_bytes);
        Ok(bytes_to_i16_samples(&output_bytes))
    }
}

fn audio_resampler(
    api: &'static FfmpegApi,
    decoder: &FfmpegCodecContext,
    output: FfmpegAudioOutputConfig,
) -> Result<SwrContext, FfmpegError> {
    let input_layout = decoder.channel_layout()?;
    let output_layout = FfmpegChannelLayout::default_for_channels(api, output.channels)?;
    let input_sample_rate = decoder.sample_rate();
    if input_sample_rate <= 0 {
        return Err(FfmpegError::Caps);
    }
    let output_sample_rate = i32::try_from(output.sample_rate).map_err(|_| FfmpegError::Caps)?;
    let mut resampler = SwrContext::new(
        &output_layout.inner,
        ffi::AV_SAMPLE_FMT_S16,
        output_sample_rate,
        &input_layout.inner,
        decoder.sample_format(),
        input_sample_rate,
    )
    .map_err(|error| FfmpegError::Resampler(error.to_string()))?;
    resampler
        .init()
        .map_err(|error| FfmpegError::Resampler(error.to_string()))?;
    Ok(resampler)
}

fn audio_frame_position(frame: *const ffi::AVFrame, time_base: ffi::AVRational) -> Duration {
    // SAFETY: `frame` is a decoded frame owned by this decoder.
    let timestamp = unsafe {
        let best_effort = (*frame).best_effort_timestamp;
        if best_effort != ffi::AV_NOPTS_VALUE {
            best_effort
        } else {
            (*frame).pts
        }
    };
    if timestamp == ffi::AV_NOPTS_VALUE {
        return Duration::ZERO;
    }
    timestamp_to_duration(timestamp, time_base)
}

fn audio_buffer_size(
    api: &'static FfmpegApi,
    channels: i32,
    samples: i32,
    sample_format: ffi::AVSampleFormat,
) -> Result<usize, FfmpegError> {
    if channels <= 0 || samples < 0 {
        return Err(FfmpegError::Caps);
    }
    // SAFETY: This is a pure FFmpeg size calculation for validated audio dimensions.
    let size = unsafe {
        (api.av_samples_get_buffer_size)(ptr::null_mut(), channels, samples, sample_format, 1)
    };
    if size < 0 {
        return Err(FfmpegError::Caps);
    }
    usize::try_from(size).map_err(|_| FfmpegError::Caps)
}

fn bytes_to_i16_samples(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|sample| i16::from_ne_bytes([sample[0], sample[1]]))
        .collect()
}

#[cfg(test)]
mod audio_tests;
