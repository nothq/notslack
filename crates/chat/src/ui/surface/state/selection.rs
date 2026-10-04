use super::{
    Context, SlackAuxPanelRow, SlackAuxPanelRowAction, SlackAuxPanelState, SlackFilesFilter,
    SlackMainTab, SlackRailView, SurfaceState,
};
use crate::ui::surface::SlackMainRoute;

mod aux_rows;

impl SurfaceState {
    pub(crate) fn focus_slack_composer(&mut self, cx: &mut Context<Self>) {
        if !self.has_current_slack_send_target() {
            return;
        }
        self.slack_reaction_picker = None;
        self.slack_dms_peek_visible = false;
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    pub(crate) fn close_slack_profile_panel(&mut self, cx: &mut Context<Self>) {
        if self.slack_profile_panel.is_none() {
            return;
        }
        self.slack_profile_panel = None;
        cx.notify();
    }

    pub(crate) fn close_slack_expanded_attachment(&mut self, cx: &mut Context<Self>) {
        let Some(expanded) = self.slack_expanded_attachment.take() else {
            return;
        };
        if self
            .slack_media_playback
            .as_ref()
            .is_some_and(|playback| playback.attachment_id() == expanded.attachment_id.as_ref())
        {
            self.stop_slack_media_playback(cx);
        }
        cx.notify();
    }

    pub(crate) fn activate_slack_rail_view(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        self.close_slack_search_results(cx);
        if self.slack_main_route == SlackMainRoute::AllThreads {
            self.leave_slack_all_threads();
            self.slack_main_route = SlackMainRoute::Conversation;
        }
        if self.slack_main_route == SlackMainRoute::NewMessage {
            self.leave_slack_new_message(cx);
        }
        if self.slack_main_route == SlackMainRoute::Directory {
            self.leave_slack_directory(cx);
        }
        if self.slack_active_tab == SlackMainTab::FilesLinks {
            self.leave_slack_conversation_files();
        }
        self.select_slack_rail_view(view, cx);
    }

    pub(crate) fn close_slack_dms_peek(&mut self, cx: &mut Context<Self>) {
        if !self.slack_dms_peek_visible {
            return;
        }
        self.slack_dms_peek_visible = false;
        cx.notify();
    }

    pub(crate) fn close_slack_dms_view(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Dms {
            return;
        }
        self.select_slack_rail_view(SlackRailView::Home, cx);
    }

    pub(crate) fn select_slack_rail_view(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() {
            return;
        }
        if !matches!(view, SlackRailView::Home | SlackRailView::Dms) {
            self.reset_slack_schedule_context(cx);
        }
        self.slack_rail_menu = None;
        self.leave_slack_bookmark_folder();
        if self.slack_main_route == SlackMainRoute::AllThreads {
            self.leave_slack_all_threads();
            self.slack_main_route = SlackMainRoute::Conversation;
        }
        if self.slack_main_route == SlackMainRoute::NewMessage {
            self.leave_slack_new_message(cx);
        }
        if self.slack_main_route == SlackMainRoute::Directory {
            self.leave_slack_directory(cx);
        }
        if view != SlackRailView::Dms {
            self.reset_slack_dm_finder();
        }
        if view != SlackRailView::Home {
            self.reset_slack_home_finder();
        }
        match view {
            SlackRailView::Activity | SlackRailView::Files | SlackRailView::Later => {
                self.select_slack_content_rail_view(view, cx);
            }
            SlackRailView::DraftsSent => {
                self.activate_slack_drafts_sent(cx);
                self.record_slack_drafts_sent_history(self.slack_drafts_sent_tab);
            }
            SlackRailView::More | SlackRailView::Admin => {
                self.select_slack_secondary_rail_view(view, cx);
            }
            SlackRailView::Home | SlackRailView::Dms => {
                self.select_slack_primary_rail_view(view, cx);
            }
        }
    }

    fn select_slack_content_rail_view(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        match view {
            SlackRailView::Activity => {
                self.leave_slack_files();
                self.leave_slack_drafts_sent();
                self.activate_slack_activity(cx);
            }
            SlackRailView::Files => {
                self.leave_slack_activity(cx);
                self.leave_slack_later();
                self.leave_slack_drafts_sent();
                self.activate_slack_files(cx);
            }
            SlackRailView::Later => {
                self.leave_slack_files();
                self.leave_slack_drafts_sent();
                self.activate_slack_later(cx);
            }
            _ => unreachable!("content rail helper requires activity, files, or later"),
        }
        self.record_slack_rail_history(view);
    }

    fn select_slack_secondary_rail_view(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.slack_dms_peek_visible = false;
        self.slack_active_rail_view = view;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        cx.notify();
    }

    fn select_slack_primary_rail_view(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        let was_loading_attachment_previews = self.should_load_slack_attachment_previews();
        self.slack_dms_peek_visible = false;
        self.slack_active_rail_view = view;
        if view == SlackRailView::Dms {
            self.slack_dm_list_state.remeasure();
        }
        self.slack_active_tab = SlackMainTab::Messages;
        self.slack_aux_panel = None;
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        if !was_loading_attachment_previews && self.should_load_slack_attachment_previews() {
            self.mark_slack_remote_image_queue_dirty();
        }
        self.queue_slack_dm_visible_images_for_current_view(cx);
        self.record_slack_rail_history(view);
        cx.notify();
    }

    pub(crate) fn select_slack_tab(&mut self, tab: SlackMainTab, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() {
            return;
        }
        let tab = if tab == SlackMainTab::BookmarkFolder {
            SlackMainTab::Messages
        } else {
            tab
        };
        if tab != SlackMainTab::Messages {
            self.reset_slack_schedule_context(cx);
        }
        self.leave_slack_bookmark_folder();
        let canvas_available = self.slack_workspace_api_capabilities.load_canvas
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.canvas_tab().is_some());
        let tab = if tab == SlackMainTab::Canvas && !canvas_available {
            SlackMainTab::Messages
        } else {
            tab
        };
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        if self.slack_main_route == SlackMainRoute::AllThreads {
            self.leave_slack_all_threads();
            self.slack_main_route = SlackMainRoute::Conversation;
        }
        if self.slack_main_route == SlackMainRoute::NewMessage {
            self.leave_slack_new_message(cx);
        }
        if self.slack_main_route == SlackMainRoute::Directory {
            self.leave_slack_directory(cx);
        }
        match tab {
            SlackMainTab::Canvas => self.select_slack_canvas_tab(cx),
            SlackMainTab::Pins => self.select_slack_pins_tab(cx),
            SlackMainTab::FilesLinks => {
                self.activate_slack_conversation_files(cx);
                self.record_current_slack_conversation_history();
            }
            SlackMainTab::Messages => self.select_slack_messages_tab(cx),
            SlackMainTab::BookmarkFolder => {
                unreachable!("bookmark folder tab was normalized to messages")
            }
        }
    }

