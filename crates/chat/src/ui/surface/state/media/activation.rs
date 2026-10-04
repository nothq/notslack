use gpui::SharedString;

use crate::ui::surface::{
    SlackAttachmentSelection, SlackMainRoute, SlackMainTab, SlackMediaHostId, SlackMediaPlayback,
    SlackMediaReconciliationKey, SlackMediaTarget, SlackMessageRenderContext, SurfaceState,
};
use crate::ui::{Context, SlackRailView};

use super::summaries::{slack_rows_contain_attachment_id, slack_search_rows_contain_attachment_id};

impl SurfaceState {
    pub(crate) fn reconcile_slack_attachment_playback(&mut self, cx: &mut Context<Self>) {
        let reconciliation_key = SlackMediaReconciliationKey {
            conversation_revision: self.slack_conversation_revision,
            main_route: self.slack_main_route,
            rail_view: self.slack_active_rail_view,
            main_tab: self.slack_active_tab,
            search_results_open: self.slack_search_results_open,
            search_generation: self.slack_search_generation,
            thread_generation: self.slack_thread_generation,
            thread_reply_generation: self.slack_thread_reply_generation,
            all_threads_generation: self.slack_all_threads_generation,
            activity_detail_generation: self.slack_activity_detail_generation,
            later_generation: self.slack_later_generation,
            pins_generation: self.slack_pins_generation,
        };
        if self.slack_media_reconciled_key == Some(reconciliation_key) {
            return;
        }
        self.slack_media_reconciled_key = Some(reconciliation_key);
        let expanded_attachment_missing =
            self.slack_expanded_attachment
                .as_ref()
                .is_some_and(|selection| {
                    self.resolve_slack_attachment_selection_by_id(selection.attachment_id.as_ref())
                        .is_none()
                });
        if expanded_attachment_missing {
            self.slack_expanded_attachment = None;
        }
        let playing_attachment_missing =
            self.slack_media_playback.as_ref().is_some_and(|playback| {
                self.resolve_slack_attachment_selection_by_id(playback.attachment_id())
                    .and_then(|selection| selection.attachment.media)
                    .is_none_or(|media| media.file_id() != playback.file_id())
            });
        if playing_attachment_missing {
            self.clear_slack_media_playback(cx);
        }
    }

    pub(crate) fn activate_slack_attachment_media(
        &mut self,
        selection: &SlackAttachmentSelection,
        host: SlackMediaHostId,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = SlackMediaTarget::from_selection(selection) else {
            return;
        };
        let current_playback = self.slack_media_playback.as_ref().filter(|playback| {
            playback
                .target()
                .matches(target.attachment_id.as_ref(), target.file_id.as_ref())
        });
        let current_playback_failed = current_playback.is_some_and(|playback| match playback {
            SlackMediaPlayback::Failed { .. } => true,
            SlackMediaPlayback::Video { player, .. } => {
                player.read(cx).playback_error_message().is_some()
            }
            SlackMediaPlayback::Audio { player, .. } => {
                player.read(cx).player.error_message().is_some()
            }
            SlackMediaPlayback::Loading { .. } => false,
        });
        if current_playback_failed {
            self.retry_slack_media_playback(selection, host, cx);
            return;
        }
        if current_playback.is_some() {
            let playback = self
                .slack_media_playback
                .as_mut()
                .expect("matching Slack media playback must remain present");
            if playback.host() != &host {
                playback.set_host(host);
                cx.notify();
            }
            return;
        }
        self.slack_aux_panel = None;
        self.prepare_slack_media_playback(selection, host, cx);
    }

    pub(crate) fn activate_slack_attachment_media_by_id(
        &mut self,
        attachment_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        if !self
            .slack_workspace_api_capabilities
            .prepare_attachment_media
        {
            return Err("Slack attachment media playback is unavailable".to_string());
        }
        let selection = self
            .resolve_slack_attachment_selection_by_id(attachment_id)
            .ok_or_else(|| format!("Chat has no current attachment with ID {attachment_id}"))?;
        let file_id = selection
            .attachment
            .media
            .as_ref()
            .ok_or_else(|| format!("Chat attachment {attachment_id} has no playable media"))?
            .file_id()
            .to_string();
        self.active_slack_workspace_api()
            .ok_or_else(|| "Chat has no active Slack workspace runtime".to_string())?;
        let host = self.preferred_slack_media_host(selection.attachment_id.clone());
        self.activate_slack_attachment_media(&selection, host, cx);
        Ok(file_id)
    }

    fn preferred_slack_media_host(&self, attachment_id: SharedString) -> SlackMediaHostId {
        if self
            .slack_expanded_attachment
            .as_ref()
            .is_some_and(|selection| selection.attachment_id == attachment_id)
        {
            return SlackMediaHostId::lightbox(attachment_id);
        }
        if self.slack_search_results_open
            && slack_search_rows_contain_attachment_id(
                &self.slack_search_rows,
                attachment_id.as_ref(),
            )
        {
            return SlackMediaHostId::search(self.slack_search_generation, attachment_id);
        }
        if self.slack_active_rail_view == SlackRailView::Activity {
            return SlackMediaHostId::message(SlackMessageRenderContext::Activity, attachment_id);
        }
        if self.slack_main_route == SlackMainRoute::AllThreads {
            return SlackMediaHostId::message(SlackMessageRenderContext::AllThreads, attachment_id);
        }
        if self.slack_active_rail_view == SlackRailView::Later {
            return SlackMediaHostId::message(SlackMessageRenderContext::Later, attachment_id);
        }
        if self.slack_active_tab == SlackMainTab::Pins {
            return SlackMediaHostId::message(SlackMessageRenderContext::Pins, attachment_id);
        }
        if self.slack_thread_panel.as_ref().is_some_and(|panel| {
            slack_rows_contain_attachment_id(
                std::slice::from_ref(&panel.parent_row),
                attachment_id.as_ref(),
            ) || slack_rows_contain_attachment_id(&panel.reply_rows, attachment_id.as_ref())
        }) {
            return SlackMediaHostId::message(SlackMessageRenderContext::Thread, attachment_id);
        }
        SlackMediaHostId::message(SlackMessageRenderContext::Conversation, attachment_id)
    }
}
