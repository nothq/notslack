mod shutdown;

use std::{
    any::Any,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{Receiver, TryRecvError},
        Arc,
    },
    time::{Duration, Instant},
};

pub(crate) use shutdown::join_worker_in_background;

use parking_lot::Mutex;
use video_native::{FfmpegDecodedFrame, FfmpegVideoDecoder, Nv12FrameData};

use super::{
    error::Error,
    ffmpeg_decoder_lock,
    state::{validate_speed, PlaybackCommand, VideoFrameData},
};

#[cfg(target_os = "linux")]
use super::{render_image::render_image_from_nv12, state::RenderedVideoFrame};

pub(crate) struct FrameWorkerInput {
    pub(crate) source: String,
    pub(crate) _retained_source: Option<Arc<dyn Any + Send + Sync>>,
    pub(crate) control_rx: Receiver<PlaybackCommand>,
    pub(crate) frame: Arc<Mutex<Option<VideoFrameData>>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) frame_buffer: Arc<Mutex<VecDeque<VideoFrameData>>>,
    pub(crate) frame_buffer_capacity: Arc<AtomicUsize>,
    #[cfg(target_os = "linux")]
    pub(crate) rendered_frame: Arc<Mutex<Option<RenderedVideoFrame>>>,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) last_frame_time: Arc<Mutex<Instant>>,
    pub(crate) looping: Arc<AtomicBool>,
    pub(crate) speed: Arc<AtomicU64>,
    pub(crate) current_position_ns: Arc<AtomicU64>,
    pub(crate) pending_seek_generation: Arc<AtomicU64>,
    pub(crate) subtitle_text: Arc<Mutex<Option<String>>>,
    pub(crate) upload_text: Arc<AtomicBool>,
    pub(crate) is_eos: Arc<AtomicBool>,
    pub(crate) paused: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
pub(crate) struct VideoMetadata {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    pub(crate) duration: Duration,
}

#[derive(Debug, Clone, Copy)]
struct PendingSeek {
    target: Duration,
    generation: u64,
}

pub(crate) fn spawn_frame_worker(input: FrameWorkerInput) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || run_frame_worker(input))
}

fn open_video_decoder(source: &str) -> Result<FfmpegVideoDecoder, Error> {
    let _guard = ffmpeg_decoder_lock();
    Ok(FfmpegVideoDecoder::open(source)?)
}

fn with_video_decoder<T>(
    decoder: &mut FfmpegVideoDecoder,
    operation: impl FnOnce(&mut FfmpegVideoDecoder) -> T,
) -> T {
    let _guard = ffmpeg_decoder_lock();
    operation(decoder)
}

fn drop_video_decoder(decoder: FfmpegVideoDecoder) {
    let _guard = ffmpeg_decoder_lock();
    drop(decoder);
}

pub(crate) fn probe_video(source: &str) -> Result<VideoMetadata, Error> {
    let decoder = open_video_decoder(source)?;
    let metadata = decoder.metadata();
    let result = VideoMetadata {
        width: metadata.width,
        height: metadata.height,
        framerate: metadata.framerate,
        duration: metadata.duration,
    };
    drop_video_decoder(decoder);
    Ok(result)
}

fn run_frame_worker(input: FrameWorkerInput) {
    let Some(mut decoder) = open_worker_decoder(&input) else {
        return;
    };
    let mut pending_seek = None;
    while input.alive.load(Ordering::Acquire) {
        if !run_frame_worker_iteration(&input, &mut decoder, &mut pending_seek) {
            break;
        }
    }
    drop_video_decoder(decoder);
}

fn open_worker_decoder(input: &FrameWorkerInput) -> Option<FfmpegVideoDecoder> {
    match open_video_decoder(&input.source) {
        Ok(decoder) => Some(decoder),
        Err(err) => {
            log::error!("failed to open video decoder: {err}");
            input.is_eos.store(true, Ordering::SeqCst);
            None
        }
    }
}

fn run_frame_worker_iteration(
    input: &FrameWorkerInput,
    decoder: &mut FfmpegVideoDecoder,
    pending_seek: &mut Option<PendingSeek>,
) -> bool {
    if !with_video_decoder(decoder, |decoder| {
        drain_commands(input, decoder, pending_seek)
    }) {
        return false;
    }
    if wait_before_decode(input, pending_seek) {
        return true;
    }
    decode_next_frame(input, decoder, pending_seek);
    true
}

