use super::super::{SlackComposerFormatAction, SlackShellIcon};

#[derive(Clone, Copy)]
pub(super) struct SlackComposerFormatControl {
    pub(super) action: SlackComposerFormatAction,
    pub(super) icon: SlackShellIcon,
    pub(super) accessibility_label: &'static str,
    pub(super) active: bool,
    pub(super) enabled: bool,
}

pub(super) fn slack_composer_primary_format_controls(
    active: [bool; SlackComposerFormatAction::TOOLBAR_CONTROLS.len()],
) -> [SlackComposerFormatControl; 4] {
    [
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Bold,
            icon: SlackShellIcon::FormatBold,
            accessibility_label: "Bold",
            active: active[SlackComposerFormatAction::Bold.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Italic,
            icon: SlackShellIcon::FormatItalic,
            accessibility_label: "Italic",
            active: active[SlackComposerFormatAction::Italic.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Underline,
            icon: SlackShellIcon::FormatUnderline,
            accessibility_label: "Underline",
            active: active[SlackComposerFormatAction::Underline.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Strikethrough,
            icon: SlackShellIcon::FormatStrikethrough,
            accessibility_label: "Strikethrough",
            active: active[SlackComposerFormatAction::Strikethrough.toolbar_index()],
            enabled: true,
        },
    ]
}

pub(super) fn slack_composer_secondary_format_controls(
    active: [bool; SlackComposerFormatAction::TOOLBAR_CONTROLS.len()],
    link_enabled: bool,
) -> [SlackComposerFormatControl; 6] {
    [
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Link,
            icon: SlackShellIcon::FormatLink,
            accessibility_label: "Link",
            active: active[SlackComposerFormatAction::Link.toolbar_index()],
            enabled: link_enabled,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::OrderedList,
            icon: SlackShellIcon::FormatOrderedList,
            accessibility_label: "Ordered list",
            active: active[SlackComposerFormatAction::OrderedList.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::BulletedList,
            icon: SlackShellIcon::FormatBulletedList,
            accessibility_label: "Bulleted list",
            active: active[SlackComposerFormatAction::BulletedList.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Quote,
            icon: SlackShellIcon::FormatBlockquote,
            accessibility_label: "Blockquote",
            active: active[SlackComposerFormatAction::Quote.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::Code,
            icon: SlackShellIcon::FormatCode,
            accessibility_label: "Code",
            active: active[SlackComposerFormatAction::Code.toolbar_index()],
            enabled: true,
        },
        SlackComposerFormatControl {
            action: SlackComposerFormatAction::CodeBlock,
            icon: SlackShellIcon::FormatCodeBlock,
            accessibility_label: "Code block",
            active: active[SlackComposerFormatAction::CodeBlock.toolbar_index()],
            enabled: true,
        },
    ]
}

pub(super) fn slack_composer_format_control_id(action: SlackComposerFormatAction) -> &'static str {
    match action {
        SlackComposerFormatAction::Bold => "slack-composer-format-bold",
        SlackComposerFormatAction::Italic => "slack-composer-format-italic",
        SlackComposerFormatAction::Underline => "slack-composer-format-underline",
        SlackComposerFormatAction::Strikethrough => "slack-composer-format-strikethrough",
        SlackComposerFormatAction::Link => "slack-composer-format-link",
        SlackComposerFormatAction::OrderedList => "slack-composer-format-ordered-list",
        SlackComposerFormatAction::BulletedList => "slack-composer-format-bulleted-list",
        SlackComposerFormatAction::Quote => "slack-composer-format-quote",
        SlackComposerFormatAction::Code => "slack-composer-format-code",
        SlackComposerFormatAction::CodeBlock => "slack-composer-format-code-block",
    }
}
