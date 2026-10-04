use crate::ui::SlackLaterFilter;
use gpui::KeyDownEvent;

pub(super) fn slack_later_filter_label(filter: SlackLaterFilter) -> &'static str {
    match filter {
        SlackLaterFilter::Saved => "In progress",
        SlackLaterFilter::Archived => "Archived",
        SlackLaterFilter::Completed => "Completed",
    }
}

pub(super) fn slack_later_thread_reply_label(reply_count: u32) -> String {
    match reply_count {
        1 => "1 reply".to_string(),
        count => format!("{count} replies"),
    }
}

pub(super) fn later_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
