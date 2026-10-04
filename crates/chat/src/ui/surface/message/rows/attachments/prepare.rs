use chrono_tz::Tz;

use super::super::body::prepare_slack_message_body;
use super::super::time::{slack_local_today, SlackMessageMoment};
use crate::ui::surface::{
    slack_remote_image_cache_key, SlackAttachmentRenderKind, SlackAttachmentRow,
    SlackSharedMessageAttachmentRow, SlackSharedMessageFileRow,
};
use crate::ui::{
    initials, slack_avatar_fill, slack_legacy_attachment_file_image_cache_key, SlackAttachment,
    SlackAttachmentPreviewSize, SlackAttachmentSource, SlackLegacyAttachmentFile,
};

pub(crate) fn slack_attachment_row_with_identity(
    attachment: &SlackAttachment,
    timezone: Tz,
    team_id: Option<&str>,
    attachment_id: String,
) -> SlackAttachmentRow {
    let preview_cache_key = slack_remote_image_cache_key(attachment);
    let kind = slack_attachment_render_kind(attachment);
    let collapsible_file_preview = matches!(&attachment.source, SlackAttachmentSource::File);
    let external_file = collapsible_file_preview
        && attachment
            .source_team_id
            .as_deref()
            .zip(team_id)
            .is_some_and(|(source_team_id, team_id)| source_team_id != team_id);
    let (preview_max_width, preview_max_height) =
        slack_attachment_preview_bounds(&attachment.source, kind, external_file);
    let preview_size =
        slack_attachment_preview_size(attachment, kind, preview_max_width, preview_max_height);
    let recording_duration_label = (kind == SlackAttachmentRenderKind::Recording)
        .then_some(attachment.duration_millis)
        .flatten()
        .map(slack_duration_label)
        .map(Into::into);
    let legacy_title_body = slack_legacy_attachment_body(attachment, kind, &attachment.title);
    let legacy_description_body =
        slack_legacy_attachment_body(attachment, kind, &attachment.description);
    let shared_message = slack_shared_message_row(attachment, &attachment_id, timezone);
    let attachment = slack_sanitized_attachment(attachment);
    let title = attachment.title.clone().into();
    let shows_title = slack_attachment_shows_title_for_row(&attachment);
    SlackAttachmentRow {
        attachment,
        attachment_id: attachment_id.into(),
        title,
        shows_title,
        kind,
        recording_duration_label,
        preview_cache_key: preview_cache_key.map(Into::into),
        preview_size,
        preview_max_width,
        preview_max_height,
        collapsible_file_preview,
        shared_message,
        legacy_title_body,
        legacy_description_body,
    }
}

