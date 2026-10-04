use gpui::{Context, EntityInputHandler, Modifiers, Window};

use super::{TextInput, TextInputEnterBehavior, TextInputMode};

impl TextInput {
    pub(super) fn handle_enter(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.marked_range.is_some() {
            return true;
        }
        if let Some(on_enter) = self.callbacks.on_enter_before_default.clone() {
            if on_enter(self.snapshot(), modifiers, window, cx) {
                return true;
            }
        }
        match (self.mode, self.enter_behavior) {
            (TextInputMode::SingleLine, _) => self.submit(modifiers, window, cx),
            (TextInputMode::Multiline { .. }, TextInputEnterBehavior::SubmitOnEnter)
                if !modifiers.shift =>
            {
                self.submit(modifiers, window, cx)
            }
            (TextInputMode::Multiline { .. }, _) => {
                self.replace_text_in_range(None, "\n", window, cx);
                true
            }
        }
    }

    fn submit(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(on_submit_with_state) = self.callbacks.on_submit_with_state.clone() {
            on_submit_with_state(self.snapshot(), modifiers, window, cx);
            return true;
        }
        let Some(on_submit) = self.callbacks.on_submit.clone() else {
            return false;
        };
        on_submit(window, cx);
        true
    }
}
