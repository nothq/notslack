use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use cpal::{traits::StreamTrait, Stream};
use rtrb::{Consumer, RingBuffer};
use tempfile::TempDir;

use super::{
    audio_clip_failure,
    input::{self, AudioInputCallbackContext, AudioInputConfiguration},
    output::{create_audio_output, AudioOutput, AudioOutputWriter},
    CAPTURE_EVENT_DURATION_LIMIT, CAPTURE_EVENT_NONE,
};
use crate::{
    AudioClipCaptureStatus, AudioClipFailure, AudioClipSessionId, CapturedAudioClip,
    MediaCaptureError, AUDIO_CLIP_MIMETYPE,
};

type AudioClipWriterThread = JoinHandle<Result<u64, AudioClipFailure>>;

struct AudioWriterTask {
    stop_requested: Arc<AtomicBool>,
    capture_event: Arc<AtomicU8>,
    thread: thread::Thread,
    handle: Option<AudioClipWriterThread>,
}

impl AudioWriterTask {
    fn start(
        session_id: AudioClipSessionId,
        writer: AudioOutputWriter,
        consumer: Consumer<i16>,
    ) -> Result<Self, MediaCaptureError> {
        let stop_requested = Arc::new(AtomicBool::new(false));
        let capture_event = Arc::new(AtomicU8::new(CAPTURE_EVENT_NONE));
        let writer_stop = Arc::clone(&stop_requested);
        let writer_event = Arc::clone(&capture_event);
        let handle = thread::Builder::new()
            .name(format!("notslack-audio-clip-writer-{}", session_id.raw()))
            .spawn(move || {
                super::writer::write_audio_clip(writer, consumer, writer_stop, writer_event)
            })
            .map_err(|error| MediaCaptureError::Output(error.to_string()))?;
        Ok(Self {
            stop_requested,
            capture_event,
            thread: handle.thread().clone(),
            handle: Some(handle),
        })
    }

    fn callback_context(
        &self,
        producer: rtrb::Producer<i16>,
        input: &AudioInputConfiguration,
    ) -> AudioInputCallbackContext {
        AudioInputCallbackContext {
            producer,
            channels: input.channels,
            maximum_samples: input.maximum_samples,
            accepted_samples: 0,
            stop_requested: Arc::clone(&self.stop_requested),
            capture_event: Arc::clone(&self.capture_event),
            writer_thread: self.thread.clone(),
        }
    }

    fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
        self.thread.unpark();
    }

    fn join(&mut self) -> Result<u64, MediaCaptureError> {
        let handle = self
            .handle
            .take()
            .ok_or(MediaCaptureError::WriterThreadStopped)?;
        handle
            .join()
            .map_err(|_| MediaCaptureError::WriterThreadStopped)?
            .map_err(MediaCaptureError::CaptureFailed)
    }

    fn is_finished(&self) -> bool {
        self.handle.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

impl Drop for AudioWriterTask {
    fn drop(&mut self) {
        self.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub(crate) struct ActiveAudioClip {
    pub(super) session_id: AudioClipSessionId,
    sample_rate: u32,
    output_path: PathBuf,
    _temp_dir: TempDir,
    stream: Option<Stream>,
    writer: AudioWriterTask,
}

impl ActiveAudioClip {
    pub(super) fn start(session_id: AudioClipSessionId) -> Result<Self, MediaCaptureError> {
        let input = input::default_input_configuration()?;
        let (producer, consumer) = RingBuffer::<i16>::new(input.ring_capacity);
        let AudioOutput {
            temp_dir,
            path: output_path,
            writer,
        } = create_audio_output(session_id, input.sample_rate)?;
        let writer = AudioWriterTask::start(session_id, writer, consumer)?;
        let callback = writer.callback_context(producer, &input);
        let stream = input::build_input_stream(&input.device, &input.supported, callback)?;
        stream
            .play()
            .map_err(|error| MediaCaptureError::StartStream(error.to_string()))?;
        Ok(Self {
            session_id,
            sample_rate: input.sample_rate,
            output_path,
            _temp_dir: temp_dir,
            stream: Some(stream),
            writer,
        })
    }

    pub(super) fn status(&self) -> AudioClipCaptureStatus {
        let event = self.writer.capture_event.load(Ordering::Acquire);
        if event == CAPTURE_EVENT_NONE
            && !self.writer.stop_requested.load(Ordering::Acquire)
            && self.writer.is_finished()
        {
            return AudioClipCaptureStatus::Failed(AudioClipFailure::Writer);
        }
        match event {
            CAPTURE_EVENT_NONE => AudioClipCaptureStatus::Recording,
            CAPTURE_EVENT_DURATION_LIMIT => AudioClipCaptureStatus::DurationLimitReached,
            event => AudioClipCaptureStatus::Failed(audio_clip_failure(event)),
        }
    }

    pub(super) fn finalize(mut self) -> Result<CapturedAudioClip, MediaCaptureError> {
        self.stop_capture();
        let sample_count = self.writer.join()?;
        let event = self.writer.capture_event.load(Ordering::Acquire);
        if event != CAPTURE_EVENT_NONE && event != CAPTURE_EVENT_DURATION_LIMIT {
            return Err(MediaCaptureError::CaptureFailed(audio_clip_failure(event)));
        }
        let bytes = fs::read(&self.output_path)
            .map_err(|error| MediaCaptureError::Output(error.to_string()))?;
        Ok(CapturedAudioClip {
            filename: format!("Audio clip {}.wav", self.session_id.raw()),
            mimetype: AUDIO_CLIP_MIMETYPE,
            duration: Duration::from_secs_f64(sample_count as f64 / f64::from(self.sample_rate)),
            bytes: Arc::from(bytes),
        })
    }

    pub(super) fn cancel(mut self) -> Result<(), MediaCaptureError> {
        self.stop_capture();
        self.writer.join().map(|_| ())
    }

    fn stop_capture(&mut self) {
        self.stream.take();
        self.writer.stop();
    }
}

impl Drop for ActiveAudioClip {
    fn drop(&mut self) {
        self.stop_capture();
        if self.writer.handle.is_some() {
            let _ = self.writer.join();
        }
    }
}
