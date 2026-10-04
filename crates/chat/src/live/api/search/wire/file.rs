use std::num::NonZeroU32;

use crate::model::{
    SlackAttachment, SlackAttachmentMedia, SlackAttachmentMediaKind, SlackAttachmentPreviewSize,
    SlackAttachmentSource,
};
use serde::Deserialize;

use super::parse_search_highlighted_text;

#[derive(Deserialize)]
pub(super) struct SlackSearchFileWire {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    mimetype: Option<String>,
    #[serde(default)]
    duration_ms: Option<u32>,
    #[serde(default)]
    permalink: Option<String>,
    #[serde(default)]
    url_private: Option<String>,
    #[serde(default)]
    url_private_download: Option<String>,
    #[serde(default)]
    thumb_video: Option<String>,
    #[serde(default)]
    thumb_720: Option<String>,
    #[serde(default)]
    thumb_480: Option<String>,
    #[serde(default)]
    thumb_360: Option<String>,
    #[serde(default)]
    thumb_360_w: Option<u32>,
    #[serde(default)]
    thumb_360_h: Option<u32>,
    #[serde(default)]
    thumb_160: Option<String>,
    #[serde(default)]
    thumb_80: Option<String>,
    #[serde(default)]
    original_w: Option<u32>,
    #[serde(default)]
    original_h: Option<u32>,
    #[serde(default)]
    source_team: Option<String>,
}

impl SlackSearchFileWire {
    pub(super) fn into_attachment(self) -> Result<Option<SlackAttachment>, String> {
        let title = self
            .title
            .map(|title| parse_search_highlighted_text("file title", title))
            .transpose()?;
        let name = self
            .name
            .map(|name| parse_search_highlighted_text("file name", name))
            .transpose()?;
        let Some(title) = nonempty(title).or_else(|| nonempty(name)) else {
            return Ok(None);
        };
        let mimetype = nonempty(self.mimetype).unwrap_or_default();
        let url_private = nonempty(self.url_private);
        let url_private_download = nonempty(self.url_private_download);
        let media = attachment_media(
            self.id,
            &mimetype,
            url_private.as_ref(),
            url_private_download.as_ref(),
        );
        let preview_image_url = attachment_preview(
            &mimetype,
            url_private.as_ref(),
            [
                self.thumb_video,
                self.thumb_720,
                self.thumb_480,
                self.thumb_360,
                self.thumb_160,
                self.thumb_80,
            ],
        );
        let preview_layout_size = self
            .thumb_360_w
            .zip(self.thumb_360_h)
            .and_then(|(width, height)| SlackAttachmentPreviewSize::new(width, height));
        Ok(Some(SlackAttachment {
            title,
            source: SlackAttachmentSource::File,
            media,
            mimetype,
            duration_millis: self.duration_ms.and_then(NonZeroU32::new),
            description: String::new(),
            link_url: nonempty(self.permalink)
                .or_else(|| url_private.clone())
                .or_else(|| url_private_download.clone())
                .unwrap_or_default(),
            source_label: String::new(),
            preview_image_url,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size,
            original_width: self.original_w.filter(|width| *width > 0),
            original_height: self.original_h.filter(|height| *height > 0),
            source_team_id: nonempty(self.source_team),
        }))
    }
}

fn attachment_media(
    id: Option<String>,
    mimetype: &str,
    url_private: Option<&String>,
    url_private_download: Option<&String>,
) -> Option<SlackAttachmentMedia> {
    let kind = SlackAttachmentMediaKind::from_mimetype(mimetype)?;
    let content_url = match kind {
        SlackAttachmentMediaKind::Audio => url_private_download.cloned(),
        SlackAttachmentMediaKind::Video => url_private.or(url_private_download).cloned(),
    }?;
    SlackAttachmentMedia::parse(nonempty(id)?, content_url, kind).ok()
}

fn attachment_preview(
    mimetype: &str,
    url_private: Option<&String>,
    candidates: [Option<String>; 6],
) -> Option<String> {
    candidates.into_iter().find_map(nonempty).or_else(|| {
        mimetype
            .starts_with("image/")
            .then(|| url_private.cloned())
            .flatten()
    })
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
