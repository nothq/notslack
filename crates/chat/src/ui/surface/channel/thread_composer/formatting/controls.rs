use gpui::SharedString;

use crate::ui::surface::{SlackComposerFormatAction, SlackReplyComposerTarget, SlackShellIcon};

#[derive(Clone, Copy)]
pub(super) struct SlackThreadFormatControl {
    pub(super) action: SlackComposerFormatAction,
    pub(super) icon: SlackShellIcon,
    pub(super) accessibility_label: &'static str,
    pub(super) active: bool,
    pub(super) enabled: bool,
}

pub(super) fn slack_thread_inline_format_controls(
    active: &[bool],
) -> [SlackThreadFormatControl; 4] {
    [
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Bold,
            icon: SlackShellIcon::FormatBold,
            accessibility_label: "Bold",
            active: active[SlackComposerFormatAction::Bold.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Italic,
            icon: SlackShellIcon::FormatItalic,
            accessibility_label: "Italic",
            active: active[SlackComposerFormatAction::Italic.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Underline,
            icon: SlackShellIcon::FormatUnderline,
            accessibility_label: "Underline",
            active: active[SlackComposerFormatAction::Underline.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Strikethrough,
            icon: SlackShellIcon::FormatStrikethrough,
            accessibility_label: "Strikethrough",
            active: active[SlackComposerFormatAction::Strikethrough.toolbar_index()],
            enabled: true,
        },
    ]
}

pub(super) fn slack_thread_block_format_controls(
    active: &[bool],
    link_enabled: bool,
) -> [SlackThreadFormatControl; 6] {
    [
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Link,
            icon: SlackShellIcon::FormatLink,
            accessibility_label: "Link",
            active: active[SlackComposerFormatAction::Link.toolbar_index()],
            enabled: link_enabled,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::OrderedList,
            icon: SlackShellIcon::FormatOrderedList,
            accessibility_label: "Ordered list",
            active: active[SlackComposerFormatAction::OrderedList.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::BulletedList,
            icon: SlackShellIcon::FormatBulletedList,
            accessibility_label: "Bulleted list",
            active: active[SlackComposerFormatAction::BulletedList.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Quote,
            icon: SlackShellIcon::FormatBlockquote,
            accessibility_label: "Blockquote",
            active: active[SlackComposerFormatAction::Quote.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::Code,
            icon: SlackShellIcon::FormatCode,
            accessibility_label: "Code",
            active: active[SlackComposerFormatAction::Code.toolbar_index()],
            enabled: true,
        },
        SlackThreadFormatControl {
            action: SlackComposerFormatAction::CodeBlock,
            icon: SlackShellIcon::FormatCodeBlock,
            accessibility_label: "Code block",
            active: active[SlackComposerFormatAction::CodeBlock.toolbar_index()],
            enabled: true,
        },
    ]
}

pub(super) fn slack_reply_format_bar_id(target: &SlackReplyComposerTarget) -> SharedString {
    match target {
        SlackReplyComposerTarget::ThreadPanel { .. } => "slack-thread-composer-formatting".into(),
        SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
            format!("slack-all-threads-formatting-{thread_key}").into()
        }
    }
}

pub(super) fn slack_reply_format_control_id(
    target: &SlackReplyComposerTarget,
    action: SlackComposerFormatAction,
) -> SharedString {
    let action = slack_format_action_id(action);
    match target {
        SlackReplyComposerTarget::ThreadPanel { .. } => {
            format!("slack-thread-format-{action}").into()
        }
        SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
            format!("slack-all-threads-format-{thread_key}-{action}").into()
        }
    }
}

fn slack_format_action_id(action: SlackComposerFormatAction) -> &'static str {
    match action {
        SlackComposerFormatAction::Bold => "bold",
        SlackComposerFormatAction::Italic => "italic",
        SlackComposerFormatAction::Underline => "underline",
        SlackComposerFormatAction::Strikethrough => "strikethrough",
        SlackComposerFormatAction::Link => "link",
        SlackComposerFormatAction::OrderedList => "ordered-list",
        SlackComposerFormatAction::BulletedList => "bulleted-list",
        SlackComposerFormatAction::Quote => "quote",
        SlackComposerFormatAction::Code => "code",
        SlackComposerFormatAction::CodeBlock => "code-block",
    }
}