fn slack_duration_label(duration_millis: std::num::NonZeroU32) -> String {
    let total_seconds = (duration_millis.get() / 1_000).max(1);
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn slack_attachment_preview_bounds(
    source: &SlackAttachmentSource,
    kind: SlackAttachmentRenderKind,
    external_file: bool,
) -> (u32, u32) {
    match source {
        SlackAttachmentSource::File
            if external_file || kind == SlackAttachmentRenderKind::FileCard =>
        {
            (600, 235)
        }
        SlackAttachmentSource::File => (360, 354),
        SlackAttachmentSource::BlockImage => (360, 360),
        SlackAttachmentSource::LegacyMessage(_) => (360, 500),
    }
}

fn slack_attachment_preview_size(
    attachment: &SlackAttachment,
    kind: SlackAttachmentRenderKind,
    preview_max_width: u32,
    preview_max_height: u32,
) -> Option<SlackAttachmentPreviewSize> {
    if !matches!(&attachment.source, SlackAttachmentSource::File) {
        return None;
    }
    let original_size = attachment
        .original_width
        .zip(attachment.original_height)
        .and_then(|(width, height)| SlackAttachmentPreviewSize::new(width, height));
    let source_size = if kind == SlackAttachmentRenderKind::FileCard {
        original_size.or(attachment.preview_layout_size)
    } else {
        attachment.preview_layout_size.or(original_size)
    };
    source_size.map(|source_size| {
        slack_constrain_attachment_preview_size(source_size, preview_max_width, preview_max_height)
    })
}

fn slack_legacy_attachment_body(
    attachment: &SlackAttachment,
    kind: SlackAttachmentRenderKind,
    text: &str,
) -> Option<crate::ui::surface::SlackMessageBody> {
    (kind == SlackAttachmentRenderKind::WebsitePreview && !text.is_empty())
        .then(|| prepare_slack_message_body(text))
        .filter(|_| attachment.legacy_metadata().is_some())
}

fn slack_shared_message_row(
    attachment: &SlackAttachment,
    attachment_id: &str,
    timezone: Tz,
) -> Option<SlackSharedMessageAttachmentRow> {
    let metadata = attachment
        .legacy_metadata()
        .filter(|metadata| metadata.is_shared_message())?;
    let body = if attachment.description.is_empty() {
        attachment.title.clone()
    } else {
        attachment.description.clone()
    };
    Some(SlackSharedMessageAttachmentRow {
        author_name: metadata.author_name.clone().into(),
        author_avatar_text: initials(&metadata.author_name).into(),
        author_avatar_fill: slack_avatar_fill(&metadata.author_name),
        author_avatar_image_url: metadata.author_avatar_image_url.clone().map(Into::into),
        body: body.into(),
        channel_label: metadata.channel_label.clone().map(Into::into),
        timestamp_label: metadata
            .timestamp
            .as_deref()
            .and_then(|timestamp| slack_shared_message_timestamp_label(timestamp, timezone))
            .map(Into::into),
        permalink: metadata.permalink.clone().map(Into::into),
        files: metadata
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| slack_shared_message_file_row(attachment_id, index, file))
            .collect::<Vec<_>>()
            .into(),
    })
}

fn slack_shared_message_file_row(
    attachment_id: &str,
    index: usize,
    file: &SlackLegacyAttachmentFile,
) -> SlackSharedMessageFileRow {
    let preview_cache_key = slack_legacy_attachment_file_image_cache_key(file).map(Into::into);
    let layout_size = file.preview_layout_size.or_else(|| {
        file.original_width
            .zip(file.original_height)
            .and_then(|(width, height)| SlackAttachmentPreviewSize::new(width, height))
    });
    let (preview_width, preview_height) =
        slack_shared_message_preview_dimensions(layout_size, preview_cache_key.is_some());
    let attachment = SlackAttachment {
        title: file.title.clone(),
        source: SlackAttachmentSource::File,
        media: file.media.clone(),
        mimetype: file.mimetype.clone(),
        duration_millis: file.duration_millis,
        description: String::new(),
        link_url: file.link_url.clone(),
        source_label: String::new(),
        preview_image_url: file.preview_image_url.clone(),
        preview_image_base64: file.preview_image_base64.clone(),
        preview_image_mimetype: file.preview_image_mimetype.clone(),
        preview_layout_size: file.preview_layout_size,
        original_width: file.original_width,
        original_height: file.original_height,
        source_team_id: None,
    };
    SlackSharedMessageFileRow {
        attachment_id: format!("{attachment_id}-file-{index}").into(),
        title: file.title.clone().into(),
        attachment,
        link_url: file.link_url.clone().into(),
        preview_cache_key,
        preview_width,
        preview_height,
    }
}

fn slack_sanitized_attachment(attachment: &SlackAttachment) -> SlackAttachment {
    let mut attachment = attachment.clone();
    attachment.preview_image_base64 = None;
    attachment.preview_image_mimetype = None;
    if let Some(metadata) = attachment.source.legacy_metadata() {
        let mut metadata = metadata.clone();
        metadata.author_avatar_image_base64 = None;
        metadata.author_avatar_image_mimetype = None;
        for file in &mut metadata.files {
            file.preview_image_base64 = None;
            file.preview_image_mimetype = None;
        }
        attachment.source = SlackAttachmentSource::LegacyMessage(Box::new(metadata));
    }
    attachment
}

