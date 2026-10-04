use std::{
    any::Any,
    collections::VecDeque,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use parking_lot::{Mutex, RwLock};

pub use super::state::{Position, VideoOptions};
use super::{
    audio::AudioPlayback,
    error::Error,
    state::{validate_speed, Internal, PlaybackCommand, VideoFrameData},
    worker::{
        join_worker_in_background, probe_video, spawn_frame_worker, FrameWorkerInput, VideoMetadata,
    },
};

/// A multimedia video loaded from a URI.
#[derive(Debug, Clone)]
pub struct Video(pub(crate) Rc<RwLock<Internal>>);

struct PlaybackRuntime {
    frame: Arc<Mutex<Option<VideoFrameData>>>,
    upload_frame: Arc<AtomicBool>,
    frame_buffer: Arc<Mutex<VecDeque<VideoFrameData>>>,
    frame_buffer_capacity: Arc<AtomicUsize>,
    #[cfg(target_os = "linux")]
    rendered_frame: Arc<Mutex<Option<super::state::RenderedVideoFrame>>>,
    alive: Arc<AtomicBool>,
    last_frame_time: Arc<Mutex<Instant>>,
    looping_flag: Arc<AtomicBool>,
    speed_state: Arc<AtomicU64>,
    current_position_ns: Arc<AtomicU64>,
    seek_generation: Arc<AtomicU64>,
    pending_seek_generation: Arc<AtomicU64>,
    subtitle_text: Arc<Mutex<Option<String>>>,
    upload_text: Arc<AtomicBool>,
    is_eos: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

struct PlaybackStart {
    id: u64,
    source: String,
    metadata: VideoMetadata,
    control_tx: mpsc::Sender<PlaybackCommand>,
    worker: std::thread::JoinHandle<()>,
    audio: Option<AudioPlayback>,
    audio_start_error: Option<String>,
}

impl PlaybackRuntime {
    fn new(options: VideoOptions) -> Result<Self, Error> {
        let initial_looping = options.looping.unwrap_or(false);
        let initial_speed = options.speed.unwrap_or(1.0);
        validate_speed(initial_speed)?;
        Ok(Self {
            frame: Arc::new(Mutex::new(None)),
            upload_frame: Arc::new(AtomicBool::new(false)),
            frame_buffer: Arc::new(Mutex::new(VecDeque::new())),
            frame_buffer_capacity: Arc::new(AtomicUsize::new(
                options.frame_buffer_capacity.unwrap_or(3),
            )),
            #[cfg(target_os = "linux")]
            rendered_frame: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            last_frame_time: Arc::new(Mutex::new(Instant::now())),
            looping_flag: Arc::new(AtomicBool::new(initial_looping)),
            speed_state: Arc::new(AtomicU64::new(initial_speed.to_bits())),
            current_position_ns: Arc::new(AtomicU64::new(0)),
            seek_generation: Arc::new(AtomicU64::new(0)),
            pending_seek_generation: Arc::new(AtomicU64::new(0)),
            subtitle_text: Arc::new(Mutex::new(None)),
            upload_text: Arc::new(AtomicBool::new(false)),
            is_eos: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        })
    }

    fn spawn_worker(
        &self,
        source: String,
        control_rx: mpsc::Receiver<PlaybackCommand>,
        retained_source: Option<Arc<dyn Any + Send + Sync>>,
    ) -> std::thread::JoinHandle<()> {
        spawn_frame_worker(FrameWorkerInput {
            source,
            _retained_source: retained_source,
            control_rx,
            frame: Arc::clone(&self.frame),
            upload_frame: Arc::clone(&self.upload_frame),
            frame_buffer: Arc::clone(&self.frame_buffer),
            frame_buffer_capacity: Arc::clone(&self.frame_buffer_capacity),
            #[cfg(target_os = "linux")]
            rendered_frame: Arc::clone(&self.rendered_frame),
            alive: Arc::clone(&self.alive),
            last_frame_time: Arc::clone(&self.last_frame_time),
            looping: Arc::clone(&self.looping_flag),
            speed: Arc::clone(&self.speed_state),
            current_position_ns: Arc::clone(&self.current_position_ns),
            pending_seek_generation: Arc::clone(&self.pending_seek_generation),
            subtitle_text: Arc::clone(&self.subtitle_text),
            upload_text: Arc::clone(&self.upload_text),
            is_eos: Arc::clone(&self.is_eos),
            paused: Arc::clone(&self.paused),
        })
    }

    fn into_internal(self, input: PlaybackStart) -> Internal {
        Internal {
            id: input.id,
            source: input.source,
            alive: self.alive,
            worker: Some(input.worker),
            control_tx: input.control_tx,

            width: input.metadata.width,
            height: input.metadata.height,
            framerate: input.metadata.framerate,
            duration: input.metadata.duration,
            speed: self.speed_state,
            current_position_ns: self.current_position_ns,
            seek_generation: self.seek_generation,
            pending_seek_generation: self.pending_seek_generation,

            frame: self.frame,
            upload_frame: self.upload_frame,
            #[cfg(target_os = "macos")]
            render_surface_pool: None,
            #[cfg(target_os = "macos")]
            render_surfaces: VecDeque::new(),
            #[cfg(target_os = "linux")]
            rendered_frame: self.rendered_frame,
            frame_buffer: self.frame_buffer,
            frame_buffer_capacity: self.frame_buffer_capacity,
            last_frame_time: self.last_frame_time,
            looping: self.looping_flag,
            is_eos: self.is_eos,
            paused: self.paused,

            subtitle_text: self.subtitle_text,
            upload_text: self.upload_text,

            display_width_override: None,
            display_height_override: None,
            volume: 1.0,
            muted: false,
            audio: input.audio,
            audio_start_error: input.audio_start_error,
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        if Rc::strong_count(&self.0) != 1 {
            return;
        }
        let Some(mut inner) = self.0.try_write() else {
            return;
        };
        let audio = request_video_shutdown(&mut inner);
        let worker = inner.worker.take();
        drop(inner);
        drop(audio);
        join_worker_in_background("video", worker);
    }
}

fn request_video_shutdown(inner: &mut Internal) -> Option<AudioPlayback> {
    if inner.alive.swap(false, Ordering::SeqCst) {
        let _ = inner.control_tx.send(PlaybackCommand::Shutdown);
    }
    inner.audio.take()
}

impl Video {
    /// Create a new video player from a given video which loads from `uri`.
    pub fn new(uri: &url::Url) -> Result<Self, Error> {
        Self::new_with_options(uri, VideoOptions::default())
    }

    /// Create a new video player from a given video which loads from `uri`,
    /// applying initialization options.
    pub fn new_with_options(uri: &url::Url, options: VideoOptions) -> Result<Self, Error> {
        Self::new_with_metadata(uri, None, None, options)
    }

    pub(crate) fn new_with_metadata(
        uri: &url::Url,
        metadata: Option<VideoMetadata>,
        retained_source: Option<Arc<dyn Any + Send + Sync>>,
        options: VideoOptions,
    ) -> Result<Self, Error> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let source = source_from_uri(uri)?;
        let metadata = match metadata {
            Some(metadata) => metadata,
            None => probe_video(&source)?,
        };
        let audio_enabled = options.audio_enabled.unwrap_or(true);
        let runtime = PlaybackRuntime::new(options)?;
        let (control_tx, control_rx) = mpsc::channel();
        let worker = runtime.spawn_worker(source.clone(), control_rx, retained_source.clone());
        let (audio, audio_start_error) = if audio_enabled {
            start_audio_playback(&source, &runtime, retained_source)
        } else {
            (None, None)
        };
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let internal = runtime.into_internal(PlaybackStart {
            id,
            source,
            metadata,
            control_tx,
            worker,
            audio,
            audio_start_error,
        });

        Ok(Video(Rc::new(RwLock::new(internal))))
    }

    pub(crate) fn shutdown_in_background(&mut self) {
        let mut inner = self.0.write();
        let audio = request_video_shutdown(&mut inner);
        let worker = inner.worker.take();
        drop(inner);
        drop(audio);
        join_worker_in_background("video", worker);
    }

    pub(crate) fn read(&'_ self) -> parking_lot::RwLockReadGuard<'_, Internal> {
        self.0.read()
    }

    pub(crate) fn write(&'_ self) -> parking_lot::RwLockWriteGuard<'_, Internal> {
        self.0.write()
    }
}

pub(crate) fn probe_video_metadata(uri: &url::Url) -> Result<VideoMetadata, Error> {
    let source = source_from_uri(uri)?;
    probe_video(&source)
}

fn start_audio_playback(
    source: &str,
    runtime: &PlaybackRuntime,
    retained_source: Option<Arc<dyn Any + Send + Sync>>,
) -> (Option<AudioPlayback>, Option<String>) {
    let initial_looping = runtime.looping_flag.load(Ordering::SeqCst);
    let initial_speed = f64::from_bits(runtime.speed_state.load(Ordering::SeqCst));
    let (audio, audio_start_error) = match AudioPlayback::start(source.to_string(), retained_source)
    {
        Ok(audio) => (audio, None),
        Err(error) => {
            let message = format!("audio unavailable: {error}");
            log::error!("{message}");
            (None, Some(message))
        }
    };
    if let Some(audio) = &audio {
        let position = Duration::from_nanos(runtime.current_position_ns.load(Ordering::SeqCst));
        audio.set_looping(initial_looping);
        audio.set_speed(initial_speed, position);
    }
    (audio, audio_start_error)
}

fn source_from_uri(uri: &url::Url) -> Result<String, Error> {
    if uri.scheme() != "file" {
        return Ok(uri.as_str().to_string());
    }
    let path = uri.to_file_path().map_err(|_| Error::Uri)?;
    Ok(path.to_string_lossy().into_owned())
}
