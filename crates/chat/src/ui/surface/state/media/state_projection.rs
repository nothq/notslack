use gpui::Entity;

use crate::ui::surface::{SlackAudioPlayer, SlackMediaPlayback, SlackMediaTarget, SurfaceState};
use crate::ui::{AudioPreviewState, Context, VideoPlayer};

impl SurfaceState {
    pub(crate) fn slack_media_playback_state(
        &self,
        cx: &Context<Self>,
    ) -> Option<crate::model::ChatMediaPlaybackState> {
        self.slack_media_playback
            .as_ref()
            .map(|playback| project_slack_media_playback(playback, cx))
    }
}

fn project_slack_media_playback(
    playback: &SlackMediaPlayback,
    cx: &Context<SurfaceState>,
) -> crate::model::ChatMediaPlaybackState {
    match playback {
        SlackMediaPlayback::Loading { target, .. } => {
            crate::model::ChatMediaPlaybackState::Loading {
                attachment_id: target.attachment_id.to_string(),
                file_id: target.file_id.to_string(),
                kind: target.kind,
            }
        }
        SlackMediaPlayback::Video { target, player, .. } => {
            project_slack_video_playback(target, player, cx)
        }
        SlackMediaPlayback::Audio { target, player, .. } => {
            project_slack_audio_playback(target, player, cx)
        }
        SlackMediaPlayback::Failed {
            target, error: _, ..
        } => crate::model::ChatMediaPlaybackState::Failed {
            attachment_id: target.attachment_id.to_string(),
            file_id: target.file_id.to_string(),
            kind: target.kind,
            error: crate::model::ChatMediaPlaybackError::Preparation,
        },
    }
}

fn project_slack_video_playback(
    target: &SlackMediaTarget,
    player: &Entity<VideoPlayer>,
    cx: &Context<SurfaceState>,
) -> crate::model::ChatMediaPlaybackState {
    let player = player.read(cx);
    crate::model::ChatMediaPlaybackState::Video {
        attachment_id: target.attachment_id.to_string(),
        file_id: target.file_id.to_string(),
        loading: player.playback_is_loading(),
        is_playing: player.playback_is_playing(),
        current_millis: slack_video_seconds_millis(player.playback_current_seconds()),
        duration_millis: player
            .playback_duration_seconds()
            .map(slack_video_seconds_millis),
        muted: player.playback_is_muted(),
        volume_percent: slack_media_volume_percent(player.playback_volume() as f64),
        error: player
            .playback_error_message()
            .map(slack_media_playback_error),
    }
}

fn project_slack_audio_playback(
    target: &SlackMediaTarget,
    player: &Entity<SlackAudioPlayer>,
    cx: &Context<SurfaceState>,
) -> crate::model::ChatMediaPlaybackState {
    let player = player.read(cx);
    let snapshot = player.player.snapshot();
    crate::model::ChatMediaPlaybackState::Audio {
        attachment_id: target.attachment_id.to_string(),
        file_id: target.file_id.to_string(),
        playback: project_slack_audio_playback_state(snapshot.state),
        current_millis: slack_audio_duration_millis(snapshot.position),
        duration_millis: (!snapshot.duration.is_zero())
            .then(|| slack_audio_duration_millis(snapshot.duration)),
        muted: snapshot.muted,
        volume_percent: slack_media_volume_percent(snapshot.volume),
        error: player.player.error_message(),
    }
}

fn project_slack_audio_playback_state(
    state: AudioPreviewState,
) -> crate::model::ChatAudioPlaybackState {
    match state {
        AudioPreviewState::Idle => crate::model::ChatAudioPlaybackState::Idle,
        AudioPreviewState::Playing => crate::model::ChatAudioPlaybackState::Playing,
        AudioPreviewState::Paused => crate::model::ChatAudioPlaybackState::Paused,
        AudioPreviewState::Finished => crate::model::ChatAudioPlaybackState::Finished,
    }
}

fn slack_video_seconds_millis(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1_000.0).round() as u64
}

fn slack_audio_duration_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn slack_media_volume_percent(volume: f64) -> u8 {
    (volume.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn slack_media_playback_error(error: &str) -> crate::model::ChatMediaPlaybackError {
    if error.contains("unsupported") {
        crate::model::ChatMediaPlaybackError::UnsupportedFormat
    } else if error.contains("HTTP")
        || error.contains("Server returned")
        || error.contains("source")
    {
        crate::model::ChatMediaPlaybackError::SourceUnavailable
    } else {
        crate::model::ChatMediaPlaybackError::Decoder
    }
}
