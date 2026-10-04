use std::sync::Arc;

use super::{App, Context, FocusHandle, Focusable, IntoElement, Render, SurfaceState, Window};

impl SurfaceState {
    fn focus_pending_slack_surface_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if std::mem::take(&mut self.slack_message_navigation_focus_pending) {
            window.focus(&self.focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_channel_move_menu_focus_pending) {
            window.focus(&self.slack_channel_move_menu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_header_move_focus_pending) {
            window.focus(&self.slack_header_move_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_channel_notifications_menu_focus_pending) {
            window.focus(&self.slack_channel_notifications_menu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_header_notifications_focus_pending) {
            window.focus(&self.slack_header_notifications_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_channel_menu_focus_pending) {
            window.focus(&self.slack_channel_menu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_channel_submenu_focus_pending) {
            window.focus(&self.slack_channel_submenu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_message_menu_focus_pending) {
            window.focus(&self.slack_message_menu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_skin_tone_menu_focus_pending) {
            window.focus(&self.slack_skin_tone_menu_focus_handle, cx);
        }
        if std::mem::take(&mut self.slack_conversation_tabs_overflow_focus_pending) {
            window.focus(&self.slack_conversation_tabs_overflow_focus_handle, cx);
        }
    }

    fn focus_pending_slack_text_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_pending_slack_reaction_picker_search(window, cx);
        self.restore_slack_later_reminder_focus(window, cx);
        self.focus_pending_slack_directory_search(window, cx);
        self.focus_pending_slack_auxiliary_text_inputs(window, cx);
    }

    fn focus_pending_slack_reaction_picker_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reaction_picker_search_focus = self
            .slack_reaction_picker_search_input
            .read(cx)
            .focus_handle_clone();
        if self.slack_reaction_picker.is_some() {
            if std::mem::take(&mut self.slack_reaction_picker_focus_pending) {
                window.focus(&reaction_picker_search_focus, cx);
            }
        } else {
            self.slack_reaction_picker_focus_pending = false;
            if reaction_picker_search_focus.is_focused(window) {
                window.focus(&self.focus_handle, cx);
            }
        }
    }

    fn restore_slack_later_reminder_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let reminder_focus = self
            .slack_later_reminder_input
            .read(cx)
            .focus_handle_clone();
        let reminder_visible = self.slack_later_reminder_dialog.is_some()
            && self.slack_main_route == super::SlackMainRoute::Conversation
            && self.slack_active_rail_view == super::SlackRailView::Later;
        let reminder_modal_focused = self
            .slack_later_reminder_layer_focus_handle
            .contains_focused(window, cx)
            || reminder_focus.is_focused(window)
            || self
                .slack_later_reminder_close_focus_handle
                .is_focused(window)
            || self
                .slack_later_reminder_save_focus_handle
                .is_focused(window)
            || self
                .slack_later_reminder_focus_guard_start
                .is_focused(window)
            || self.slack_later_reminder_focus_guard_end.is_focused(window);
        if !reminder_visible && reminder_modal_focused {
            window.focus(&self.focus_handle, cx);
        }
    }

    fn focus_pending_slack_directory_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory_focus = self
            .slack_directory_search_input
            .read(cx)
            .focus_handle_clone();
        if self.slack_main_route == super::SlackMainRoute::Directory {
            if std::mem::take(&mut self.slack_directory_focus_pending) {
                window.focus(&directory_focus, cx);
            }
        } else {
            self.slack_directory_focus_pending = false;
            if directory_focus.is_focused(window) {
                window.focus(&self.focus_handle, cx);
            }
        }
    }

    fn focus_pending_slack_auxiliary_text_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if std::mem::take(&mut self.slack_members_focus_pending) {
            let focus = self
                .slack_members_search_input
                .read(cx)
                .focus_handle_clone();
            window.focus(&focus, cx);
        }
        if std::mem::take(&mut self.slack_composer_link_focus_pending) {
            let focus = self.slack_link_url_input.read(cx).focus_handle_clone();
            window.focus(&focus, cx);
        }
        if std::mem::take(&mut self.slack_message_forward_focus_pending) {
            let focus = self
                .slack_message_forward_destination_input
                .read(cx)
                .focus_handle_clone();
            window.focus(&focus, cx);
        }
        if std::mem::take(&mut self.slack_message_forward_note_focus_pending) {
            let focus = self
                .slack_message_forward_note_input
                .read(cx)
                .focus_handle_clone();
            window.focus(&focus, cx);
        }
    }

    fn ensure_slack_render_observers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_slack_skin_tone_menu_focus_observer(window, cx);
        self.ensure_slack_composer_blur_observer(window, cx);
        self.ensure_slack_thread_composer_blur_observer(window, cx);
        self.ensure_slack_schedule_input_blur_observers(window, cx);
        self.ensure_slack_home_finder_focus_observers(window, cx);
        self.ensure_slack_dm_finder_focus_observers(window, cx);
        self.ensure_slack_new_message_focus_observers(window, cx);
        self.ensure_slack_later_reminder_focus_observers(window, cx);
        self.schedule_slack_sidebar_active_row_reveal(window, cx);
    }
}

impl Render for SurfaceState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.reconcile_slack_attachment_playback(cx);
        self.ensure_slack_conversation_live_sync(cx);
        self.schedule_slack_conversation_read_visibility_check(window, cx);
        self.slack_thread_read_window_active = window.is_window_active();
        self.schedule_slack_thread_read_visibility_check(window, cx);
        self.focus_pending_slack_surface_controls(window, cx);
        self.focus_pending_slack_text_inputs(window, cx);
        self.ensure_slack_render_observers(window, cx);
        if self.embedded_shell {
            self.render_chat_preview(window, cx)
        } else {
            let workspace = self
                .slack_workspace
                .clone()
                .unwrap_or_else(|| Arc::new(Self::default_slack_workspace()));
            self.render_slack_root(&workspace, window, cx)
        }
    }
}

impl Focusable for SurfaceState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
