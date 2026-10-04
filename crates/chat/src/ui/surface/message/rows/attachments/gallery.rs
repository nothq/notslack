use std::sync::Arc;

use crate::ui::surface::{
    SlackAttachmentLayoutRow, SlackAttachmentRenderKind, SlackAttachmentRow,
    SlackFileGalleryCellRow, SlackFileGalleryLineRow, SlackFileGalleryRow,
};
use crate::ui::SlackAttachmentMediaKind;

pub(super) fn slack_attachment_layout_rows(
    attachments: &[SlackAttachmentRow],
) -> Arc<[SlackAttachmentLayoutRow]> {
    let mut layout = Vec::with_capacity(attachments.len());
    let mut attachment_index = 0;
    while attachment_index < attachments.len() {
        if !slack_attachment_is_gallery_file(&attachments[attachment_index]) {
            layout.push(SlackAttachmentLayoutRow::Single { attachment_index });
            attachment_index += 1;
            continue;
        }

        let gallery_start = attachment_index;
        while attachment_index < attachments.len()
            && slack_attachment_is_gallery_file(&attachments[attachment_index])
        {
            attachment_index += 1;
        }
        let gallery_end = attachment_index;
        let file_count = gallery_end - gallery_start;
        if file_count == 1 {
            layout.push(SlackAttachmentLayoutRow::Single {
                attachment_index: gallery_start,
            });
            continue;
        }

        let count_label = format!("{file_count} files");
        layout.push(SlackAttachmentLayoutRow::FileGallery(SlackFileGalleryRow {
            attachment_id: format!(
                "{}-gallery",
                attachments[gallery_start].attachment_id.as_ref()
            )
            .into(),
            accessibility_label: format!("Toggle {count_label}").into(),
            count_label: count_label.into(),
            lines: slack_file_gallery_lines(attachments, gallery_start, gallery_end),
        }));
    }
    layout.into()
}

fn slack_attachment_is_gallery_file(attachment: &SlackAttachmentRow) -> bool {
    (attachment.kind == SlackAttachmentRenderKind::FileCard
        || attachment
            .attachment
            .media
            .as_ref()
            .is_some_and(|media| media.kind() == SlackAttachmentMediaKind::Video))
        && attachment.collapsible_file_preview
        && attachment.preview_cache_key.is_some()
}

fn slack_file_gallery_lines(
    attachments: &[SlackAttachmentRow],
    gallery_start: usize,
    gallery_end: usize,
) -> Arc<[SlackFileGalleryLineRow]> {
    const MAX_GALLERY_WIDTH: u64 = 936;
    const GALLERY_GAP: u64 = 8;

    let mut lines = Vec::new();
    let mut line_cells = Vec::<(usize, u64)>::new();
    let mut line_width = 0_u64;
    for (gallery_offset, attachment) in attachments[gallery_start..gallery_end].iter().enumerate() {
        let attachment_index = gallery_start + gallery_offset;
        let natural_width = slack_file_gallery_natural_width(attachment);
        let gap_width = if line_cells.is_empty() {
            0
        } else {
            GALLERY_GAP
        };
        let next_width = line_width + gap_width + natural_width;
        if !line_cells.is_empty() && next_width > MAX_GALLERY_WIDTH {
            lines.push(slack_file_gallery_line(&line_cells));
            line_cells.clear();
            line_width = 0;
        }
        if !line_cells.is_empty() {
            line_width += GALLERY_GAP;
        }
        line_width += natural_width;
        line_cells.push((attachment_index, natural_width));
    }
    if !line_cells.is_empty() {
        lines.push(slack_file_gallery_line(&line_cells));
    }
    lines.into()
}

fn slack_file_gallery_natural_width(attachment: &SlackAttachmentRow) -> u64 {
    const GALLERY_HEIGHT: u64 = 354;
    const DEFAULT_PREVIEW_WIDTH: u64 = 360;

    let Some(size) = attachment.preview_size else {
        return DEFAULT_PREVIEW_WIDTH;
    };
    let numerator = u64::from(size.width()) * GALLERY_HEIGHT;
    (numerator + u64::from(size.height()) / 2) / u64::from(size.height())
}

fn slack_file_gallery_line(cells: &[(usize, u64)]) -> SlackFileGalleryLineRow {
    const MAX_GALLERY_WIDTH: u64 = 936;
    const GALLERY_HEIGHT: u64 = 354;
    const GALLERY_GAP: u64 = 8;

    let gap_count = u64::try_from(cells.len() - 1).expect("Slack gallery file count fits u64");
    let gap_width = GALLERY_GAP * gap_count;
    let available_width = MAX_GALLERY_WIDTH - gap_width;
    let natural_width = cells.iter().map(|(_, width)| width).copied().sum::<u64>();
    let height = if natural_width <= available_width {
        GALLERY_HEIGHT
    } else {
        (GALLERY_HEIGHT * available_width / natural_width).max(1)
    };
    let cells = cells
        .iter()
        .map(
            |(attachment_index, natural_width)| SlackFileGalleryCellRow {
                attachment_index: *attachment_index,
                width: u32::try_from((natural_width * height / GALLERY_HEIGHT).max(1))
                    .expect("prepared Slack gallery cell width fits u32"),
            },
        )
        .collect::<Vec<_>>()
        .into();
    SlackFileGalleryLineRow {
        height: u32::try_from(height).expect("prepared Slack gallery line height fits u32"),
        cells,
    }
}