    fn select_slack_canvas_tab(&mut self, cx: &mut Context<Self>) {
        self.leave_slack_conversation_files();
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::Canvas;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.record_current_slack_conversation_history();
        cx.notify();
    }

    fn select_slack_pins_tab(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.pins_tab().is_none())
        {
            return;
        }
        self.leave_slack_conversation_files();
        self.activate_slack_pins(cx);
        self.record_current_slack_conversation_history();
    }

    fn select_slack_messages_tab(&mut self, cx: &mut Context<Self>) {
        self.leave_slack_conversation_files();
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        let was_loading_attachment_previews = self.should_load_slack_attachment_previews();
        self.slack_dms_peek_visible = false;
        self.slack_active_tab = SlackMainTab::Messages;
        self.slack_active_rail_view = if self.slack_active_rail_view == SlackRailView::Dms {
            SlackRailView::Dms
        } else {
            SlackRailView::Home
        };
        self.slack_aux_panel = None;
        if !was_loading_attachment_previews && self.should_load_slack_attachment_previews() {
            self.mark_slack_remote_image_queue_dirty();
        }
        self.record_current_slack_conversation_history();
        cx.notify();
    }

    pub(crate) fn select_slack_dm_conversation(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_conversation {
            return;
        }
        self.reset_slack_dm_finder();
        let selected_from_peek = self.slack_dms_peek_visible;
        self.slack_dms_peek_visible = false;
        self.select_slack_conversation(conversation_id, cx);
        if !selected_from_peek {
            self.slack_active_rail_view = SlackRailView::Dms;
        }
        self.slack_active_tab = SlackMainTab::Messages;
        self.queue_slack_dm_visible_images_for_current_view(cx);
        cx.notify();
    }

    pub(crate) fn toggle_slack_dms_unread_filter(&mut self, cx: &mut Context<Self>) {
        self.slack_dms_show_unread_only = !self.slack_dms_show_unread_only;
        self.refresh_slack_dm_visible_rows();
        self.queue_slack_dm_visible_images_for_current_view(cx);
        cx.notify();
    }

    pub(crate) fn select_slack_files_filter(
        &mut self,
        filter: SlackFilesFilter,
        cx: &mut Context<Self>,
    ) {
        if self.slack_files_filter == filter {
            return;
        }
        self.slack_files_filter = filter;
        cx.notify();
    }

    pub(crate) fn toggle_slack_section(&mut self, section_label: &str, cx: &mut Context<Self>) {
        if !self
            .slack_collapsed_sections
            .insert(section_label.to_string())
        {
            self.slack_collapsed_sections.remove(section_label);
        }
        self.refresh_slack_sidebar_rows();
        cx.notify();
    }

    pub(super) fn set_slack_aux_panel(
        &mut self,
        panel: SlackAuxPanelState,
        cx: &mut Context<Self>,
    ) {
        if panel.query_behavior != Some(super::SlackAuxPanelQueryBehavior::Mention) {
            self.slack_mention_picker_state = None;
        }
        if !matches!(
            panel.query_behavior,
            Some(
                super::SlackAuxPanelQueryBehavior::Emoji
                    | super::SlackAuxPanelQueryBehavior::Mention
            )
        ) {
            self.slack_composer_aux_target = None;
        }
        self.slack_reaction_picker = None;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = Some(panel);
        self.slack_composer_focused = false;
        self.slack_error = None;
        self.slack_expanded_attachment = None;
        cx.notify();
    }
}
