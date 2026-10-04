use super::{SlackAttachmentRenderKind, SlackAttachmentRow};
use crate::ui::SlackAttachment;

pub(super) fn slack_attachment_shows_title(attachment: &SlackAttachment) -> bool {
    !attachment.title.is_empty() && !attachment.title.starts_with("Yesterday's ")
}

pub(super) fn slack_attachment_source_label(attachment: &SlackAttachment) -> String {
    if !attachment.source_label.is_empty() {
        attachment.source_label.clone()
    } else {
        slack_attachment_host_label(&attachment.link_url)
    }
}

pub(super) fn slack_attachment_display_url(url: &str) -> String {
    url.trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

pub(super) fn slack_attachment_host_label(url: &str) -> String {
    slack_attachment_display_url(url)
        .split('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

pub(super) fn slack_compact_attachment_kind_label(attachment: &SlackAttachment) -> String {
    if attachment.mimetype == "application/pdf" {
        "PDF".to_string()
    } else if attachment.mimetype.starts_with("image/") {
        "Image".to_string()
    } else if attachment.mimetype.starts_with("video/") {
        "Video".to_string()
    } else if attachment.mimetype.starts_with("audio/") {
        "Audio".to_string()
    } else {
        "File".to_string()
    }
}

pub(super) fn slack_compact_attachment_detail(
    attachment_row: &SlackAttachmentRow,
) -> Option<String> {
    let attachment = &attachment_row.attachment;
    let display_url = slack_attachment_display_url(&attachment.link_url);
    match attachment_row.kind {
        SlackAttachmentRenderKind::Recording => None,
        SlackAttachmentRenderKind::SharedMessage => None,
        SlackAttachmentRenderKind::WebsitePreview => {
            if !attachment.description.is_empty() {
                Some(attachment.description.clone())
            } else if !display_url.is_empty() {
                Some(display_url)
            } else {
                None
            }
        }
        SlackAttachmentRenderKind::FileCard => {
            if !attachment.description.is_empty() {
                Some(attachment.description.clone())
            } else if !attachment.mimetype.is_empty() {
                Some(attachment.mimetype.clone())
            } else if !display_url.is_empty() {
                Some(display_url)
            } else {
                None
            }
        }
    }
}