fn slack_attachment_shows_title_for_row(attachment: &SlackAttachment) -> bool {
    !attachment.title.is_empty() && !attachment.title.starts_with("Yesterday's ")
}

fn slack_attachment_render_kind(attachment: &SlackAttachment) -> SlackAttachmentRenderKind {
    if attachment
        .legacy_metadata()
        .is_some_and(|metadata| metadata.is_shared_message())
    {
        SlackAttachmentRenderKind::SharedMessage
    } else if attachment.legacy_metadata().is_some() {
        SlackAttachmentRenderKind::WebsitePreview
    } else if attachment.media.is_some()
        || attachment.title.contains("Screen Recording")
        || attachment.title.ends_with(".mov")
        || attachment.mimetype.starts_with("video/")
        || attachment.mimetype.starts_with("audio/")
    {
        SlackAttachmentRenderKind::Recording
    } else {
        SlackAttachmentRenderKind::FileCard
    }
}

fn slack_shared_message_preview_dimensions(
    layout_size: Option<SlackAttachmentPreviewSize>,
    has_preview: bool,
) -> (u32, u32) {
    const PREVIEW_MAX_WIDTH: u32 = 360;
    const PREVIEW_HEIGHT: u32 = 154;

    if !has_preview {
        return (0, 0);
    }
    let Some(layout_size) = layout_size else {
        return (PREVIEW_MAX_WIDTH, PREVIEW_HEIGHT);
    };
    let layout_width = layout_size.width();
    let layout_height = layout_size.height();
    let width_at_target_height =
        u64::from(layout_width) * u64::from(PREVIEW_HEIGHT) / u64::from(layout_height);
    if width_at_target_height <= u64::from(PREVIEW_MAX_WIDTH) {
        return (
            u32::try_from(width_at_target_height).unwrap_or(PREVIEW_MAX_WIDTH),
            PREVIEW_HEIGHT,
        );
    }
    let height_at_max_width =
        u64::from(layout_height) * u64::from(PREVIEW_MAX_WIDTH) / u64::from(layout_width);
    (
        PREVIEW_MAX_WIDTH,
        u32::try_from(height_at_max_width).unwrap_or(PREVIEW_HEIGHT),
    )
}

fn slack_constrain_attachment_preview_size(
    source: SlackAttachmentPreviewSize,
    max_width: u32,
    max_height: u32,
) -> SlackAttachmentPreviewSize {
    let source_width = source.width() as f32;
    let source_height = source.height() as f32;
    let scale = (max_width as f32 / source_width)
        .min(max_height as f32 / source_height)
        .min(1.0);
    let width = (source_width * scale).floor().max(1.0) as u32;
    let height = (source_height * scale).floor().max(1.0) as u32;
    SlackAttachmentPreviewSize::new(width, height)
        .expect("constrained Slack preview dimensions must stay nonzero")
}

fn slack_shared_message_timestamp_label(value: &str, timezone: Tz) -> Option<String> {
    let moment = SlackMessageMoment::parse(value, timezone)?;
    let timestamp = moment.local_timestamp;
    let today = slack_local_today(timezone);
    let date_label = if timestamp.date() == today {
        "Today".to_string()
    } else if today.previous_day() == Some(timestamp.date()) {
        "Yesterday".to_string()
    } else {
        timestamp
            .format(time::macros::format_description!(
                "[month repr:short] [day padding:none]"
            ))
            .ok()?
    };
    let time_label = timestamp
        .format(time::macros::format_description!(
            "[hour repr:12 padding:none]:[minute] [period case:upper]"
        ))
        .ok()?;
    Some(format!("{date_label} at {time_label}"))
}
