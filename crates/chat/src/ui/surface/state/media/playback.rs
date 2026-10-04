use std::sync::Arc;

use gpui::{AppContext as _, Entity};

use crate::ui::surface::{
    SlackAttachmentSelection, SlackAudioPlayer, SlackAudioPlayerEvent, SlackMediaHostId,
    SlackMediaPlayback, SlackMediaTarget, SurfaceState,
};
use crate::ui::{
    AudioPreviewPlayer, Context, SlackAttachmentMediaKind, VideoFrameFit, VideoPlayer,
    VideoPlayerConfig, VideoPlayerSource, WorkspaceApi,
};

struct SlackMediaPlaybackCompletion {
    generation: u64,
    target: SlackMediaTarget,
    workspace_api: Arc<dyn WorkspaceApi>,
}

struct SlackMediaPlaybackInstallation {
    target: SlackMediaTarget,
    host: SlackMediaHostId,
    prepared: crate::model::SlackPreparedMediaSource,
    workspace_api: Arc<dyn WorkspaceApi>,
}

impl SurfaceState {
    pub(super) fn prepare_slack_media_playback(
        &mut self,
        selection: &SlackAttachmentSelection,
        host: SlackMediaHostId,
        cx: &mut Context<Self>,
    ) {
        if !self
            .slack_workspace_api_capabilities
            .prepare_attachment_media
        {
            return;
        }
        let Some(media) = selection.attachment.media.clone() else {
            return;
        };
        let declared_mimetype = selection.attachment.mimetype.clone();
        let Some(target) = SlackMediaTarget::from_selection(selection) else {
            return;
        };
        if self.slack_media_playback.as_ref().is_some_and(|playback| {
            playback
                .target()
                .matches(target.attachment_id.as_ref(), target.file_id.as_ref())
        }) {
            return;
        }
        self.stop_slack_media_playback(cx);
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_media_playback_generation = self.slack_media_playback_generation.wrapping_add(1);
        let generation = self.slack_media_playback_generation;
        self.slack_media_playback = Some(SlackMediaPlayback::Loading {
            generation,
            target: target.clone(),
            host: host.clone(),
        });
        let completion = SlackMediaPlaybackCompletion {
            generation,
            target,
            workspace_api: workspace_api.clone(),
        };
        self.spawn_background_task(
            (workspace_api, media, declared_mimetype),
            cx,
            |(workspace_api, media, declared_mimetype)| {
                workspace_api.prepare_slack_attachment_media(&media, &declared_mimetype)
            },
            move |this, result, cx| {
                this.finish_slack_media_playback_load(completion, result, cx);
            },
        );
        cx.notify();
    }

