use super::{
    ChatComposerCaptureOwner, ChatComposerCaptureTarget, ChatComposerFileStatus,
    ChatVideoClipAttachmentState, ChatVideoClipAttachmentStatus, ChatVideoClipCaptureState,
    ChatVideoClipCaptureStatus, Context, SlackComposerDestination, SlackMainComposerDraftOwner,
    SlackVideoClipCaptureState, SurfaceState, VideoClipCaptureStatus,
};

impl SurfaceState {
    pub(crate) fn control_slack_video_clip_capture_state(
        &self,
        cx: &Context<Self>,
    ) -> ChatVideoClipCaptureState {
        let Some(state) = self.slack_composer_capture.video() else {
            return empty_video_clip_capture_state();
        };
        let countdown_seconds_remaining =
            self.slack_video_clip_modal_countdown_seconds(state.operation(), cx);
        let status = control_slack_video_clip_status(state, countdown_seconds_remaining);
        let snapshot = state.snapshot();
        let preview_generation = snapshot
            .and_then(|snapshot| snapshot.latest_preview.as_ref())
            .map(|preview| preview.generation().get());
        ChatVideoClipCaptureState {
            status,
            generation: Some(state.operation().generation.get()),
            owner: Some(control_capture_owner(&state.operation().owner)),
            session_id: state.session_id().map(|session_id| session_id.to_string()),
            duration_millis: u64::try_from(state.duration().as_millis())
                .expect("bounded video clip duration must fit in u64 milliseconds"),
            recording_started_at_unix_millis: state.recording_started_at_unix_millis(),
            owned_by_current_draft: self
                .slack_composer_capture_operation_is_current(state.operation()),
            preview_available: preview_generation.is_some(),
            preview_generation,
            countdown_seconds_remaining,
            attachment: self.control_slack_video_clip_attachment(state),
            diagnostic: self.control_slack_video_clip_diagnostic(state, cx),
        }
    }

    fn control_slack_video_clip_diagnostic(
        &self,
        state: &SlackVideoClipCaptureState,
        cx: &Context<Self>,
    ) -> Option<String> {
        let state_diagnostic = match state {
            SlackVideoClipCaptureState::Cancelling { terminal_error, .. } => terminal_error.clone(),
            SlackVideoClipCaptureState::Failed { diagnostic, .. } => Some(diagnostic.clone()),
            _ => None,
        };
        state_diagnostic.or_else(|| {
            self.slack_video_clip_modal
                .as_ref()
                .filter(|modal| modal.read(cx).generation() == state.operation().generation)
                .and_then(|modal| modal.read(cx).diagnostic())
        })
    }

    fn control_slack_video_clip_attachment(
        &self,
        state: &SlackVideoClipCaptureState,
    ) -> ChatVideoClipAttachmentState {
        match state {
            SlackVideoClipCaptureState::Attaching { .. } => ChatVideoClipAttachmentState {
                status: ChatVideoClipAttachmentStatus::Attaching,
                file: None,
                diagnostic: None,
            },
            SlackVideoClipCaptureState::Attached {
                operation, file_id, ..
            } => {
                let file = self
                    .slack_main_draft_files(&operation.owner)
                    .and_then(|files| files.file(*file_id))
                    .map(|file| file.control_summary());
                match file {
                    Some(file) => {
                        let (status, diagnostic) = match file.status {
                            ChatComposerFileStatus::Queued
                            | ChatComposerFileStatus::Uploading
                            | ChatComposerFileStatus::RemoteLoading => {
                                (ChatVideoClipAttachmentStatus::Staging, None)
                            }
                            ChatComposerFileStatus::Complete
                            | ChatComposerFileStatus::RemoteReady => {
                                (ChatVideoClipAttachmentStatus::Attached, None)
                            }
                            ChatComposerFileStatus::Error
                            | ChatComposerFileStatus::RetainedUnknown
                            | ChatComposerFileStatus::RemoteError => (
                                ChatVideoClipAttachmentStatus::Error,
                                file.diagnostic.clone(),
                            ),
                        };
                        ChatVideoClipAttachmentState {
                            status,
                            file: Some(file),
                            diagnostic,
                        }
                    }
                    None => ChatVideoClipAttachmentState {
                        status: ChatVideoClipAttachmentStatus::Removed,
                        file: None,
                        diagnostic: None,
                    },
                }
            }
            _ => empty_video_clip_attachment(),
        }
    }
}

