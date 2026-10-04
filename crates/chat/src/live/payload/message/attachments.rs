use std::{collections::HashMap, num::NonZeroU32};

use crate::model::{
    SlackAttachment, SlackAttachmentMedia, SlackAttachmentMediaKind, SlackAttachmentPreviewSize,
    SlackAttachmentSource, SlackLegacyAttachmentFile, SlackLegacyAttachmentMetadata,
};
use base64::Engine as _;
use serde_json::Value;

use crate::live::api::SlackApiClient;

use super::super::{
    sidebar_dom::SlackSidebarSnapshot,
    util::{
        normalize_whitespace, slack_user_avatar_image_url, slack_user_display_name, string_at,
        value_as_u32, SlackAvatarPurpose,
    },
};

pub(super) fn slack_message_attachments(
    message: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Vec<SlackAttachment> {
    let files = message
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(slack_attachment_from_file);
    let message_attachments = message
        .get("attachments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|attachment| {
            slack_attachment_from_message_attachment(attachment, users, sidebar_snapshot)
        });
    let block_attachments = message
        .get("blocks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, block)| slack_attachment_from_block_image(index, block, message));
    files
        .chain(message_attachments)
        .chain(block_attachments)
        .take(12)
        .collect()
}

pub(crate) fn load_slack_attachment_preview(
    api: &SlackApiClient,
    url: &str,
    timeout: std::time::Duration,
) -> Result<(String, String), String> {
    let (bytes, mimetype) = api.get_remote_image_bytes(url, timeout)?;
    let mimetype = mimetype
        .or_else(|| slack_preview_mimetype_from_url(url))
        .ok_or_else(|| {
            format!("failed to determine Slack attachment preview mimetype for {url}")
        })?;
    Ok((base64::prelude::BASE64_STANDARD.encode(bytes), mimetype))
}

pub(in crate::live::payload) fn slack_attachment_from_file(
    file: &Value,
) -> Option<SlackAttachment> {
    let title = string_at(file, &["title"]).or_else(|| string_at(file, &["name"]))?;
    let mimetype = string_at(file, &["mimetype"]).unwrap_or_default();
    let media = SlackAttachmentMediaKind::from_mimetype(&mimetype).and_then(|kind| {
        SlackAttachmentMedia::parse(
            string_at(file, &["id"])?,
            slack_attachment_media_content_url(file, kind)?,
            kind,
        )
        .ok()
    });
    let preview = slack_attachment_preview(file);
    Some(SlackAttachment {
        title,
        source: SlackAttachmentSource::File,
        media,
        mimetype,
        duration_millis: value_as_u32(file.get("duration_ms")).and_then(NonZeroU32::new),
        description: String::new(),
        link_url: string_at(file, &["permalink"])
            .or_else(|| string_at(file, &["url_private"]))
            .or_else(|| string_at(file, &["external_url"]))
            .unwrap_or_default(),
        source_label: String::new(),
        preview_image_url: preview.as_ref().map(|preview| preview.url.clone()),
        preview_image_base64: None,
        preview_image_mimetype: None,
        preview_layout_size: preview.and_then(|preview| preview.layout_size),
        original_width: value_as_u32(file.get("original_w")).filter(|width| *width > 0),
        original_height: value_as_u32(file.get("original_h")).filter(|height| *height > 0),
        source_team_id: string_at(file, &["source_team"]),
    })
}

fn slack_attachment_from_message_attachment(
    attachment: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackAttachment> {
    let mut mapped_attachment = slack_attachment_metadata_from_message_attachment_with_context(
        attachment,
        users,
        sidebar_snapshot,
    )?;
    mapped_attachment.preview_image_url = string_at(attachment, &["image_url"]);
    Some(mapped_attachment)
}

#[cfg(test)]
fn slack_attachment_metadata_from_message_attachment(
    attachment: &Value,
) -> Option<SlackAttachment> {
    slack_attachment_metadata_from_message_attachment_with_context(
        attachment,
        &HashMap::new(),
        None,
    )
}

fn slack_attachment_metadata_from_message_attachment_with_context(
    attachment: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackAttachment> {
    let title = slack_message_attachment_title(attachment)?;
    let link_url = string_at(attachment, &["title_link"])
        .or_else(|| string_at(attachment, &["original_url"]))
        .or_else(|| string_at(attachment, &["from_url"]))
        .unwrap_or_default();
    let description = string_at(attachment, &["text"])
        .map(normalize_whitespace)
        .filter(|text| text != &title)
        .unwrap_or_default();
    let source_label = string_at(attachment, &["service_name"])
        .or_else(|| slack_url_host_label(&link_url))
        .unwrap_or_default();
    let legacy_metadata = slack_legacy_attachment_metadata(attachment, users, sidebar_snapshot);
    Some(SlackAttachment {
        title,
        source: SlackAttachmentSource::LegacyMessage(Box::new(legacy_metadata)),
        media: None,
        mimetype: String::new(),
        duration_millis: None,
        description,
        link_url,
        source_label,
        preview_image_url: None,
        preview_image_base64: None,
        preview_image_mimetype: None,
        preview_layout_size: None,
        original_width: None,
        original_height: None,
        source_team_id: None,
    })
}

fn slack_legacy_attachment_metadata(
    attachment: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> SlackLegacyAttachmentMetadata {
    let author_id = string_at(attachment, &["author_id"]);
    let author = author_id
        .as_deref()
        .and_then(|author_id| users.get(author_id));
    let channel_id = string_at(attachment, &["channel_id"]);
    SlackLegacyAttachmentMetadata {
        service_name: string_at(attachment, &["service_name"]).unwrap_or_default(),
        service_icon_url: string_at(attachment, &["service_icon"]),
        image_width: value_as_u32(attachment.get("image_width")).filter(|width| *width > 0),
        image_height: value_as_u32(attachment.get("image_height")).filter(|height| *height > 0),
        thumbnail_url: string_at(attachment, &["thumb_url"]),
        thumbnail_width: value_as_u32(attachment.get("thumb_width")),
        thumbnail_height: value_as_u32(attachment.get("thumb_height")),
        author_name: string_at(attachment, &["author_name"])
            .or_else(|| author.and_then(slack_user_display_name))
            .unwrap_or_default(),
        author_id,
        author_avatar_image_url: string_at(attachment, &["author_icon"]).or_else(|| {
            author.and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Compact))
        }),
        channel_label: channel_id
            .as_deref()
            .and_then(|channel_id| sidebar_snapshot.and_then(|snapshot| snapshot.item(channel_id)))
            .map(|item| item.label.clone())
            .filter(|label| !label.trim().is_empty()),
        channel_id,
        source_team_id: string_at(attachment, &["source_team_id"]),
        timestamp: string_at(attachment, &["ts"]),
        permalink: string_at(attachment, &["from_url"]),
        files: attachment
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(slack_legacy_attachment_file)
            .take(12)
            .collect(),
        ..SlackLegacyAttachmentMetadata::default()
    }
}

fn slack_legacy_attachment_file(file: &Value) -> Option<SlackLegacyAttachmentFile> {
    let title = string_at(file, &["title"]).or_else(|| string_at(file, &["name"]))?;
    let mimetype = string_at(file, &["mimetype"]).unwrap_or_default();
    let media = SlackAttachmentMediaKind::from_mimetype(&mimetype).and_then(|kind| {
        SlackAttachmentMedia::parse(
            string_at(file, &["id"])?,
            slack_attachment_media_content_url(file, kind)?,
            kind,
        )
        .ok()
    });
    let preview = slack_attachment_preview(file);
    Some(SlackLegacyAttachmentFile {
        id: string_at(file, &["id"]).unwrap_or_default(),
        title,
        mimetype,
        media,
        duration_millis: value_as_u32(file.get("duration_ms")).and_then(NonZeroU32::new),
        link_url: string_at(file, &["permalink"])
            .or_else(|| string_at(file, &["url_private"]))
            .unwrap_or_default(),
        preview_image_url: preview.as_ref().map(|preview| preview.url.clone()),
        preview_image_base64: None,
        preview_image_mimetype: None,
        preview_layout_size: preview.and_then(|preview| preview.layout_size),
        original_width: value_as_u32(file.get("original_w")),
        original_height: value_as_u32(file.get("original_h")),
    })
}

fn slack_attachment_media_content_url(
    file: &Value,
    kind: SlackAttachmentMediaKind,
) -> Option<String> {
    match kind {
        SlackAttachmentMediaKind::Audio => string_at(file, &["url_private_download"]),
        SlackAttachmentMediaKind::Video => {
            string_at(file, &["url_private"]).or_else(|| string_at(file, &["url_private_download"]))
        }
    }
}

fn slack_message_attachment_title(attachment: &Value) -> Option<String> {
    string_at(attachment, &["title"])
        .or_else(|| string_at(attachment, &["fallback"]))
        .or_else(|| string_at(attachment, &["text"]))
        .map(normalize_whitespace)
}

fn slack_attachment_from_block_image(
    block_index: usize,
    block: &Value,
    message: &Value,
) -> Option<SlackAttachment> {
    if block.get("type").and_then(Value::as_str) != Some("image") {
        return None;
    }
    let preview_image_url = string_at(block, &["image_url"])?;
    let title = string_at(block, &["alt_text"])
        .or_else(|| {
            block
                .get("slack_file")
                .and_then(|file| string_at(file, &["title"]))
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| format!("Image {}", block_index + 1));
    let link_url = slack_block_image_link_url(block_index, message).unwrap_or_default();
    let size_label = value_as_u32(block.get("image_bytes"))
        .map(slack_attachment_image_size_label)
        .unwrap_or_default();
    Some(SlackAttachment {
        title,
        source: SlackAttachmentSource::BlockImage,
        media: None,
        mimetype: slack_preview_mimetype_from_url(&preview_image_url).unwrap_or_default(),
        duration_millis: None,
        description: size_label,
        link_url,
        source_label: String::new(),
        preview_image_url: Some(preview_image_url),
        preview_image_base64: None,
        preview_image_mimetype: None,
        preview_layout_size: None,
        original_width: None,
        original_height: None,
        source_team_id: None,
    })
}

fn slack_block_image_link_url(block_index: usize, message: &Value) -> Option<String> {
    let blocks = message.get("blocks").and_then(Value::as_array)?;
    blocks[..block_index].iter().rev().find_map(|block| {
        if block.get("type").and_then(Value::as_str) != Some("section") {
            return None;
        }
        let text = block
            .get("text")
            .and_then(|text| text.get("text"))
            .and_then(Value::as_str)
            .map(normalize_whitespace)?;
        let start = text.find('<')?;
        let end = text[start + 1..].find('>')? + start + 1;
        let link_text = &text[start + 1..end];
        link_text.split('|').next().map(str::to_string)
    })
}

fn slack_attachment_image_size_label(image_bytes: u32) -> String {
    if image_bytes >= 1_000_000 {
        format!("({:.1} MB)", image_bytes as f32 / 1_000_000.0)
    } else {
        format!("({} kB)", image_bytes.div_ceil(1000))
    }
}

struct SlackAttachmentPreviewSelection {
    url: String,
    layout_size: Option<SlackAttachmentPreviewSize>,
}

fn slack_attachment_preview(file: &Value) -> Option<SlackAttachmentPreviewSelection> {
    let image_file =
        string_at(file, &["mimetype"]).is_some_and(|mimetype| mimetype.starts_with("image/"));
    let url = if image_file {
        [
            string_at(file, &["thumb_720"]),
            string_at(file, &["thumb_480"]),
            string_at(file, &["thumb_360"]),
            string_at(file, &["thumb_160"]),
            string_at(file, &["thumb_80"]),
            string_at(file, &["thumb_64"]),
            string_at(file, &["url_private"]),
        ]
        .into_iter()
        .flatten()
        .next()
    } else {
        [
            string_at(file, &["thumb_video"]),
            string_at(file, &["thumb_720"]),
            string_at(file, &["thumb_480"]),
            string_at(file, &["thumb_360"]),
            string_at(file, &["thumb_160"]),
            string_at(file, &["thumb_80"]),
            string_at(file, &["thumb_64"]),
        ]
        .into_iter()
        .flatten()
        .next()
    }?;
    let layout_size = value_as_u32(file.get("thumb_360_w"))
        .zip(value_as_u32(file.get("thumb_360_h")))
        .and_then(|(width, height)| SlackAttachmentPreviewSize::new(width, height));
    Some(SlackAttachmentPreviewSelection { url, layout_size })
}

fn slack_url_host_label(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?;
    host.trim_start_matches("www.")
        .split(':')
        .next()
        .map(str::to_string)
}

fn slack_preview_mimetype_from_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg".to_string())
    } else if lower.ends_with(".png") {
        Some("image/png".to_string())
    } else if lower.ends_with(".gif") {
        Some("image/gif".to_string())
    } else if lower.ends_with(".webp") {
        Some("image/webp".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
