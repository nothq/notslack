use super::{
    SlackAttachment, SlackMessageRow, SlackSidebarRow, SlackSidebarRowKind,
    SLACK_SIDEBAR_ROW_HEIGHT,
};

pub(in crate::ui::surface::state) fn slack_attachment_remote_image_urls(
    attachment: &SlackAttachment,
) -> impl Iterator<Item = &str> {
    let legacy_metadata = attachment.legacy_metadata();
    [
        attachment.preview_image_url.as_deref(),
        attachment.service_icon_url(),
        legacy_metadata.and_then(|metadata| metadata.author_avatar_image_url.as_deref()),
    ]
    .into_iter()
    .flatten()
    .chain(legacy_metadata.into_iter().flat_map(|metadata| {
        metadata
            .files
            .iter()
            .filter_map(|file| file.preview_image_url.as_deref())
    }))
}

pub(in crate::ui::surface::state) fn slack_message_reaction_remote_image_urls(
    row: &SlackMessageRow,
) -> impl Iterator<Item = &str> {
    row.reactions
        .iter()
        .flat_map(|reaction| reaction.variants.iter())
        .filter_map(|variant| variant.image_cache_key.as_deref())
}

pub(in crate::ui::surface::state) fn slack_attachment_has_remote_preview(
    attachment: &SlackAttachment,
) -> bool {
    attachment.preview_image_url.is_some()
        || attachment.legacy_metadata().is_some_and(|metadata| {
            metadata
                .files
                .iter()
                .any(|file| file.preview_image_url.is_some())
        })
}

pub(in crate::ui::surface::state) fn slack_sidebar_reveal_anchor(
    rows: &[SlackSidebarRow],
    active_index: usize,
    viewport_height: f32,
) -> usize {
    let mut anchor_index = active_index;
    let mut occupied_height = slack_sidebar_row_layout_height(&rows[active_index]);
    while let Some(previous_index) = anchor_index.checked_sub(1) {
        let previous_height = slack_sidebar_row_layout_height(&rows[previous_index]);
        if occupied_height + previous_height > viewport_height {
            break;
        }
        occupied_height += previous_height;
        anchor_index = previous_index;
    }
    anchor_index
}

pub(in crate::ui::surface::state) fn slack_sidebar_row_layout_height(row: &SlackSidebarRow) -> f32 {
    match &row.kind {
        SlackSidebarRowKind::Shortcut { .. } => SLACK_SIDEBAR_ROW_HEIGHT,
        SlackSidebarRowKind::Item { .. } => SLACK_SIDEBAR_ROW_HEIGHT,
        SlackSidebarRowKind::SectionHeader { .. } => SLACK_SIDEBAR_ROW_HEIGHT + 10.0,
        SlackSidebarRowKind::Separator => 24.0,
        SlackSidebarRowKind::Spacer { height } => *height,
        SlackSidebarRowKind::DropHint => 44.0,
    }
}

pub(in crate::ui::surface::state) fn slack_sidebar_layout_changed(
    previous: &[SlackSidebarRow],
    next: &[SlackSidebarRow],
) -> bool {
    previous.len() != next.len()
        || previous
            .iter()
            .zip(next)
            .any(|(previous, next)| !slack_sidebar_rows_share_layout(previous, next))
}

pub(in crate::ui::surface::state) fn slack_sidebar_rows_share_layout(
    previous: &SlackSidebarRow,
    next: &SlackSidebarRow,
) -> bool {
    match (&previous.kind, &next.kind) {
        (SlackSidebarRowKind::SectionHeader { .. }, SlackSidebarRowKind::SectionHeader { .. })
        | (SlackSidebarRowKind::Separator, SlackSidebarRowKind::Separator)
        | (SlackSidebarRowKind::DropHint, SlackSidebarRowKind::DropHint)
        | (SlackSidebarRowKind::Item { .. }, SlackSidebarRowKind::Item { .. }) => true,
        (
            SlackSidebarRowKind::Spacer {
                height: previous_height,
            },
            SlackSidebarRowKind::Spacer {
                height: next_height,
            },
        ) => previous_height == next_height,
        _ => false,
    }
}
