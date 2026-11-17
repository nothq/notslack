//! Typing with nothing focused goes into the message box, like Slack: the
//! thread's reply box when that was the last one used, otherwise the
//! conversation's.

use gpui::{EntityInputHandler, KeyDownEvent, Window};

use crate::ui::surface::{keystroke_input_text, Context, SurfaceState};

impl SurfaceState {
    /// Sends a typed character to the message box when no text field has
    /// focus and no menu or dialog is open. Returns whether it did.
    pub(crate) fn type_into_slack_composer(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(text) = keystroke_input_text(event) else {
            return false;
        };
        if text.chars().any(char::is_control)
            || gpui_components::text_input::any_text_input_focused(window)
            || self.slack_typing_layer_open()
        {
            return false;
        }
        let thread_open = self.slack_thread_panel.is_some();
        let main_available = self.has_current_slack_send_target();
        let use_thread = thread_open && (self.slack_last_composer_was_thread || !main_available);
        let input = if use_thread {
            self.slack_thread_composer_input.clone()
        } else if main_available {
            self.slack_composer_input.clone()
        } else {
            return false;
        };
        window.focus(&input.read(cx).focus_handle_clone(), cx);
        // The input reports the change back to this surface, which is being
        // updated right now, so the text goes in once this update is done.
        let text = text.to_string();
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, &text, window, cx);
            });
        });
        if use_thread {
            if let Some(panel) = self.slack_thread_panel.as_mut() {
                panel.reply_composer_focused = true;
            }
        } else {
            self.slack_composer_focused = true;
        }
        cx.notify();
        true
    }

    /// Remembers which message box was used last, for typing with nothing
    /// focused.
    pub(crate) fn track_slack_last_composer(&mut self, window: &Window, cx: &Context<Self>) {
        if self
            .slack_thread_composer_input
            .read(cx)
            .focus_handle_clone()
            .is_focused(window)
        {
            self.slack_last_composer_was_thread = true;
        } else if self
            .slack_composer_input
            .read(cx)
            .focus_handle_clone()
            .is_focused(window)
        {
            self.slack_last_composer_was_thread = false;
        }
        if self.slack_thread_panel.is_none() {
            self.slack_last_composer_was_thread = false;
        }
    }

    /// A menu, picker or dialog that keys belong to instead.
    fn slack_typing_layer_open(&self) -> bool {
        self.slack_expanded_attachment.is_some()
            || self.slack_message_menu.is_some()
            || self.slack_reaction_picker.is_some()
            || self.slack_message_forward_modal.is_some()
            || self.slack_message_delete_modal.is_some()
            || self.slack_message_edit.is_some()
            || self.slack_search_open
            || self.slack_date_jump_overlay.is_some()
            || self.slack_rail_menu.is_some()
            || self.slack_self_menu_open
            || self.slack_skin_tone_menu_open
            || self.slack_channel_menu_open
            || self.slack_channel_notifications_menu_open
            || self.slack_history_menu_open
            || self.slack_composer_link_dialog.is_some()
            || self.slack_schedule_overlay.is_some()
            || self.slack_later_reminder_dialog.is_some()
            || self.current_slack_aux_panel_query().is_some()
    }
}
