use std::{collections::HashMap, sync::Arc};

use base64::prelude::{Engine as _, BASE64_STANDARD};
use gpui::Image;

use super::downscale_slack_attachment_preview_image;
use crate::ui::{
    SlackAttachment, SlackConversationSnapshot, SlackLegacyAttachmentFile, SlackMessage,
    SlackShellSnapshot, SlackSidebarSnapshot, SlackWorkspace,
};

const SLACK_INITIAL_MESSAGE_IMAGE_LIMIT: usize = 32;
const SLACK_INITIAL_SIDEBAR_IMAGE_LIMIT: usize = 40;

pub(crate) fn slack_remote_image_cache_key(attachment: &SlackAttachment) -> Option<String> {
    attachment.preview_image_url.clone().or_else(|| {
        attachment.preview_image_base64.as_ref().map(|_| {
            format!(
                "{}|{}|{}",
                attachment.title,
                attachment.link_url,
                attachment
                    .preview_image_mimetype
                    .as_deref()
                    .unwrap_or_default()
            )
        })
    })
}

pub(crate) fn slack_legacy_attachment_file_image_cache_key(
    file: &SlackLegacyAttachmentFile,
) -> Option<String> {
    file.preview_image_url.clone().or_else(|| {
        file.preview_image_base64.as_ref().map(|_| {
            format!(
                "{}|{}|{}",
                file.title,
                file.link_url,
                file.preview_image_mimetype.as_deref().unwrap_or_default()
            )
        })
    })
}

pub(crate) fn build_slack_remote_image_from_parts(base64: &str, mime: &str) -> Option<Image> {
    let format = gpui_components::image_format_from_mime_type(mime)?;
    let bytes = BASE64_STANDARD.decode(base64).ok()?;
    Some(downscale_slack_attachment_preview_image(format, bytes))
}

fn build_slack_remote_image(attachment: &SlackAttachment) -> Option<Image> {
    let base64 = attachment.preview_image_base64.as_deref()?;
    let mime = attachment.preview_image_mimetype.as_deref()?;
    build_slack_remote_image_from_parts(base64, mime)
}

fn slack_attachment_remote_image(attachment: &SlackAttachment) -> Option<(String, Image)> {
    let key = slack_remote_image_cache_key(attachment)?;
    let image = build_slack_remote_image(attachment)?;
    Some((key, image))
}

fn slack_legacy_attachment_file_remote_image(
    file: &SlackLegacyAttachmentFile,
) -> Option<(String, Image)> {
    let key = slack_legacy_attachment_file_image_cache_key(file)?;
    let base64 = file.preview_image_base64.as_deref()?;
    let mimetype = file.preview_image_mimetype.as_deref()?;
    let image = build_slack_remote_image_from_parts(base64, mimetype)?;
    Some((key, image))
}

#[derive(Clone, Copy)]
struct SlackInlineRemoteImageSource<'a> {
    url: Option<&'a str>,
    base64: Option<&'a str>,
    mimetype: Option<&'a str>,
}

fn slack_inline_remote_image_sources(
    workspace: &SlackWorkspace,
) -> Vec<SlackInlineRemoteImageSource<'_>> {
    let mut sources = slack_workspace_remote_image_sources(workspace);
    sources.extend(
        workspace
            .messages
            .iter()
            .rev()
            .take(SLACK_INITIAL_MESSAGE_IMAGE_LIMIT)
            .flat_map(|message| {
                std::iter::once(SlackInlineRemoteImageSource {
                    url: message.avatar_image_url.as_deref(),
                    base64: message.avatar_image_base64.as_deref(),
                    mimetype: message.avatar_image_mimetype.as_deref(),
                })
                .chain(message.replies.iter().map(|reply| {
                    SlackInlineRemoteImageSource {
                        url: reply.avatar_image_url.as_deref(),
                        base64: reply.avatar_image_base64.as_deref(),
                        mimetype: reply.avatar_image_mimetype.as_deref(),
                    }
                }))
            }),
    );
    let sidebar_items = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter());
    sources.extend(
        sidebar_items
            .clone()
            .filter(|item| item.active)
            .chain(sidebar_items.take(SLACK_INITIAL_SIDEBAR_IMAGE_LIMIT))
            .map(|item| SlackInlineRemoteImageSource {
                url: item.avatar_image_url.as_deref(),
                base64: item.avatar_image_base64.as_deref(),
                mimetype: item.avatar_image_mimetype.as_deref(),
            }),
    );
    sources
}

fn slack_workspace_remote_image_sources(
    workspace: &SlackWorkspace,
) -> Vec<SlackInlineRemoteImageSource<'_>> {
    vec![
        SlackInlineRemoteImageSource {
            url: workspace.workspace_logo_url.as_deref(),
            base64: workspace.workspace_logo_image_base64.as_deref(),
            mimetype: workspace.workspace_logo_image_mimetype.as_deref(),
        },
        SlackInlineRemoteImageSource {
            url: workspace.self_avatar_image_url.as_deref(),
            base64: workspace.self_avatar_image_base64.as_deref(),
            mimetype: workspace.self_avatar_image_mimetype.as_deref(),
        },
    ]
}

