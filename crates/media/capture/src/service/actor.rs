use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, SyncSender},
    time::Duration,
};

use crate::{
    audio::AudioActorState, service::CaptureActorCommand, video::VideoActorState,
    AudioClipCaptureStatus, AudioClipSessionId, CapturedAudioClip, CapturedVideoClip,
    MediaCaptureError, VideoClipSessionId, VideoClipSnapshot,
};

const ACTOR_POLL_INTERVAL: Duration = Duration::from_millis(50);

enum CaptureActorState {
    Idle,
    Audio(AudioActorState),
    Video(VideoActorState),
}

impl CaptureActorState {
    fn needs_poll(&self) -> bool {
        match self {
            Self::Idle => false,
            Self::Audio(audio) => audio.needs_poll(),
            Self::Video(video) => video.needs_poll(),
        }
    }

    fn poll(&mut self) {
        *self = match std::mem::replace(self, Self::Idle) {
            Self::Audio(audio) => Self::Audio(audio.poll()),
            Self::Video(video) => Self::Video(video.poll()),
            Self::Idle => Self::Idle,
        };
    }

    fn shutdown(&mut self) {
        match std::mem::replace(self, Self::Idle) {
            Self::Audio(audio) => audio.shutdown(),
            Self::Video(video) => video.shutdown(),
            Self::Idle => {}
        }
    }
}

pub(super) fn run_capture_actor(command_rx: Receiver<CaptureActorCommand>) {
    let mut state = CaptureActorState::Idle;
    loop {
        let command = if state.needs_poll() {
            match command_rx.recv_timeout(ACTOR_POLL_INTERVAL) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => {
                    state.poll();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    state.shutdown();
                    return;
                }
            }
        } else {
            let Ok(command) = command_rx.recv() else {
                state.shutdown();
                return;
            };
            command
        };
        match command {
            CaptureActorCommand::Shutdown { reply } => {
                state.shutdown();
                let _ = reply.send(());
                return;
            }
            command => handle_command(&mut state, command),
        }
    }
}

fn handle_command(state: &mut CaptureActorState, command: CaptureActorCommand) {
    match command {
        CaptureActorCommand::StartAudio { session_id, reply } => {
            start_audio(state, session_id, reply)
        }
        CaptureActorCommand::AudioStatus { session_id, reply } => {
            let _ = reply.send(audio_status(state, session_id));
        }
        CaptureActorCommand::StopAudio { session_id, reply } => {
            let _ = reply.send(stop_audio(state, session_id));
        }
        CaptureActorCommand::CancelAudio { session_id, reply } => {
            let _ = reply.send(cancel_audio(state, session_id));
        }
        CaptureActorCommand::PrepareVideo { session_id, reply } => {
            prepare_video(state, session_id, reply)
        }
        CaptureActorCommand::BeginVideoRecording { session_id, reply } => {
            let _ = reply.send(begin_video_recording(state, session_id));
        }
        CaptureActorCommand::VideoStatus { session_id, reply } => {
            let _ = reply.send(video_status(state, session_id));
        }
        CaptureActorCommand::StopVideo { session_id, reply } => {
            let _ = reply.send(stop_video(state, session_id));
        }
        CaptureActorCommand::CancelVideo { session_id, reply } => {
            let _ = reply.send(cancel_video(state, session_id));
        }
        CaptureActorCommand::Shutdown { reply } => {
            state.shutdown();
            let _ = reply.send(());
        }
    }
}

fn start_audio(
    state: &mut CaptureActorState,
    session_id: AudioClipSessionId,
    reply: SyncSender<Result<AudioClipSessionId, MediaCaptureError>>,
) {
    let result = match state {
        CaptureActorState::Idle => AudioActorState::start(session_id),
        CaptureActorState::Audio(audio) => {
            let _ = reply.send(Err(MediaCaptureError::SessionAlreadyActive(
                audio.session_id(),
            )));
            return;
        }
        CaptureActorState::Video(video) => {
            let _ = reply.send(Err(MediaCaptureError::VideoSessionActive(
                video.session_id(),
            )));
            return;
        }
    };
    install_audio_state(state, session_id, reply, result);
}

fn install_audio_state(
    state: &mut CaptureActorState,
    session_id: AudioClipSessionId,
    reply: SyncSender<Result<AudioClipSessionId, MediaCaptureError>>,
    result: Result<AudioActorState, MediaCaptureError>,
) {
    match result {
        Ok(audio) if reply.send(Ok(session_id)).is_ok() => {
            *state = CaptureActorState::Audio(audio);
        }
        Ok(audio) => audio.shutdown(),
        Err(error) => {
            let _ = reply.send(Err(error));
        }
    }
}