fn wait_before_decode(input: &FrameWorkerInput, pending_seek: &Option<PendingSeek>) -> bool {
    if input.is_eos.load(Ordering::Acquire) {
        std::thread::sleep(Duration::from_millis(50));
        return true;
    }
    if input.paused.load(Ordering::Acquire) && pending_seek.is_none() {
        std::thread::sleep(Duration::from_millis(16));
        return true;
    }
    false
}

fn decode_next_frame(
    input: &FrameWorkerInput,
    decoder: &mut FfmpegVideoDecoder,
    pending_seek: &mut Option<PendingSeek>,
) {
    let (decoded, framerate) = with_video_decoder(decoder, |decoder| {
        (decoder.next_frame(), decoder.framerate())
    });
    match decoded {
        Ok(Some(decoded)) => store_decoded_frame(input, pending_seek, decoded, framerate),
        Ok(None) => with_video_decoder(decoder, |decoder| {
            clear_pending_seek(input, pending_seek);
            handle_eos(input, decoder)
        }),
        Err(err) => {
            log::error!("error decoding video frame: {err}");
            clear_pending_seek(input, pending_seek);
            input.is_eos.store(true, Ordering::SeqCst);
        }
    }
}

fn store_decoded_frame(
    input: &FrameWorkerInput,
    pending_seek: &mut Option<PendingSeek>,
    decoded: FfmpegDecodedFrame,
    framerate: f64,
) {
    let Some(display_position) =
        display_position_for_decoded_frame(input, pending_seek, decoded.position, framerate)
    else {
        return;
    };
    if !input.paused.load(Ordering::Acquire) {
        pace_frame(input, decoded.interval);
    }
    if let Err(error) = store_frame(input, video_frame_data(decoded.frame)) {
        log::error!("failed to prepare decoded video frame: {error}");
        input.is_eos.store(true, Ordering::SeqCst);
        return;
    }
    input
        .current_position_ns
        .store(duration_to_nanos(display_position), Ordering::SeqCst);
    clear_pending_seek(input, pending_seek);
}

