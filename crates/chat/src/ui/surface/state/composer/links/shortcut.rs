use super::{Context, SurfaceState};

impl SurfaceState {
    pub(crate) fn handle_slack_composer_link_shortcut(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !slack_composer_link_shortcut(event) {
            return false;
        }
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.reply_composer_focused)
        {
            self.open_slack_thread_link_dialog(cx)
        } else if self.slack_composer_focused {
            self.open_slack_composer_link_dialog(cx)
        } else {
            false
        }
    }
}

fn slack_composer_link_shortcut(event: &gpui::KeyDownEvent) -> bool {
    event.keystroke.key.eq_ignore_ascii_case("u")
        && event.keystroke.modifiers.platform
        && event.keystroke.modifiers.shift
        && !event.keystroke.modifiers.control
        && !event.keystroke.modifiers.alt
        && !event.keystroke.modifiers.function
}
