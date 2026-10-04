use gpui::KeyDownEvent;

mod controls;
mod members;
mod move_menu;
mod navigation;
mod notifications;
mod title;

pub(super) fn slack_header_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