fn empty_video_clip_capture_state() -> ChatVideoClipCaptureState {
    ChatVideoClipCaptureState {
        status: ChatVideoClipCaptureStatus::Idle,
        generation: None,
        owner: None,
        session_id: None,
        duration_millis: 0,
        recording_started_at_unix_millis: None,
        owned_by_current_draft: false,
        preview_available: false,
        preview_generation: None,
        countdown_seconds_remaining: None,
        attachment: empty_video_clip_attachment(),
        diagnostic: None,
    }
}

fn control_slack_video_clip_status(
    state: &SlackVideoClipCaptureState,
    countdown_seconds_remaining: Option<u8>,
) -> ChatVideoClipCaptureStatus {
    if countdown_seconds_remaining.is_some() {
        return ChatVideoClipCaptureStatus::CountingDown;
    }
    match state {
        SlackVideoClipCaptureState::Preparing { .. } => ChatVideoClipCaptureStatus::Preparing,
        SlackVideoClipCaptureState::CancellingPreparation { .. }
        | SlackVideoClipCaptureState::CancellingStart { .. }
        | SlackVideoClipCaptureState::Cancelling { .. } => ChatVideoClipCaptureStatus::Cancelling,
        SlackVideoClipCaptureState::Prepared { snapshot, .. } => {
            control_video_clip_status(&snapshot.status)
        }
        SlackVideoClipCaptureState::Starting { .. } => ChatVideoClipCaptureStatus::Starting,
        SlackVideoClipCaptureState::Recording { .. } => ChatVideoClipCaptureStatus::Recording,
        SlackVideoClipCaptureState::Finalizing { .. } => ChatVideoClipCaptureStatus::Finalizing,
        SlackVideoClipCaptureState::Reviewing { .. } => ChatVideoClipCaptureStatus::Reviewing,
        SlackVideoClipCaptureState::Attaching { .. } => ChatVideoClipCaptureStatus::Attaching,
        SlackVideoClipCaptureState::Attached { .. } => ChatVideoClipCaptureStatus::Attached,
        SlackVideoClipCaptureState::Failed { .. } => ChatVideoClipCaptureStatus::Failed,
        SlackVideoClipCaptureState::Cancelled { .. } => ChatVideoClipCaptureStatus::Cancelled,
    }
}

fn control_video_clip_status(status: &VideoClipCaptureStatus) -> ChatVideoClipCaptureStatus {
    match status {
        VideoClipCaptureStatus::RequestingPermissions => {
            ChatVideoClipCaptureStatus::RequestingPermissions
        }
        VideoClipCaptureStatus::Ready => ChatVideoClipCaptureStatus::Ready,
        VideoClipCaptureStatus::Previewing => ChatVideoClipCaptureStatus::Previewing,
        VideoClipCaptureStatus::Recording => ChatVideoClipCaptureStatus::Recording,
        VideoClipCaptureStatus::DurationLimitReached => ChatVideoClipCaptureStatus::Finalizing,
        VideoClipCaptureStatus::Failed(_) => ChatVideoClipCaptureStatus::Failed,
    }
}

fn control_capture_owner(
    handle: &crate::ui::surface::SlackMainComposerDraftHandle,
) -> ChatComposerCaptureOwner {
    let target = match &handle.owner {
        SlackMainComposerDraftOwner::Conversation(key) => {
            let SlackComposerDestination::Conversation { conversation_id } = &key.destination
            else {
                panic!("a main composer capture cannot own a thread destination");
            };
            ChatComposerCaptureTarget::Conversation {
                team_id: key.team_id.clone(),
                self_user_id: key.self_user_id.clone(),
                conversation_id: conversation_id.clone(),
            }
        }
        SlackMainComposerDraftOwner::NewMessage(key) => ChatComposerCaptureTarget::NewMessage {
            team_id: key.team_id.clone(),
            self_user_id: key.self_user_id.clone(),
            draft_key: key.draft_key.clone(),
        },
    };
    ChatComposerCaptureOwner {
        draft_id: handle.draft_id.get(),
        target,
    }
}

fn empty_video_clip_attachment() -> ChatVideoClipAttachmentState {
    ChatVideoClipAttachmentState {
        status: ChatVideoClipAttachmentStatus::None,
        file: None,
        diagnostic: None,
    }
}
