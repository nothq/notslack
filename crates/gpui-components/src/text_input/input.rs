use gpui::{ClipboardItem, Context, EntityInputHandler, KeyDownEvent, Modifiers, Window};

use super::{
    sanitize_for_mode, TextInput, TextInputMode, TextInputPreMutationAction,
    TextInputPreMutationDecision, TextInputReplacementOrigin, TEXT_INPUT_AUTOMATION_IME_CANCEL_KEY,
    TEXT_INPUT_AUTOMATION_IME_COMMIT_KEY, TEXT_INPUT_AUTOMATION_IME_SET_KEY,
};

impl TextInput {
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || !self.focus_handle.is_focused(window) {
            return;
        }
        let previous_selection = self.selected_range.clone();
        let previous_selection_reversed = self.selection_reversed;
        self.handling_key_down = true;
        let handled = self.handle_key_down(event, window, cx);
        self.handling_key_down = false;
        if handled {
            self.emit_selection_change_if_needed(
                previous_selection,
                previous_selection_reversed,
                window,
                cx,
            );
            cx.stop_propagation();
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(on_key_down) = self.callbacks.on_key_down_before_default.clone() {
            if on_key_down(self.snapshot(), event, window, cx) {
                return true;
            }
        }
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        match key {
            TEXT_INPUT_AUTOMATION_IME_SET_KEY => {
                self.replace_and_mark_text_in_range(
                    None,
                    event.keystroke.key_char.as_deref().unwrap_or_default(),
                    None,
                    window,
                    cx,
                );
                return true;
            }
            TEXT_INPUT_AUTOMATION_IME_COMMIT_KEY => {
                self.replace_text_in_range(
                    None,
                    event.keystroke.key_char.as_deref().unwrap_or_default(),
                    window,
                    cx,
                );
                return true;
            }
            TEXT_INPUT_AUTOMATION_IME_CANCEL_KEY => {
                self.replace_and_mark_text_in_range(None, "", None, window, cx);
                return true;
            }
            _ => {}
        }
        if modifiers.platform {
            return self.handle_platform_key(key, modifiers, window, cx);
        }
        if modifiers.control || modifiers.function {
            return false;
        }
        if modifiers.alt {
            return self.handle_alt_key(key, modifiers, window, cx);
        }
        self.handle_editing_key(key, modifiers, window, cx)
    }