fn audio_status(
    state: &CaptureActorState,
    session_id: AudioClipSessionId,
) -> Result<AudioClipCaptureStatus, MediaCaptureError> {
    match state {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveSession),
        CaptureActorState::Audio(audio) => audio.status(session_id),
        CaptureActorState::Video(video) => {
            Err(MediaCaptureError::VideoSessionActive(video.session_id()))
        }
    }
}

fn stop_audio(
    state: &mut CaptureActorState,
    session_id: AudioClipSessionId,
) -> Result<CapturedAudioClip, MediaCaptureError> {
    match std::mem::replace(state, CaptureActorState::Idle) {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveSession),
        CaptureActorState::Audio(audio) => {
            let transition = audio.stop(session_id);
            if let Some(audio) = transition.next_state {
                *state = CaptureActorState::Audio(audio);
            }
            transition.result
        }
        CaptureActorState::Video(video) => {
            let active = video.session_id();
            *state = CaptureActorState::Video(video);
            Err(MediaCaptureError::VideoSessionActive(active))
        }
    }
}

fn cancel_audio(
    state: &mut CaptureActorState,
    session_id: AudioClipSessionId,
) -> Result<(), MediaCaptureError> {
    match std::mem::replace(state, CaptureActorState::Idle) {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveSession),
        CaptureActorState::Audio(audio) => {
            let transition = audio.cancel(session_id);
            if let Some(audio) = transition.next_state {
                *state = CaptureActorState::Audio(audio);
            }
            transition.result
        }
        CaptureActorState::Video(video) => {
            let active = video.session_id();
            *state = CaptureActorState::Video(video);
            Err(MediaCaptureError::VideoSessionActive(active))
        }
    }
}

fn prepare_video(
    state: &mut CaptureActorState,
    session_id: VideoClipSessionId,
    reply: SyncSender<Result<VideoClipSessionId, MediaCaptureError>>,
) {
    let result = match state {
        CaptureActorState::Idle => VideoActorState::prepare(session_id),
        CaptureActorState::Audio(audio) => {
            let _ = reply.send(Err(MediaCaptureError::AudioSessionActive(
                audio.session_id(),
            )));
            return;
        }
        CaptureActorState::Video(video) => {
            let _ = reply.send(Err(MediaCaptureError::VideoSessionAlreadyActive(
                video.session_id(),
            )));
            return;
        }
    };
    match result {
        Ok(video) if reply.send(Ok(session_id)).is_ok() => {
            *state = CaptureActorState::Video(video);
        }
        Ok(video) => video.shutdown(),
        Err(error) => {
            let _ = reply.send(Err(error));
        }
    }
}

fn begin_video_recording(
    state: &mut CaptureActorState,
    session_id: VideoClipSessionId,
) -> Result<(), MediaCaptureError> {
    match std::mem::replace(state, CaptureActorState::Idle) {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveVideoSession),
        CaptureActorState::Audio(audio) => {
            let active = audio.session_id();
            *state = CaptureActorState::Audio(audio);
            Err(MediaCaptureError::AudioSessionActive(active))
        }
        CaptureActorState::Video(video) => {
            let transition = video.begin_recording(session_id);
            if let Some(video) = transition.next_state {
                *state = CaptureActorState::Video(video);
            }
            transition.result
        }
    }
}

fn video_status(
    state: &CaptureActorState,
    session_id: VideoClipSessionId,
) -> Result<VideoClipSnapshot, MediaCaptureError> {
    match state {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveVideoSession),
        CaptureActorState::Audio(audio) => {
            Err(MediaCaptureError::AudioSessionActive(audio.session_id()))
        }
        CaptureActorState::Video(video) => video.status(session_id),
    }
}

fn stop_video(
    state: &mut CaptureActorState,
    session_id: VideoClipSessionId,
) -> Result<CapturedVideoClip, MediaCaptureError> {
    match std::mem::replace(state, CaptureActorState::Idle) {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveVideoSession),
        CaptureActorState::Audio(audio) => {
            let active = audio.session_id();
            *state = CaptureActorState::Audio(audio);
            Err(MediaCaptureError::AudioSessionActive(active))
        }
        CaptureActorState::Video(video) => {
            let transition = video.stop(session_id);
            if let Some(video) = transition.next_state {
                *state = CaptureActorState::Video(video);
            }
            transition.result
        }
    }
}

fn cancel_video(
    state: &mut CaptureActorState,
    session_id: VideoClipSessionId,
) -> Result<(), MediaCaptureError> {
    match std::mem::replace(state, CaptureActorState::Idle) {
        CaptureActorState::Idle => Err(MediaCaptureError::NoActiveVideoSession),
        CaptureActorState::Audio(audio) => {
            let active = audio.session_id();
            *state = CaptureActorState::Audio(audio);
            Err(MediaCaptureError::AudioSessionActive(active))
        }
        CaptureActorState::Video(video) => {
            let transition = video.cancel(session_id);
            if let Some(video) = transition.next_state {
                *state = CaptureActorState::Video(video);
            }
            transition.result
        }
    }
}