fn slack_inline_remote_image(source: SlackInlineRemoteImageSource<'_>) -> Option<(String, Image)> {
    let url = source.url?;
    let base64 = source.base64?;
    let mimetype = source.mimetype?;
    let image = build_slack_remote_image_from_parts(base64, mimetype)?;
    Some((url.to_string(), image))
}

pub(crate) fn build_slack_remote_images(
    workspace: Option<&SlackWorkspace>,
) -> HashMap<String, Arc<Image>> {
    let mut remote_images = HashMap::new();
    let Some(workspace) = workspace else {
        return remote_images;
    };

    for attachment in workspace
        .messages
        .iter()
        .rev()
        .take(SLACK_INITIAL_MESSAGE_IMAGE_LIMIT)
        .flat_map(|message| message.attachments.iter())
    {
        extend_slack_attachment_remote_images(&mut remote_images, attachment);
    }

    for image_source in slack_inline_remote_image_sources(workspace) {
        if let Some((key, image)) = slack_inline_remote_image(image_source) {
            remote_images.insert(key, Arc::new(image));
        }
    }

    remote_images
}

pub(crate) fn build_slack_shell_remote_images(
    shell: &SlackShellSnapshot,
) -> HashMap<String, Arc<Image>> {
    inline_remote_images([
        SlackInlineRemoteImageSource {
            url: shell.workspace_logo_url.as_deref(),
            base64: shell.workspace_logo_image_base64.as_deref(),
            mimetype: shell.workspace_logo_image_mimetype.as_deref(),
        },
        SlackInlineRemoteImageSource {
            url: shell.self_avatar_image_url.as_deref(),
            base64: shell.self_avatar_image_base64.as_deref(),
            mimetype: shell.self_avatar_image_mimetype.as_deref(),
        },
    ])
}

pub(crate) fn build_slack_sidebar_remote_images(
    sidebar: &SlackSidebarSnapshot,
) -> HashMap<String, Arc<Image>> {
    inline_remote_images(
        sidebar
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter(|item| item.active)
            .chain(
                sidebar
                    .sections
                    .iter()
                    .flat_map(|section| section.items.iter())
                    .take(SLACK_INITIAL_SIDEBAR_IMAGE_LIMIT),
            )
            .map(|item| SlackInlineRemoteImageSource {
                url: item.avatar_image_url.as_deref(),
                base64: item.avatar_image_base64.as_deref(),
                mimetype: item.avatar_image_mimetype.as_deref(),
            }),
    )
}

pub(crate) fn build_slack_conversation_remote_images(
    conversation: &SlackConversationSnapshot,
) -> HashMap<String, Arc<Image>> {
    build_slack_message_remote_images(&conversation.messages)
}

pub(crate) fn build_slack_message_remote_images(
    messages: &[SlackMessage],
) -> HashMap<String, Arc<Image>> {
    let mut remote_images = HashMap::new();
    for attachment in messages
        .iter()
        .rev()
        .take(SLACK_INITIAL_MESSAGE_IMAGE_LIMIT)
        .flat_map(|message| message.attachments.iter())
    {
        extend_slack_attachment_remote_images(&mut remote_images, attachment);
    }
    remote_images.extend(inline_remote_images(
        messages
            .iter()
            .rev()
            .take(SLACK_INITIAL_MESSAGE_IMAGE_LIMIT)
            .flat_map(|message| {
                std::iter::once(SlackInlineRemoteImageSource {
                    url: message.avatar_image_url.as_deref(),
                    base64: message.avatar_image_base64.as_deref(),
                    mimetype: message.avatar_image_mimetype.as_deref(),
                })
                .chain(message.replies.iter().map(|reply| {
                    SlackInlineRemoteImageSource {
                        url: reply.avatar_image_url.as_deref(),
                        base64: reply.avatar_image_base64.as_deref(),
                        mimetype: reply.avatar_image_mimetype.as_deref(),
                    }
                }))
            }),
    ));
    remote_images
}

fn extend_slack_attachment_remote_images(
    remote_images: &mut HashMap<String, Arc<Image>>,
    attachment: &SlackAttachment,
) {
    if let Some((key, image)) = slack_attachment_remote_image(attachment) {
        remote_images.insert(key, Arc::new(image));
    }
    let Some(metadata) = attachment.legacy_metadata() else {
        return;
    };
    if let Some((key, image)) = slack_inline_remote_image(SlackInlineRemoteImageSource {
        url: metadata.author_avatar_image_url.as_deref(),
        base64: metadata.author_avatar_image_base64.as_deref(),
        mimetype: metadata.author_avatar_image_mimetype.as_deref(),
    }) {
        remote_images.insert(key, Arc::new(image));
    }
    for file in &metadata.files {
        if let Some((key, image)) = slack_legacy_attachment_file_remote_image(file) {
            remote_images.insert(key, Arc::new(image));
        }
    }
}

fn inline_remote_images<'a>(
    sources: impl IntoIterator<Item = SlackInlineRemoteImageSource<'a>>,
) -> HashMap<String, Arc<Image>> {
    sources
        .into_iter()
        .filter_map(slack_inline_remote_image)
        .map(|(key, image)| (key, Arc::new(image)))
        .collect()
}