    fn finish_slack_media_playback_load(
        &mut self,
        completion: SlackMediaPlaybackCompletion,
        result: Result<crate::model::SlackPreparedMediaSource, String>,
        cx: &mut Context<Self>,
    ) {
        let SlackMediaPlaybackCompletion {
            generation,
            target,
            workspace_api,
        } = completion;
        let Some(host) = self.slack_loading_media_host(generation, &target) else {
            if let Ok(prepared) = result {
                release_slack_media_source(&workspace_api, &prepared);
            }
            return;
        };
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_media_playback = Some(SlackMediaPlayback::Failed {
                    target,
                    host,
                    error: error.into(),
                });
                cx.notify();
                return;
            }
        };
        let installation = SlackMediaPlaybackInstallation {
            target,
            host,
            prepared,
            workspace_api,
        };
        match installation.target.kind {
            SlackAttachmentMediaKind::Video => {
                self.install_slack_video_playback(installation, cx);
            }
            SlackAttachmentMediaKind::Audio => {
                self.install_slack_audio_playback(installation, cx);
            }
        }
        cx.notify();
    }

    fn slack_loading_media_host(
        &self,
        generation: u64,
        target: &SlackMediaTarget,
    ) -> Option<SlackMediaHostId> {
        self.slack_media_playback
            .as_ref()
            .and_then(|playback| match playback {
                SlackMediaPlayback::Loading {
                    generation: current_generation,
                    target: current_target,
                    host,
                } if *current_generation == generation && current_target == target => {
                    Some(host.clone())
                }
                _ => None,
            })
    }

    fn install_slack_video_playback(
        &mut self,
        installation: SlackMediaPlaybackInstallation,
        cx: &mut Context<Self>,
    ) {
        let SlackMediaPlaybackInstallation {
            target,
            host,
            prepared,
            workspace_api,
        } = installation;
        let player = cx.new(|_| {
            VideoPlayer::new(VideoPlayerConfig {
                show_playlist: false,
                show_controls: true,
                muted: false,
                audio_enabled: true,
                looping: false,
                frame_fit: VideoFrameFit::Contain,
            })
        });
        let source = VideoPlayerSource::new(
            target.file_id.to_string(),
            target.title.to_string(),
            "Slack",
            prepared.url().to_string(),
            None,
        );
        player.update(cx, |player, cx| player.open_source(source, cx));
        self.slack_media_playback = Some(SlackMediaPlayback::Video {
            target,
            host,
            player,
            source: prepared,
            workspace_api,
        });
    }

    fn install_slack_audio_playback(
        &mut self,
        installation: SlackMediaPlaybackInstallation,
        cx: &mut Context<Self>,
    ) {
        let SlackMediaPlaybackInstallation {
            target,
            host,
            prepared,
            workspace_api,
        } = installation;
        let mut preview_player = AudioPreviewPlayer::new();
        match preview_player.play_source(prepared.url()) {
            Ok(()) => {
                let player = cx.new(|cx| SlackAudioPlayer::new(preview_player, target.clone(), cx));
                let subscription = cx.subscribe(
                    &player,
                    |this, player, event: &SlackAudioPlayerEvent, cx| match event {
                        SlackAudioPlayerEvent::Failed => {
                            this.handle_slack_audio_player_failed(&player, cx);
                        }
                    },
                );
                self.slack_media_playback = Some(SlackMediaPlayback::Audio {
                    target,
                    host,
                    player,
                    _subscription: subscription,
                    source: prepared,
                    workspace_api,
                });
            }
            Err(error) => {
                release_slack_media_source(&workspace_api, &prepared);
                self.slack_media_playback = Some(SlackMediaPlayback::Failed {
                    target,
                    host,
                    error: error.into(),
                });
            }
        }
    }

    fn handle_slack_audio_player_failed(
        &mut self,
        failed_player: &Entity<SlackAudioPlayer>,
        cx: &mut Context<Self>,
    ) {
        let is_current = self.slack_media_playback.as_ref().is_some_and(|playback| {
            matches!(
                playback,
                SlackMediaPlayback::Audio { player, .. } if player == failed_player
            )
        });
        if !is_current {
            return;
        }
        let Some(SlackMediaPlayback::Audio {
            target,
            host,
            player,
            source,
            workspace_api,
            ..
        }) = self.slack_media_playback.take()
        else {
            unreachable!("current Slack audio playback changed after identity check");
        };
        player.update(cx, |player, cx| player.stop(cx));
        release_slack_media_source(&workspace_api, &source);
        self.slack_media_playback = Some(SlackMediaPlayback::Failed {
            target,
            host,
            error: "Audio playback failed".into(),
        });
        cx.notify();
    }

    pub(crate) fn stop_slack_media_playback(&mut self, cx: &mut Context<Self>) {
        if self.clear_slack_media_playback(cx) {
            cx.notify();
        }
    }

    pub(super) fn clear_slack_media_playback(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(playback) = self.slack_media_playback.take() else {
            return false;
        };
        match playback {
            SlackMediaPlayback::Video {
                player,
                source,
                workspace_api,
                ..
            } => {
                player.update(cx, |player, cx| player.clear(cx));
                release_slack_media_source(&workspace_api, &source);
            }
            SlackMediaPlayback::Audio {
                player,
                source,
                workspace_api,
                ..
            } => {
                player.update(cx, |player, cx| player.stop(cx));
                release_slack_media_source(&workspace_api, &source);
            }
            SlackMediaPlayback::Loading { .. } | SlackMediaPlayback::Failed { .. } => {}
        }
        true
    }
}

fn release_slack_media_source(
    workspace_api: &Arc<dyn WorkspaceApi>,
    source: &crate::model::SlackPreparedMediaSource,
) {
    if let Err(error) = workspace_api.release_slack_attachment_media(source) {
        eprintln!("[notslack-slack-media] release_error={error}");
    }
}
