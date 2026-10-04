use super::{alpha, px, rgb, slack_palette, SlackConversationKind, TextInputStyle};

pub(in crate::ui::surface::state) fn slack_home_finder_input_style(
    appearance_mode: crate::ui::AppearanceMode,
) -> TextInputStyle {
    let palette = slack_palette(appearance_mode);
    TextInputStyle {
        height: px(28.0),
        min_height: px(28.0),
        padding_x: px(9.0),
        padding_y: px(4.0),
        radius: px(0.0),
        background: alpha(0x000000, 0.0),
        border: alpha(0x000000, 0.0),
        focused_border: alpha(0x000000, 0.0),
        text: rgb(palette.sidebar_header_text).into(),
        placeholder: rgb(palette.sidebar_muted_text).into(),
        selection: alpha(0x1264a3, 0.36),
        caret: rgb(palette.sidebar_header_text).into(),
        font_size: px(15.0),
        line_height: px(20.0),
        font_family: Some("Lato".into()),
    }
}

pub(in crate::ui::surface::state) fn slack_home_finder_conversation_kind(
    kind: SlackConversationKind,
) -> bool {
    matches!(
        kind,
        SlackConversationKind::Channel
            | SlackConversationKind::PrivateChannel
            | SlackConversationKind::DirectMessage
            | SlackConversationKind::GroupMessage
    )
}

pub(in crate::ui::surface::state) fn flush_slack_home_finder_section(
    section_header: Option<usize>,
    section_matches: &mut Vec<usize>,
    row_indices: &mut Vec<usize>,
    selectable_indices: &mut Vec<usize>,
) {
    if section_matches.is_empty() {
        return;
    }
    if let Some(section_header) = section_header {
        row_indices.push(section_header);
    }
    for source_index in section_matches.drain(..) {
        selectable_indices.push(row_indices.len());
        row_indices.push(source_index);
    }
}
