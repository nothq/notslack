use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    thread,
    time::Instant,
};

use hound::WavWriter;
use rtrb::Consumer;

use super::{
    CAPTURE_EVENT_DURATION_LIMIT, CAPTURE_EVENT_NONE, CAPTURE_EVENT_WRITER, WRITER_IDLE_WAIT,
};
use crate::{AudioClipFailure, MAX_AUDIO_CLIP_DURATION};

pub(super) fn write_audio_clip(
    mut writer: WavWriter<std::io::BufWriter<std::fs::File>>,
    mut consumer: Consumer<i16>,
    stop_requested: Arc<AtomicBool>,
    capture_event: Arc<AtomicU8>,
) -> Result<u64, AudioClipFailure> {
    let mut sample_count = 0_u64;
    let started_at = Instant::now();
    loop {
        while let Ok(sample) = consumer.pop() {
            if writer.write_sample(sample).is_err() {
                capture_event.store(CAPTURE_EVENT_WRITER, Ordering::Release);
                stop_requested.store(true, Ordering::Release);
                return Err(AudioClipFailure::Writer);
            }
            sample_count += 1;
        }
        if !stop_requested.load(Ordering::Acquire)
            && started_at.elapsed() >= MAX_AUDIO_CLIP_DURATION
        {
            let _ = capture_event.compare_exchange(
                CAPTURE_EVENT_NONE,
                CAPTURE_EVENT_DURATION_LIMIT,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            stop_requested.store(true, Ordering::Release);
        }
        if stop_requested.load(Ordering::Acquire) {
            break;
        }
        thread::park_timeout(WRITER_IDLE_WAIT);
    }
    writer.finalize().map_err(|_| {
        capture_event.store(CAPTURE_EVENT_WRITER, Ordering::Release);
        stop_requested.store(true, Ordering::Release);
        AudioClipFailure::Writer
    })?;
    Ok(sample_count)
}
