use std::time::Duration;

use crate::ui::surface::{
    SlackAttachmentSelection, SlackMediaHostId, SlackMediaPlayback, SurfaceState,
};
use crate::ui::Context;

impl SurfaceState {
    pub(crate) fn retry_slack_media_playback(
        &mut self,
        selection: &SlackAttachmentSelection,
        host: SlackMediaHostId,
        cx: &mut Context<Self>,
    ) {
        self.stop_slack_media_playback(cx);
        self.prepare_slack_media_playback(selection, host, cx);
    }

    pub(crate) fn play_slack_media(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        match self
            .slack_media_playback
            .as_ref()
            .ok_or_else(|| "Chat has no active media playback".to_string())?
        {
            SlackMediaPlayback::Video { player, .. } => {
                player.update(cx, |player, cx| player.play(cx));
                Ok(())
            }
            SlackMediaPlayback::Audio { player, .. } => {
                player.update(cx, |player, cx| player.play(cx));
                Ok(())
            }
            SlackMediaPlayback::Loading { .. } => {
                Err("Chat media playback is still preparing".to_string())
            }
            SlackMediaPlayback::Failed { .. } => Err("Chat media playback failed".to_string()),
        }
    }

    pub(crate) fn pause_slack_media(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        match self
            .slack_media_playback
            .as_ref()
            .ok_or_else(|| "Chat has no active media playback".to_string())?
        {
            SlackMediaPlayback::Video { player, .. } => {
                player.update(cx, |player, cx| player.pause(cx));
                Ok(())
            }
            SlackMediaPlayback::Audio { player, .. } => {
                player.update(cx, |player, cx| player.pause(cx));
                Ok(())
            }
            SlackMediaPlayback::Loading { .. } => {
                Err("Chat media playback is still preparing".to_string())
            }
            SlackMediaPlayback::Failed { .. } => Err("Chat media playback failed".to_string()),
        }
    }

    pub(crate) fn seek_slack_media(
        &mut self,
        position_millis: u64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self
            .slack_media_playback
            .as_ref()
            .ok_or_else(|| "Chat has no active media playback".to_string())?
        {
            SlackMediaPlayback::Video { player, .. } => {
                let duration_seconds = player
                    .read(cx)
                    .playback_duration_seconds()
                    .filter(|duration| *duration > 0.0)
                    .ok_or_else(|| "Chat video duration is not available yet".to_string())?;
                let position_seconds = position_millis as f64 / 1_000.0;
                player.update(cx, |player, cx| {
                    player.seek_to_fraction(position_seconds / duration_seconds, cx);
                });
                Ok(())
            }
            SlackMediaPlayback::Audio { player, .. } => {
                player.update(cx, |player, cx| {
                    player.seek(Duration::from_millis(position_millis), cx);
                });
                Ok(())
            }
            SlackMediaPlayback::Loading { .. } => {
                Err("Chat media playback is still preparing".to_string())
            }
            SlackMediaPlayback::Failed { .. } => Err("Chat media playback failed".to_string()),
        }
    }

    pub(crate) fn set_slack_media_muted(
        &mut self,
        muted: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self
            .slack_media_playback
            .as_ref()
            .ok_or_else(|| "Chat has no active media playback".to_string())?
        {
            SlackMediaPlayback::Video { player, .. } => {
                player.update(cx, |player, cx| player.set_muted(muted, cx));
                Ok(())
            }
            SlackMediaPlayback::Audio { player, .. } => {
                player.update(cx, |player, cx| player.set_muted(muted, cx));
                Ok(())
            }
            SlackMediaPlayback::Loading { .. } => {
                Err("Chat media playback is still preparing".to_string())
            }
            SlackMediaPlayback::Failed { .. } => Err("Chat media playback failed".to_string()),
        }
    }

    pub(crate) fn stop_slack_media(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.slack_media_playback.is_none() {
            return Err("Chat has no active media playback".to_string());
        }
        self.stop_slack_media_playback(cx);
        Ok(())
    }
}
