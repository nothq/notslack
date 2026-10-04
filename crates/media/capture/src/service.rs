use std::{
    num::NonZeroU64,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use crate::{
    AudioClipCaptureStatus, AudioClipSessionId, CapturedAudioClip, CapturedVideoClip,
    MediaCaptureCapabilities, MediaCaptureError, MediaCaptureService, VideoClipSessionId,
    VideoClipSnapshot,
};

mod actor;

const ACTOR_COMMAND_CAPACITY: usize = 1;

pub fn production_media_capture_service() -> Arc<dyn MediaCaptureService> {
    Arc::new(
        NativeMediaCaptureService::new()
            .expect("the production media capture actor must start successfully"),
    )
}

struct NativeMediaCaptureService {
    next_session_id: AtomicU64,
    commands: SyncSender<CaptureActorCommand>,
    actor_thread: Option<JoinHandle<()>>,
}

impl NativeMediaCaptureService {
    fn new() -> Result<Self, MediaCaptureError> {
        let (commands, command_rx) = mpsc::sync_channel(ACTOR_COMMAND_CAPACITY);
        let actor_thread = thread::Builder::new()
            .name("notslack-media-capture-owner".to_string())
            .spawn(move || actor::run_capture_actor(command_rx))
            .map_err(|error| MediaCaptureError::ActorSpawn(error.to_string()))?;
        Ok(Self {
            next_session_id: AtomicU64::new(1),
            commands,
            actor_thread: Some(actor_thread),
        })
    }

    fn next_session_id(&self) -> Result<NonZeroU64, MediaCaptureError> {
        let value = self
            .next_session_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| MediaCaptureError::SessionIdExhausted)?;
        NonZeroU64::new(value).ok_or(MediaCaptureError::SessionIdExhausted)
    }

    fn request<Reply: Send>(
        &self,
        command: impl FnOnce(SyncSender<Result<Reply, MediaCaptureError>>) -> CaptureActorCommand,
    ) -> Result<Reply, MediaCaptureError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(0);
        self.commands
            .send(command(reply_tx))
            .map_err(|_| MediaCaptureError::ActorStopped)?;
        reply_rx
            .recv()
            .map_err(|_| MediaCaptureError::ActorStopped)?
    }
}

impl MediaCaptureService for NativeMediaCaptureService {
    fn capabilities(&self) -> MediaCaptureCapabilities {
        MediaCaptureCapabilities {
            audio_clip: true,
            video_clip: cfg!(target_os = "macos"),
        }
    }

    fn start_audio_clip(&self) -> Result<AudioClipSessionId, MediaCaptureError> {
        let session_id = AudioClipSessionId::from_raw(self.next_session_id()?);
        self.request(|reply| CaptureActorCommand::StartAudio { session_id, reply })
    }

    fn audio_clip_status(
        &self,
        session_id: AudioClipSessionId,
    ) -> Result<AudioClipCaptureStatus, MediaCaptureError> {
        self.request(|reply| CaptureActorCommand::AudioStatus { session_id, reply })
    }

    fn stop_audio_clip(
        &self,
        session_id: AudioClipSessionId,
    ) -> Result<CapturedAudioClip, MediaCaptureError> {
        self.request(|reply| CaptureActorCommand::StopAudio { session_id, reply })
    }

    fn cancel_audio_clip(&self, session_id: AudioClipSessionId) -> Result<(), MediaCaptureError> {
        self.request(|reply| CaptureActorCommand::CancelAudio { session_id, reply })
    }

    fn prepare_video_clip(&self) -> Result<VideoClipSessionId, MediaCaptureError> {
        require_video_capture_support()?;
        let session_id = VideoClipSessionId::from_raw(self.next_session_id()?);
        self.request(|reply| CaptureActorCommand::PrepareVideo { session_id, reply })
    }

    fn begin_video_clip_recording(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<(), MediaCaptureError> {
        require_video_capture_support()?;
        self.request(|reply| CaptureActorCommand::BeginVideoRecording { session_id, reply })
    }

    fn video_clip_status(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<VideoClipSnapshot, MediaCaptureError> {
        require_video_capture_support()?;
        self.request(|reply| CaptureActorCommand::VideoStatus { session_id, reply })
    }

    fn stop_video_clip(
        &self,
        session_id: VideoClipSessionId,
    ) -> Result<CapturedVideoClip, MediaCaptureError> {
        require_video_capture_support()?;
        self.request(|reply| CaptureActorCommand::StopVideo { session_id, reply })
    }

    fn cancel_video_clip(&self, session_id: VideoClipSessionId) -> Result<(), MediaCaptureError> {
        require_video_capture_support()?;
        self.request(|reply| CaptureActorCommand::CancelVideo { session_id, reply })
    }
}

impl Drop for NativeMediaCaptureService {
    fn drop(&mut self) {
        let (reply_tx, reply_rx) = mpsc::sync_channel(0);
        if self
            .commands
            .send(CaptureActorCommand::Shutdown { reply: reply_tx })
            .is_ok()
        {
            let _ = reply_rx.recv();
        }
        if let Some(actor_thread) = self.actor_thread.take() {
            let _ = actor_thread.join();
        }
    }
}

fn require_video_capture_support() -> Result<(), MediaCaptureError> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(MediaCaptureError::VideoCaptureUnsupported)
    }
}

pub(super) enum CaptureActorCommand {
    StartAudio {
        session_id: AudioClipSessionId,
        reply: SyncSender<Result<AudioClipSessionId, MediaCaptureError>>,
    },
    AudioStatus {
        session_id: AudioClipSessionId,
        reply: SyncSender<Result<AudioClipCaptureStatus, MediaCaptureError>>,
    },
    StopAudio {
        session_id: AudioClipSessionId,
        reply: SyncSender<Result<CapturedAudioClip, MediaCaptureError>>,
    },
    CancelAudio {
        session_id: AudioClipSessionId,
        reply: SyncSender<Result<(), MediaCaptureError>>,
    },
    PrepareVideo {
        session_id: VideoClipSessionId,
        reply: SyncSender<Result<VideoClipSessionId, MediaCaptureError>>,
    },
    BeginVideoRecording {
        session_id: VideoClipSessionId,
        reply: SyncSender<Result<(), MediaCaptureError>>,
    },
    VideoStatus {
        session_id: VideoClipSessionId,
        reply: SyncSender<Result<VideoClipSnapshot, MediaCaptureError>>,
    },
    StopVideo {
        session_id: VideoClipSessionId,
        reply: SyncSender<Result<CapturedVideoClip, MediaCaptureError>>,
    },
    CancelVideo {
        session_id: VideoClipSessionId,
        reply: SyncSender<Result<(), MediaCaptureError>>,
    },
    Shutdown {
        reply: SyncSender<()>,
    },
}
