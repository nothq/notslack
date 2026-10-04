use std::num::NonZeroU32;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use url::Url;

const SLACK_FILE_ID_MAX_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackAttachmentMediaKind {
    Audio,
    Video,
}

impl SlackAttachmentMediaKind {
    pub fn from_mimetype(mimetype: &str) -> Option<Self> {
        if mimetype.starts_with("audio/") {
            Some(Self::Audio)
        } else if mimetype.starts_with("video/") {
            Some(Self::Video)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SlackAttachmentMedia {
    file_id: String,
    content_url: String,
    kind: SlackAttachmentMediaKind,
}

#[derive(Deserialize)]
struct SlackAttachmentMediaWire {
    file_id: String,
    content_url: String,
    kind: SlackAttachmentMediaKind,
}

impl SlackAttachmentMedia {
    pub fn parse(
        file_id: impl Into<String>,
        content_url: impl Into<String>,
        kind: SlackAttachmentMediaKind,
    ) -> Result<Self, String> {
        let file_id = file_id.into();
        if file_id.is_empty()
            || file_id.len() > SLACK_FILE_ID_MAX_BYTES
            || !file_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("Slack media file ID has an invalid shape".to_string());
        }
        let content_url = content_url.into();
        let parsed_url = Url::parse(&content_url)
            .map_err(|error| format!("Slack media content URL is invalid: {error}"))?;
        if parsed_url.scheme() != "https"
            || !parsed_url.username().is_empty()
            || parsed_url.password().is_some()
            || !parsed_url
                .host_str()
                .is_some_and(slack_media_content_host_is_trusted)
        {
            return Err(
                "Slack media content URL must use an authenticated Slack HTTPS host".to_string(),
            );
        }
        Ok(Self {
            file_id,
            content_url: parsed_url.into(),
            kind,
        })
    }

    pub fn file_id(&self) -> &str {
        &self.file_id
    }

    pub fn content_url(&self) -> &str {
        &self.content_url
    }

    pub fn kind(&self) -> SlackAttachmentMediaKind {
        self.kind
    }
}

impl<'de> Deserialize<'de> for SlackAttachmentMedia {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SlackAttachmentMediaWire::deserialize(deserializer)?;
        Self::parse(wire.file_id, wire.content_url, wire.kind).map_err(de::Error::custom)
    }
}

fn slack_media_content_host_is_trusted(host: &str) -> bool {
    host == "slack.com"
        || host.ends_with(".slack.com")
        || host.ends_with(".slack-edge.com")
        || host.ends_with(".slack-files.com")
        || host.ends_with(".slack-imgs.com")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAttachmentPreviewSize {
    width: NonZeroU32,
    height: NonZeroU32,
}

impl SlackAttachmentPreviewSize {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        Some(Self {
            width: NonZeroU32::new(width)?,
            height: NonZeroU32::new(height)?,
        })
    }

    pub fn width(self) -> u32 {
        self.width.get()
    }

    pub fn height(self) -> u32 {
        self.height.get()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLegacyAttachmentFile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mimetype: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<SlackAttachmentMedia>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_millis: Option<NonZeroU32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub link_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_layout_size: Option<SlackAttachmentPreviewSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_height: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackLegacyAttachmentMetadata {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub service_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_height: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_avatar_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_avatar_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permalink: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SlackLegacyAttachmentFile>,
}

impl SlackLegacyAttachmentMetadata {
    pub fn is_shared_message(&self) -> bool {
        self.channel_id.is_some() && self.timestamp.is_some() && self.permalink.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackAttachmentSource {
    #[default]
    File,
    LegacyMessage(Box<SlackLegacyAttachmentMetadata>),
    BlockImage,
}

impl SlackAttachmentSource {
    fn is_default(source: &Self) -> bool {
        source == &Self::default()
    }

    pub fn legacy_metadata(&self) -> Option<&SlackLegacyAttachmentMetadata> {
        match self {
            Self::LegacyMessage(metadata) => Some(metadata.as_ref()),
            Self::File | Self::BlockImage => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAttachment {
    pub title: String,
    #[serde(default, skip_serializing_if = "SlackAttachmentSource::is_default")]
    pub source: SlackAttachmentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<SlackAttachmentMedia>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mimetype: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_millis: Option<NonZeroU32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub link_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_image_mimetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_layout_size: Option<SlackAttachmentPreviewSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_team_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackAttachmentWire {
    Label(String),
    Rich(Box<SlackAttachment>),
}

impl SlackAttachment {
    pub fn label_only(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            source: SlackAttachmentSource::File,
            media: None,
            mimetype: String::new(),
            duration_millis: None,
            description: String::new(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        }
    }

    pub fn website_preview(
        title: impl Into<String>,
        link_url: impl Into<String>,
        source_label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            source: SlackAttachmentSource::LegacyMessage(Box::default()),
            media: None,
            mimetype: String::new(),
            duration_millis: None,
            description: description.into(),
            link_url: link_url.into(),
            source_label: source_label.into(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        }
    }

    pub fn is_website_preview(&self) -> bool {
        self.source.legacy_metadata().is_some()
    }

    pub fn legacy_metadata(&self) -> Option<&SlackLegacyAttachmentMetadata> {
        self.source.legacy_metadata()
    }

    pub fn service_icon_url(&self) -> Option<&str> {
        self.legacy_metadata()
            .and_then(|metadata| metadata.service_icon_url.as_deref())
    }
}

pub(crate) fn deserialize_slack_attachments<'de, D>(
    deserializer: D,
) -> Result<Vec<SlackAttachment>, D::Error>
where
    D: Deserializer<'de>,
{
    let items = Vec::<SlackAttachmentWire>::deserialize(deserializer)?;
    items
        .into_iter()
        .map(|item| match item {
            SlackAttachmentWire::Label(label) => Ok(SlackAttachment::label_only(label)),
            SlackAttachmentWire::Rich(attachment) => {
                if attachment.title.trim().is_empty() {
                    Err(de::Error::custom("Slack attachment title cannot be empty"))
                } else {
                    Ok(*attachment)
                }
            }
        })
        .collect()
}
