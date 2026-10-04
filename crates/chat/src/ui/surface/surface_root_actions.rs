mod audio_clip;
mod main_composer;
mod media;
mod thread_composer;
mod video_clip;

use super::{
    Context, SlackAttachmentPathSelection, SlackAuxPanelRowAction, SlackComposerFileId,
    SlackComposerFormatAction, SlackFilesFilter, SlackMainComposerDraftHandle, SlackMainTab,
    SlackRailView, SlackThreadDraftHandle, SurfaceRoot,
};

impl SurfaceRoot {
    #[cfg(test)]
    pub fn open_slack_help_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_aux_panel = Some(super::SlackAuxPanelState {
                title: "Slack help".to_string(),
                ..Default::default()
            });
            cx.notify();
        });
    }

    pub fn activate_slack_aux_panel_action<AppState: 'static>(
        &mut self,
        action: SlackAuxPanelRowAction,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.activate_slack_aux_panel_action(action, cx);
        });
    }

    pub fn open_slack_emoji_picker<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_emoji_picker(cx));
    }

    pub fn open_slack_mention_picker<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_mention_picker(cx));
    }

    #[cfg(test)]
    pub fn open_slack_video_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_video_panel(cx));
    }

    #[cfg(test)]
    pub fn open_slack_audio_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_audio_panel(cx));
    }

    pub fn open_slack_send_options<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_send_options(cx));
    }

    pub fn submit_slack_schedule<AppState: 'static>(
        &mut self,
        post_at_unix_seconds: i64,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatScheduleState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_submit_slack_schedule(post_at_unix_seconds, cx)
        })
    }

    pub fn edit_slack_scheduled_draft<AppState: 'static>(
        &mut self,
        draft_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatScheduleState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_edit_slack_scheduled_draft(draft_id, cx)
        })
    }

    pub fn cancel_slack_scheduled_draft<AppState: 'static>(
        &mut self,
        draft_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatScheduleState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_cancel_slack_scheduled_draft(draft_id, cx)
        })
    }

    pub fn open_slack_search_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_search_panel(cx));
    }

    pub fn search_slack_messages<AppState: 'static>(
        &mut self,
        query: &str,
        cx: &mut Context<AppState>,
    ) -> Result<String, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_search_slack_messages(query, cx)
        })
    }

    pub fn open_slack_search_result<AppState: 'static>(
        &mut self,
        result_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_open_slack_search_result(result_id, cx)
        })
    }

    pub fn open_slack_search_result_thread<AppState: 'static>(
        &mut self,
        result_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_open_slack_search_result_thread(result_id, cx)
        })
    }

    pub fn toggle_slack_message_reaction<AppState: 'static>(
        &mut self,
        message_id: &str,
        reaction_name: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatMessageReactionState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.control_toggle_slack_reaction(message_id, reaction_name, cx)
        })
    }

    pub fn open_slack_workspace_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_workspace_panel(cx));
    }

    pub fn open_slack_self_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_self_panel(cx));
    }

    pub fn open_slack_members_panel<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_members_panel(cx));
    }

    pub fn open_slack_channel_menu<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_channel_menu(cx));
    }

    #[cfg(test)]
    pub fn open_slack_attachment_menu<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.open_slack_attachment_menu(attachment_title, cx);
        });
    }

    pub fn open_slack_external_connection<AppState: 'static>(
        &mut self,
        name: &str,
        company: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.open_slack_external_connection(name, company, cx);
        });
    }

    pub fn open_slack_profile<AppState: 'static>(
        &mut self,
        user_id: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.open_slack_profile(user_id, cx));
    }

    pub fn select_slack_rail_view<AppState: 'static>(
        &mut self,
        view: SlackRailView,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.select_slack_rail_view(view, cx));
    }

    pub fn select_slack_activity_item<AppState: 'static>(
        &mut self,
        item_key: &str,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if surface.slack_active_rail_view != SlackRailView::Activity {
                return Err("Slack Activity is not active".to_string());
            }
            if !surface
                .slack_activity_visible_row_indices
                .iter()
                .filter_map(|index| surface.slack_activity_rows.get(*index))
                .any(|row| row.key.as_ref() == item_key)
            {
                return Err(format!("Slack Activity item `{item_key}` is not visible"));
            }
            surface.select_slack_activity_row(item_key, cx);
            Ok(())
        })
    }

    pub fn select_slack_tab<AppState: 'static>(
        &mut self,
        tab: SlackMainTab,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.select_slack_tab(tab, cx));
    }

    pub fn activate_slack_bookmark_folder<AppState: 'static>(
        &mut self,
        tab: crate::ui::SlackConversationTab,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.activate_slack_bookmark_folder(tab, cx);
        });
    }

    pub fn select_slack_files_filter<AppState: 'static>(
        &mut self,
        filter: SlackFilesFilter,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.select_slack_files_filter(filter, cx);
        });
    }

    pub fn toggle_slack_section<AppState: 'static>(
        &mut self,
        section_label: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_section(section_label, cx);
        });
    }

    pub fn is_slack_section_collapsed<AppState: 'static>(
        &mut self,
        section_label: &str,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.is_slack_section_collapsed(section_label)
        })
    }

    pub fn toggle_slack_formatting<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.toggle_slack_formatting(cx));
    }

    #[cfg(test)]
    pub fn toggle_slack_conversation_starred<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_test_conversation_starred = !surface.slack_test_conversation_starred;
            cx.notify();
        });
    }

    #[cfg(test)]
    pub fn is_slack_conversation_starred<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_test_conversation_starred)
    }

    #[cfg(test)]
    pub fn toggle_slack_channel_details<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_test_channel_details_visible =
                !surface.slack_test_channel_details_visible;
            if surface.slack_test_channel_details_visible {
                let title = surface
                    .slack_workspace()
                    .map(|workspace| format!("#{}", workspace.channel_name))
                    .unwrap_or_else(|| "Channel details".to_string());
                surface.slack_aux_panel = Some(super::SlackAuxPanelState {
                    title,
                    ..Default::default()
                });
            }
            cx.notify();
        });
    }

    #[cfg(test)]
    pub fn slack_channel_details_visible<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_test_channel_details_visible
        })
    }

    #[cfg(test)]
    pub fn toggle_slack_conversation_muted<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_test_conversation_muted = !surface.slack_test_conversation_muted;
            cx.notify();
        });
    }

    #[cfg(test)]
    pub fn slack_conversation_muted<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_test_conversation_muted)
    }

    #[cfg(test)]
    pub fn toggle_slack_huddle<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.slack_test_huddle_active = !surface.slack_test_huddle_active;
            if surface.slack_test_huddle_active {
                surface.slack_aux_panel = Some(super::SlackAuxPanelState {
                    title: "Huddle".to_string(),
                    ..Default::default()
                });
            }
            cx.notify();
        });
    }

    #[cfg(test)]
    pub fn slack_huddle_active<AppState: 'static>(&mut self, cx: &mut Context<AppState>) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.slack_test_huddle_active)
    }
}
