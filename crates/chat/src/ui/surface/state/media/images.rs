use std::collections::HashSet;
use std::sync::Arc;

use crate::ui::surface::{
    build_slack_remote_image_from_bytes, SlackAttachmentSelection, SlackMediaHostId, SurfaceState,
};
#[cfg(test)]
use crate::ui::surface::{SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState};
use crate::ui::Context;

use super::super::{SLACK_REMOTE_IMAGE_LOAD_CONCURRENCY, SLACK_REMOTE_IMAGE_PENDING_LIMIT};

impl SurfaceState {
    #[cfg(test)]
    pub(crate) fn open_slack_video_panel(&mut self, cx: &mut Context<Self>) {
        self.set_slack_aux_panel(
            SlackAuxPanelState {
                title: "Clips and huddles".to_string(),
                subtitle: Some("Share a clip or manage the live huddle".to_string()),
                query: None,
                query_behavior: None,
                sections: vec![SlackAuxPanelSection {
                    title: Some("Share".to_string()),
                    rows: vec![Self::slack_aux_row(
                        "Record a clip",
                        Some("Attach a fresh screen recording preview to this draft.".to_string()),
                        None,
                        false,
                        Some(SlackAuxPanelRowAction::AttachComposerAttachment(Box::new(
                            self.slack_clip_attachment_template(),
                        ))),
                    )],
                }],
            },
            cx,
        );
    }

    #[cfg(test)]
    pub(crate) fn open_slack_audio_panel(&mut self, cx: &mut Context<Self>) {
        self.set_slack_aux_panel(
            SlackAuxPanelState {
                title: "Record audio".to_string(),
                subtitle: Some("Share a voice clip in the current draft".to_string()),
                query: None,
                query_behavior: None,
                sections: vec![SlackAuxPanelSection {
                    title: Some("Recording".to_string()),
                    rows: vec![Self::slack_aux_row(
                        "Record voice clip",
                        Some("Attach a fresh audio note to the current draft.".to_string()),
                        None,
                        false,
                        Some(SlackAuxPanelRowAction::AttachComposerAttachment(Box::new(
                            self.slack_audio_attachment_template(),
                        ))),
                    )],
                }],
            },
            cx,
        );
    }

    pub(crate) fn rebuild_slack_remote_image_queue(&mut self) {
        let mut seen = HashSet::<String>::new();
        let mut remote_image_urls = self
            .slack_workspace()
            .into_iter()
            .flat_map(|workspace| self.slack_workspace_remote_image_urls(workspace))
            .filter(|url| !self.slack_remote_images.contains_key(url))
            .filter(|url| seen.insert(url.clone()))
            .collect::<Vec<_>>();
        remote_image_urls.truncate(SLACK_REMOTE_IMAGE_PENDING_LIMIT);
        remote_image_urls.reverse();
        self.slack_pending_remote_image_urls = remote_image_urls;
        self.slack_remote_image_queue_dirty = false;
    }

    pub(crate) fn ensure_slack_remote_image_loads(&mut self, cx: &mut Context<Self>) {
        if !self.slack_pending_remote_image_urls.is_empty() {
            self.start_next_slack_remote_image_load(cx);
            return;
        }
        if !self.slack_remote_image_queue_dirty {
            return;
        }
        self.rebuild_slack_remote_image_queue();
        self.start_next_slack_remote_image_load(cx);
    }

    pub(crate) fn start_next_slack_remote_image_load(&mut self, cx: &mut Context<Self>) {
        let pending_excess = self
            .slack_pending_remote_image_urls
            .len()
            .saturating_sub(SLACK_REMOTE_IMAGE_PENDING_LIMIT);
        self.slack_pending_remote_image_urls.drain(..pending_excess);
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        while self.slack_active_remote_image_urls.len() < SLACK_REMOTE_IMAGE_LOAD_CONCURRENCY {
            let Some(url) = self.slack_pending_remote_image_urls.pop() else {
                return;
            };
            if self.slack_active_remote_image_urls.contains(&url) {
                continue;
            }
            self.slack_active_remote_image_urls.insert(url.clone());
            let workspace_api = workspace_api.clone();
            self.spawn_background_task(
                url.clone(),
                cx,
                move |url| {
                    workspace_api.load_slack_remote_image(&url).map(|image| {
                        image.and_then(|image| {
                            build_slack_remote_image_from_bytes(image.bytes, &image.mimetype)
                                .map(Arc::new)
                        })
                    })
                },
                move |this, result, cx| {
                    this.slack_active_remote_image_urls.remove(url.as_str());
                    let Ok(Some(image)) = result else {
                        cx.notify();
                        return;
                    };
                    this.apply_slack_remote_image(&url, image, cx);
                },
            );
        }
    }

    pub(crate) fn toggle_slack_attachment_expanded(
        &mut self,
        selection: &SlackAttachmentSelection,
        cx: &mut Context<Self>,
    ) {
        let was_loading_attachment_previews = self.should_load_slack_attachment_previews();
        if self
            .slack_expanded_attachment
            .as_ref()
            .is_some_and(|expanded| expanded.attachment_id == selection.attachment_id)
        {
            self.slack_expanded_attachment = None;
            if self.slack_media_playback.as_ref().is_some_and(|playback| {
                playback.attachment_id() == selection.attachment_id.as_ref()
            }) {
                self.stop_slack_media_playback(cx);
            }
        } else {
            self.slack_expanded_attachment = Some(selection.clone());
            self.slack_aux_panel = None;
            self.activate_slack_attachment_media(
                selection,
                SlackMediaHostId::lightbox(selection.attachment_id.clone()),
                cx,
            );
        }
        if !was_loading_attachment_previews && self.should_load_slack_attachment_previews() {
            self.mark_slack_remote_image_queue_dirty();
        }
        cx.notify();
    }
}
