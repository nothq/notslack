use std::collections::HashMap;
use std::sync::Arc;

use crate::model::SlackActivityReadKind;
use crate::model::{
    ChatActivityFilter, ChatActivityReadTargetKind, ChatActivityRowSummary, ChatActivityState,
    ChatRailView, ChatReactionPickerCatalogState, ChatReactionPickerCategory,
    ChatReactionPickerOpenState, ChatReactionPickerSkinToneState, ChatReactionPickerState,
    ChatReactionPickerTarget,
};

#[cfg(test)]
use super::SlackMediaState;
use super::{
    px, workspace_host::PendingSlackNotificationNavigation, Context, Image, KeyDownEvent,
    SlackActivityFilter, SlackAuxPanelState, SlackFilesFilter, SlackMainTab,
    SlackProfilePanelState, SlackRailView, SurfaceRoot,
};

mod activity;
mod debug;

#[cfg(test)]
mod test_support;

impl SurfaceRoot {
    pub fn slack_notification_window_context<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::model::SlackNotificationWindowContext> {
        self.ensure_event_source(cx)
            .read(cx)
            .slack_notification_window_context()
    }

    pub fn slack_notification_team_badge<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::model::SlackNotificationTeamBadge> {
        self.ensure_event_source(cx)
            .read(cx)
            .slack_notification_team_badge()
    }

    pub fn scroll_message_list<AppState: 'static>(
        &mut self,
        delta: f32,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_message_list_state.scroll_by(px(delta));
            surface.mark_slack_remote_image_queue_dirty();
            cx.notify();
        });
    }

    pub fn scroll_sidebar_list<AppState: 'static>(
        &mut self,
        delta: f32,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.cancel_slack_sidebar_active_row_reveal();
            surface.slack_sidebar_list_state.scroll_by(px(delta));
            surface.mark_slack_remote_image_queue_dirty();
            cx.notify();
        });
    }

    pub fn select_conversation<AppState: 'static>(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<AppState>,
    ) {
        let conversation_id = conversation_id.to_string();
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.select_slack_conversation(&conversation_id, cx);
        });
    }

    pub fn open_slack_notification_target<AppState: 'static>(
        &mut self,
        target: crate::model::SlackNotificationTarget<'_>,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let crate::model::SlackNotificationTarget {
            team_id,
            conversation_id,
            message_timestamp,
            thread_timestamp,
            launch_uri,
        } = target;
        let route = crate::model::ChatLaunchRoute::try_from_parts(
            team_id.to_string(),
            conversation_id.to_string(),
            None,
        )?;
        let team_id = route.team_id().clone();
        message_timestamp
            .map(crate::model::SlackMessageTimestamp::parse)
            .transpose()?;
        thread_timestamp
            .map(crate::model::SlackMessageTimestamp::parse)
            .transpose()?;
        let (selection_generation, _surface) =
            self.select_workspace(team_id.clone(), Some(route), cx)?;
        let event_source = self.ensure_event_source(cx);
        let pending = PendingSlackNotificationNavigation {
            team_id,
            selection_generation,
            conversation_id: conversation_id.to_string(),
            message_timestamp: message_timestamp.map(str::to_string),
            thread_timestamp: thread_timestamp.map(str::to_string),
            launch_uri: launch_uri.map(str::to_string),
        };
        event_source.update(cx, |event_source, cx| {
            event_source.queue_notification_navigation(pending, cx)
        })
    }

    pub fn handle_slack_key_down<AppState: 'static>(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<AppState>,
    ) -> bool {
        self.handle_key_down(event, cx)
    }

    pub fn slack_workspace<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::ui::SlackWorkspace> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_workspace().cloned())
    }

    pub fn slack_search_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> crate::model::ChatSearchState {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.control_slack_search_state())
    }

    pub fn slack_message_reaction_state<AppState: 'static>(
        &mut self,
        message_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatMessageReactionState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.control_slack_reaction_state(message_id)
        })
    }

    #[cfg(test)]
    pub fn slack_media_state<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) -> SlackMediaState {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_media_state(attachment_title)
        })
    }

    pub fn slack_aux_panel<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<SlackAuxPanelState> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_aux_panel.clone())
    }

    pub fn slack_profile_panel<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<SlackProfilePanelState> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_profile_panel.clone())
    }

    pub fn slack_remote_images<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> HashMap<String, Arc<Image>> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_remote_images.snapshot())
    }

    pub fn slack_remote_image_prefetch_request<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::model::SlackRemoteImagePrefetchRequest> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_remote_image_prefetch_request()
        })
    }

    pub fn apply_slack_remote_image<AppState: 'static>(
        &mut self,
        url: &str,
        image: Arc<Image>,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.apply_slack_remote_image(url, image, cx);
        });
    }

    pub fn slack_message_list_item_count<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> usize {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_message_list_state.item_count()
        })
    }

    pub fn slack_message_list_item_visible<AppState: 'static>(
        &mut self,
        index: usize,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface
                .slack_message_list_state
                .bounds_for_item(index)
                .is_some()
        })
    }

    pub fn slack_message_chunk_count<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> usize {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_message_chunks.len())
    }

    pub fn slack_composer_text<AppState: 'static>(&mut self, cx: &mut Context<AppState>) -> String {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_composer_text.clone())
    }

    pub fn slack_schedule_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatScheduleState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.control_slack_schedule_state())
    }

    pub fn set_slack_composer_text<AppState: 'static>(
        &mut self,
        text: String,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if surface.slack_schedule_blocks_current_composer_mutation() {
                return Err(
                    "Wait for the pending Slack scheduled-draft mutation to finish.".to_string(),
                );
            }
            if surface.replace_slack_send_draft_with_plain_text(text) {
                surface.slack_main_composer_draft_changed(cx);
                cx.notify();
            }
            Ok(())
        })
    }

    pub fn slack_composer_focused<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_composer_focused)
    }

    pub fn slack_composer_files<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<crate::model::ChatComposerFileSummary> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_composer_files.control_summaries()
        })
    }

    pub fn slack_draft_attachment_count<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> usize {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_composer_files.len())
    }

    pub fn slack_draft_attachment_mimetypes<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface
                .slack_composer_files
                .iter()
                .map(|file| file.attachment().mimetype.clone())
                .collect()
        })
    }

    pub fn slack_draft_attachments_empty<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_composer_files.is_empty())
    }

    pub fn slack_draft_upload_files_empty<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            !surface.slack_composer_files.has_local_pending()
        })
    }

    pub fn slack_scheduled_message_count<AppState: 'static>(
        &mut self,
        _cx: &mut Context<AppState>,
    ) -> usize {
        0
    }

    pub fn slack_active_rail_view<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> SlackRailView {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_active_rail_view)
    }

    pub fn slack_active_tab<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> SlackMainTab {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_active_tab)
    }

    pub fn slack_files_filter<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> SlackFilesFilter {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_files_filter)
    }

    pub fn slack_formatting_enabled<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_formatting_enabled)
    }
}
