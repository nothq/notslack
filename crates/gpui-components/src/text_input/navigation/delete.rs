use gpui::{Context, Modifiers, Window};

use super::super::{TextInput, TextInputReplacementOrigin};

impl TextInput {
    pub(super) fn handle_visual_line_document_edge_delete(
        &mut self,
        start: bool,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let cursor = self.cursor_offset();
        if start && cursor == 0 {
            if self.preflight_text_deletion(
                cursor..cursor,
                TextInputReplacementOrigin::DeleteBackward,
                window,
                cx,
            ) {
                return true;
            }
            if let Some(callback) = self.callbacks.on_backspace_at_start.clone() {
                callback(self.snapshot(), modifiers, window, cx);
            } else if self.content.is_empty() {
                if let Some(callback) = self.callbacks.on_backspace_when_empty.clone() {
                    callback(window, cx);
                }
            }
            return true;
        }
        if !start && cursor == self.content.len() {
            if self.preflight_text_deletion(
                cursor..cursor,
                TextInputReplacementOrigin::DeleteForward,
                window,
                cx,
            ) {
                return true;
            }
            if let Some(callback) = self.callbacks.on_delete_at_end.clone() {
                callback(self.snapshot(), modifiers, window, cx);
            }
            return true;
        }
        false
    }
}
