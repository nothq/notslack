use gpui::{KeyDownEvent, KeystrokeEvent};

pub(super) fn slack_workspace_item_key_target(
    event: &KeyDownEvent,
    index: usize,
    item_count: usize,
) -> Option<usize> {
    match event.keystroke.key.as_str() {
        "enter" | "space" if !event.keystroke.modifiers.modified() => Some(index),
        "up" | "arrowup" if !event.keystroke.modifiers.modified() => Some(index.saturating_sub(1)),
        "down" | "arrowdown" if !event.keystroke.modifiers.modified() => {
            Some((index + 1).min(item_count - 1))
        }
        "home" if !event.keystroke.modifiers.modified() => Some(0),
        "end" if !event.keystroke.modifiers.modified() => Some(item_count - 1),
        _ => None,
    }
}

pub(super) fn slack_workspace_shortcut_index(event: &KeystrokeEvent) -> Option<usize> {
    let modifiers = &event.keystroke.modifiers;
    if !modifiers.platform
        || modifiers.control
        || modifiers.alt
        || modifiers.function
        || modifiers.shift
    {
        return None;
    }
    let key = event
        .keystroke
        .key_char
        .as_deref()
        .unwrap_or(event.keystroke.key.as_str());
    match key {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        "5" => Some(4),
        "6" => Some(5),
        "7" => Some(6),
        "8" => Some(7),
        "9" => Some(8),
        _ => None,
    }
}