    fn handle_alt_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match key {
            "left" => {
                let offset = self.previous_word_boundary(self.cursor_offset());
                self.move_or_select_to(offset, modifiers.shift, cx);
                true
            }
            "right" => {
                let offset = self.next_word_boundary(self.cursor_offset());
                self.move_or_select_to(offset, modifiers.shift, cx);
                true
            }
            "backspace" => self.delete_previous_word(modifiers, window, cx),
            "delete" => self.delete_next_word(modifiers, window, cx),
            _ => false,
        }
    }

    fn delete_previous_word(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let cursor = self.cursor_offset();
        if self.selected_range.is_empty() && cursor == 0 {
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
            }
            return true;
        }
        let range = if self.selected_range.is_empty() {
            self.previous_word_boundary(cursor)..cursor
        } else {
            self.selected_range.clone()
        };
        self.delete_utf8_range_with_origin(
            range,
            TextInputReplacementOrigin::DeleteBackward,
            window,
            cx,
        );
        true
    }

    fn delete_next_word(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let cursor = self.cursor_offset();
        if self.selected_range.is_empty() && cursor == self.content.len() {
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
        let range = if self.selected_range.is_empty() {
            cursor..self.next_word_boundary(cursor)
        } else {
            self.selected_range.clone()
        };
        self.delete_utf8_range_with_origin(
            range,
            TextInputReplacementOrigin::DeleteForward,
            window,
            cx,
        );
        true
    }

    fn handle_platform_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let exact_platform_shift =
            modifiers.shift && !modifiers.alt && !modifiers.control && !modifiers.function;
        if exact_platform_shift {
            let callback = match key {
                "up" => self.callbacks.on_platform_shift_up_with_state.clone(),
                "down" => self.callbacks.on_platform_shift_down_with_state.clone(),
                _ => None,
            };
            if let Some(callback) = callback {
                callback(self.snapshot(), modifiers, window, cx);
                return true;
            }
        }
        match key {
            "a" => {
                let exact_platform_a =
                    !modifiers.shift && !modifiers.alt && !modifiers.control && !modifiers.function;
                if exact_platform_a && self.selected_range == (0..self.content.len()) {
                    if let Some(on_select_all_again_with_state) =
                        self.callbacks.on_select_all_again_with_state.clone()
                    {
                        on_select_all_again_with_state(self.snapshot(), modifiers, window, cx);
                        return true;
                    }
                }
                self.select_all(cx);
                true
            }
            "c" => {
                self.copy(window, cx);
                true
            }
            "x" => {
                self.cut(window, cx);
                true
            }
            "v" => {
                self.paste(window, cx);
                true
            }
            "z" if modifiers.shift => self.redo(modifiers, window, cx),
            "z" => self.undo(modifiers, window, cx),
            "left" => self.move_or_select_to_visual_line_edge(true, modifiers.shift, cx),
            "right" => self.move_or_select_to_visual_line_edge(false, modifiers.shift, cx),
            "up" => self.move_or_select_to_document_edge(true, modifiers.shift, cx),
            "down" => self.move_or_select_to_document_edge(false, modifiers.shift, cx),
            "backspace" => self.delete_to_visual_line_edge(true, modifiers, window, cx),
            "delete" => self.delete_to_visual_line_edge(false, modifiers, window, cx),
            _ => false,
        }
    }

    fn handle_editing_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match key {
            "backspace" => self.handle_backspace(modifiers, window, cx),
            "delete" => self.handle_delete(modifiers, window, cx),
            "left" => self.handle_left(modifiers, window, cx),
            "right" => self.handle_right(modifiers, window, cx),
            "up" => self.handle_up(modifiers, window, cx),
            "down" => self.handle_down(modifiers, window, cx),
            "home" => {
                if !modifiers.shift {
                    if let Some(on_home) = self.callbacks.on_home.clone() {
                        on_home(window, cx);
                        return true;
                    }
                }
                self.move_or_select_to_visual_line_edge(true, modifiers.shift, cx)
            }
            "end" => {
                if !modifiers.shift {
                    if let Some(on_end) = self.callbacks.on_end.clone() {
                        on_end(window, cx);
                        return true;
                    }
                }
                self.move_or_select_to_visual_line_edge(false, modifiers.shift, cx)
            }
            "escape" => self.handle_escape(window, cx),
            "enter" => self.handle_enter(modifiers, window, cx),
            "tab" => self.handle_tab(modifiers, window, cx),
            _ => false,
        }
    }

    fn handle_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if cx.stop_active_drag(window) {
            return true;
        }
        if let Some(on_escape) = self.callbacks.on_escape.clone() {
            on_escape(window, cx);
            return true;
        }
        false
    }

    fn handle_tab(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.marked_range.is_some() {
            return true;
        }
        if let Some(on_tab_with_state) = self.callbacks.on_tab_with_state.clone() {
            on_tab_with_state(self.snapshot(), modifiers, window, cx);
            return true;
        }
        if let Some(on_tab) = self.callbacks.on_tab.clone() {
            on_tab(window, cx);
            return true;
        }
        match self.mode {
            TextInputMode::SingleLine => false,
            TextInputMode::Multiline { .. } => {
                let tab_text = self.tab_text.to_string();
                self.replace_text_in_range(None, &tab_text, window, cx);
                true
            }
        }
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let clipboard_text = item.text();
        let text = sanitize_for_mode(self.mode, clipboard_text.as_deref().unwrap_or_default());
        let range = self.active_replacement_range();
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Paste {
                snapshot: self.snapshot(),
                range: range.clone(),
                text: text.clone(),
                item,
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            return;
        }
        if clipboard_text.is_some() {
            self.apply_utf8_replacement(range, text, window, cx);
        }
    }

    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Copy {
                snapshot: self.snapshot(),
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Cut {
                snapshot: self.snapshot(),
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.apply_utf8_replacement(self.selected_range.clone(), String::new(), window, cx);
        }
    }
}
