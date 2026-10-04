use crate::ui::surface::SlackShellIcon;
use crate::ui::{
    div, px, rgb, Div, FontWeight, KeyDownEvent, ParentElement, SlackAttachmentMediaKind, Styled,
};

pub(super) fn slack_shared_message_file_title(title: gpui::SharedString, color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .flex_shrink(1.0)
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(13.0))
        .line_height(px(19.0668))
        .font_weight(FontWeight::NORMAL)
        .text_color(rgb(color))
        .child(title)
}

pub(super) fn slack_shared_message_media_action(
    media_kind: SlackAttachmentMediaKind,
) -> (SlackShellIcon, &'static str) {
    match media_kind {
        SlackAttachmentMediaKind::Audio => (SlackShellIcon::Mic, "audio"),
        SlackAttachmentMediaKind::Video => (SlackShellIcon::Play, "video"),
    }
}

pub(super) fn slack_shared_message_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
