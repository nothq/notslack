use super::{
    keystroke_input_text, Context, KeyDownEvent, SlackMainRoute, SlackRailView, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn handle_slack_escape_key(&mut self, cx: &mut Context<Self>) -> bool {
        self.dismiss_slack_message_layer(cx)
            || self.dismiss_slack_channel_layer(cx)
            || self.dismiss_slack_overlay_layer(cx)
            || self.dismiss_slack_panel_layer(cx)
    }

    fn dismiss_slack_message_layer(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_date_jump_overlay.is_some() {
            self.close_slack_date_jump_overlay(cx);
            return true;
        }
        if self.slack_message_delete_modal.is_some() {
            self.close_slack_message_delete(cx);
            return true;
        }
        if self.slack_message_forward_modal.is_some() {
            self.close_slack_message_forward(cx);
            return true;
        }
        if self.slack_pending_message_edit.is_none() {
            let edit_target = self
                .slack_message_edit
                .as_ref()
                .map(|editor| editor.read(cx).target().clone());
            if let Some(target) = edit_target {
                self.cancel_slack_message_edit(&target, cx);
                return true;
            }
        }
        if self.slack_message_menu.is_some() {
            self.close_slack_message_menu(cx);
            return true;
        }
        if self.slack_rail_menu.is_some() {
            self.slack_rail_menu = None;
            cx.notify();
            return true;
        }
        if self.slack_composer_link_dialog.is_some() {
            self.close_slack_composer_link_dialog(cx);
            return true;
        }
        false
    }

    fn dismiss_slack_channel_layer(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_channel_details_open {
            self.close_slack_channel_details(cx);
            return true;
        }
        if self.slack_members_panel_open {
            self.close_slack_members_panel(cx);
            return true;
        }
        if self.slack_channel_notifications_menu_open {
            if self.slack_channel_notifications_advanced_open {
                self.return_to_slack_channel_notifications_menu(cx);
            } else {
                self.close_slack_channel_notifications_menu(cx);
            }
            return true;
        }
        if self.slack_channel_menu_open {
            self.close_slack_channel_menu(cx);
            return true;
        }
        if self.slack_history_menu_open {
            self.close_slack_history_menu(cx);
            return true;
        }
        if self.slack_files_menu.is_some() {
            self.close_slack_files_menu(cx);
            return true;
        }
        if self.slack_active_rail_view == SlackRailView::Activity
            && !matches!(
                self.slack_activity_detail,
                crate::ui::surface::SlackActivityDetailState::Empty
            )
        {
            self.close_slack_activity_detail(cx);
            return true;
        }
        false
    }

    fn dismiss_slack_overlay_layer(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_schedule_overlay.is_some() {
            self.dismiss_slack_schedule_layer(cx);
            return true;
        }
        if self.slack_skin_tone_menu_open {
            self.close_slack_skin_tone_menu(cx);
            return true;
        }
        if self.slack_reaction_picker.is_some() {
            self.close_slack_reaction_picker(cx);
            return true;
        }
        if self.slack_search_open {
            self.close_slack_search(cx);
            return true;
        }
        if self.slack_search_results_open {
            self.close_slack_search_results(cx);
            return true;
        }
        if self.slack_dms_peek_visible {
            self.close_slack_dms_peek(cx);
            return true;
        }
        if self.slack_expanded_attachment.is_some() {
            self.slack_expanded_attachment = None;
            cx.notify();
            return true;
        }
        false
    }

    fn dismiss_slack_panel_layer(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_aux_panel.is_some() {
            self.dismiss_slack_inline_mention_picker();
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
            self.slack_mention_picker_state = None;
            cx.notify();
            return true;
        }
        if self.slack_profile_panel.is_some() {
            self.slack_profile_panel = None;
            cx.notify();
            return true;
        }
        if self.slack_thread_panel.is_some() {
            self.close_slack_thread_panel(cx);
            return true;
        }
        if self.slack_main_route == SlackMainRoute::NewMessage {
            self.leave_slack_new_message(cx);
            self.record_current_slack_conversation_history();
            return true;
        }
        if self.slack_main_route == SlackMainRoute::Directory {
            self.leave_slack_directory(cx);
            self.record_current_slack_conversation_history();
            return true;
        }
        if self.slack_composer_focused {
            self.slack_composer_focused = false;
            cx.notify();
            return true;
        }
        false
    }

    pub(in crate::ui::surface::state) fn handle_slack_backspace_key(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_search_open {
            if self.slack_search_query.is_empty() {
                return true;
            }
            let mut query = self.slack_search_query.clone();
            query.pop();
            self.set_slack_search_query(query, cx);
            return true;
        }
        if let Some((behavior, mut query)) = self.current_slack_aux_panel_query() {
            if query.is_empty() {
                return true;
            }
            query.pop();
            self.set_slack_aux_panel_query(behavior, query, cx);
            return true;
        }
        if !self.can_mutate_current_slack_send_draft()
            || !self.slack_composer_focused
            || self.slack_composer_text.is_empty()
        {
            return false;
        }
        self.slack_composer_text.pop();
        self.slack_composer_document
            .borrow_mut()
            .apply_text_edit(&self.slack_composer_text);
        self.advance_slack_send_draft_revision();
        self.slack_main_composer_draft_changed(cx);
        cx.notify();
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_enter_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_search_open {
            self.activate_selected_slack_search_option(cx);
            return true;
        }
        if self.is_slack_query_panel_open() {
            if let Some(action) = self.first_slack_aux_panel_action() {
                self.activate_slack_aux_panel_action(action, cx);
            }
            return true;
        }
        if !self.can_mutate_current_slack_send_draft() || !self.slack_composer_focused {
            return false;
        }
        if event.keystroke.modifiers.shift {
            self.slack_composer_text.push(char::from(10));
            self.slack_composer_document
                .borrow_mut()
                .apply_text_edit(&self.slack_composer_text);
            self.advance_slack_send_draft_revision();
            self.slack_main_composer_draft_changed(cx);
            self.slack_error = None;
            cx.notify();
            return true;
        }
        self.submit_slack_composer(cx);
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_text_input(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_search_open {
            let Some(input) = keystroke_input_text(event) else {
                return false;
            };
            let mut query = self.slack_search_query.clone();
            query.push_str(input);
            self.set_slack_search_query(query, cx);
            return true;
        }
        if let Some((behavior, mut query)) = self.current_slack_aux_panel_query() {
            let Some(input) = keystroke_input_text(event) else {
                return false;
            };
            query.push_str(input);
            self.set_slack_aux_panel_query(behavior, query, cx);
            return true;
        }
        if !self.can_mutate_current_slack_send_draft() || !self.slack_composer_focused {
            return false;
        }
        let Some(input) = keystroke_input_text(event) else {
            return false;
        };
        self.slack_composer_text.push_str(input);
        if !input.is_empty() {
            self.slack_composer_document
                .borrow_mut()
                .apply_text_edit(&self.slack_composer_text);
            self.advance_slack_send_draft_revision();
            self.slack_main_composer_draft_changed(cx);
        }
        self.slack_error = None;
        cx.notify();
        true
    }
}