fn drain_commands(
    input: &FrameWorkerInput,
    decoder: &mut FfmpegVideoDecoder,
    pending_seek: &mut Option<PendingSeek>,
) -> bool {
    loop {
        match input.control_rx.try_recv() {
            Ok(PlaybackCommand::SetPaused(paused)) => {
                input.paused.store(paused, Ordering::SeqCst);
                if !paused {
                    *input.last_frame_time.lock() = Instant::now();
                }
            }
            Ok(PlaybackCommand::Seek { target, generation }) => {
                clear_pending_seek(input, pending_seek);
                input
                    .pending_seek_generation
                    .store(generation, Ordering::SeqCst);
                if let Err(err) = decoder.seek(target) {
                    log::error!("failed to seek video: {err}");
                    clear_pending_seek_generation(input, generation);
                    input.is_eos.store(true, Ordering::SeqCst);
                } else {
                    *pending_seek = Some(PendingSeek { target, generation });
                    reset_after_seek(input, target);
                }
            }
            Ok(PlaybackCommand::SetSpeed(speed)) => {
                if let Err(err) = validate_speed(speed) {
                    log::error!("failed to set video speed: {err}");
                }
                *input.last_frame_time.lock() = Instant::now();
            }
            Ok(PlaybackCommand::Restart) => {
                clear_pending_seek(input, pending_seek);
                input.pending_seek_generation.store(0, Ordering::SeqCst);
                if let Err(err) = decoder.seek(Duration::ZERO) {
                    log::error!("failed to restart video: {err}");
                    input.is_eos.store(true, Ordering::SeqCst);
                } else {
                    reset_after_seek(input, Duration::ZERO);
                    input.paused.store(false, Ordering::SeqCst);
                }
            }
            Ok(PlaybackCommand::Shutdown) => return false,
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
}

fn reset_after_seek(input: &FrameWorkerInput, target: Duration) {
    input.is_eos.store(false, Ordering::SeqCst);
    input.frame_buffer.lock().clear();
    input.upload_frame.store(false, Ordering::SeqCst);
    *input.subtitle_text.lock() = None;
    input.upload_text.store(true, Ordering::SeqCst);
    input
        .current_position_ns
        .store(duration_to_nanos(target), Ordering::SeqCst);
    *input.last_frame_time.lock() = Instant::now();
}

fn handle_eos(input: &FrameWorkerInput, decoder: &mut FfmpegVideoDecoder) {
    if !input.looping.load(Ordering::SeqCst) {
        input.is_eos.store(true, Ordering::SeqCst);
        return;
    }
    if let Err(err) = decoder.seek(Duration::ZERO) {
        log::error!("failed to restart video for looping: {err}");
        input.is_eos.store(true, Ordering::SeqCst);
        return;
    }
    reset_after_seek(input, Duration::ZERO);
}

fn display_position_for_decoded_frame(
    input: &FrameWorkerInput,
    pending_seek: &mut Option<PendingSeek>,
    decoded_position: Duration,
    framerate: f64,
) -> Option<Duration> {
    let active_generation = input.pending_seek_generation.load(Ordering::SeqCst);
    let Some(seek) = *pending_seek else {
        if active_generation == 0 {
            return Some(decoded_position);
        }
        return None;
    };

    if active_generation != seek.generation {
        *pending_seek = None;
        if active_generation == 0 {
            return Some(decoded_position);
        }
        return None;
    }

    if !frame_reaches_seek_target(decoded_position, seek.target, framerate) {
        return None;
    }

    Some(max_duration(decoded_position, seek.target))
}

fn clear_pending_seek(input: &FrameWorkerInput, pending_seek: &mut Option<PendingSeek>) {
    let Some(seek) = pending_seek.take() else {
        return;
    };
    clear_pending_seek_generation(input, seek.generation);
}

fn clear_pending_seek_generation(input: &FrameWorkerInput, generation: u64) {
    let _ = input.pending_seek_generation.compare_exchange(
        generation,
        0,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );
}

fn frame_reaches_seek_target(position: Duration, target: Duration, framerate: f64) -> bool {
    if position >= target {
        return true;
    }
    match position.checked_add(seek_tolerance(framerate)) {
        Some(position) => position >= target,
        None => true,
    }
}

fn seek_tolerance(framerate: f64) -> Duration {
    if framerate.is_finite() && framerate > 0.0 {
        return Duration::from_secs_f64(0.5 / framerate);
    }
    Duration::from_millis(2)
}

fn max_duration(left: Duration, right: Duration) -> Duration {
    if left >= right {
        left
    } else {
        right
    }
}

fn store_frame(input: &FrameWorkerInput, frame: VideoFrameData) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let rendered_frame = render_image_from_nv12(&frame)?;
        *input.rendered_frame.lock() = Some(rendered_frame);
    }
    *input.frame.lock() = Some(frame.clone());
    let capacity = input.frame_buffer_capacity.load(Ordering::SeqCst);
    if capacity == 0 {
        input.upload_frame.store(true, Ordering::SeqCst);
        return Ok(());
    }
    let mut buffer = input.frame_buffer.lock();
    buffer.push_back(frame);
    while buffer.len() > capacity {
        buffer.pop_front();
    }
    input.upload_frame.store(true, Ordering::SeqCst);
    Ok(())
}

fn pace_frame(input: &FrameWorkerInput, frame_interval: Duration) {
    let speed = f64::from_bits(input.speed.load(Ordering::SeqCst));
    let interval = if speed.is_finite() && speed > 0.0 {
        frame_interval.div_f64(speed)
    } else {
        frame_interval
    };
    let mut last_frame_time = input.last_frame_time.lock();
    if interval.is_zero() {
        *last_frame_time = Instant::now();
        return;
    }

    let target = (*last_frame_time)
        .checked_add(interval)
        .unwrap_or_else(Instant::now);
    if let Some(delay) = target.checked_duration_since(Instant::now()) {
        std::thread::sleep(delay);
    }
    *last_frame_time = target;
}

fn duration_to_nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn video_frame_data(frame: Nv12FrameData) -> VideoFrameData {
    VideoFrameData {
        nv12_data: frame.nv12_data,
        width: frame.width,
        height: frame.height,
        uv_row_width: frame.uv_row_width,
    }
}
