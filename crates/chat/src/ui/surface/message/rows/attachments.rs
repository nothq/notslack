use std::sync::Arc;

use chrono_tz::Tz;

use self::gallery::slack_attachment_layout_rows;
pub(crate) use self::prepare::slack_attachment_row_with_identity;
#[cfg(test)]
use crate::ui::surface::slack_remote_image_cache_key;
use crate::ui::surface::{SlackAttachmentLayoutRow, SlackAttachmentRow};
use crate::ui::{SlackAttachment, SlackMessage};

mod gallery;
mod prepare;

type SlackMessageAttachmentRows = (Vec<SlackAttachmentRow>, Arc<[SlackAttachmentLayoutRow]>);

#[derive(Clone, Copy)]
pub(super) struct SlackAttachmentMessageScope<'a> {
    pub(super) team_id: &'a str,
    pub(super) conversation_id: &'a str,
}

impl<'a> SlackAttachmentMessageScope<'a> {
    pub(super) fn new(team_id: &'a str, conversation_id: &'a str) -> Self {
        Self {
            team_id,
            conversation_id,
        }
    }
}

pub(super) fn slack_attachment_rows_for_message(
    message: &SlackMessage,
    timezone: Tz,
    scope: SlackAttachmentMessageScope<'_>,
) -> SlackMessageAttachmentRows {
    let attachments = message
        .attachments
        .iter()
        .enumerate()
        .map(|(index, attachment)| {
            slack_attachment_row_for_message(attachment, timezone, scope, &message.id, index)
        })
        .collect::<Vec<_>>();
    let layout = slack_attachment_layout_rows(&attachments);
    (attachments, layout)
}

pub(super) fn slack_attachment_rows_for_local_delivery(
    attachments: &[SlackAttachment],
    timezone: Tz,
    team_id: &str,
    client_message_id: &str,
) -> SlackMessageAttachmentRows {
    let attachments = attachments
        .iter()
        .enumerate()
        .map(|(index, attachment)| {
            slack_attachment_row_with_identity(
                attachment,
                timezone,
                Some(team_id),
                format!("slack-attachment-{team_id}-local-delivery-{client_message_id}-{index}"),
            )
        })
        .collect::<Vec<_>>();
    let layout = slack_attachment_layout_rows(&attachments);
    (attachments, layout)
}

#[cfg(test)]
pub(crate) fn slack_attachment_row(
    attachment: &SlackAttachment,
    timezone: Tz,
) -> SlackAttachmentRow {
    let preview_cache_key = slack_remote_image_cache_key(attachment);
    let attachment_id = format!(
        "slack-attachment-standalone-{}-{}",
        preview_cache_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .unwrap_or(attachment.link_url.as_str()),
        attachment.title
    );
    slack_attachment_row_with_identity(attachment, timezone, None, attachment_id)
}

fn slack_attachment_row_for_message(
    attachment: &SlackAttachment,
    timezone: Tz,
    scope: SlackAttachmentMessageScope<'_>,
    message_id: &str,
    attachment_index: usize,
) -> SlackAttachmentRow {
    slack_attachment_row_with_identity(
        attachment,
        timezone,
        Some(scope.team_id),
        format!(
            "slack-attachment-t{}:{}-c{}:{}-m{}:{}-a{attachment_index}",
            scope.team_id.len(),
            scope.team_id,
            scope.conversation_id.len(),
            scope.conversation_id,
            message_id.len(),
            message_id,
        ),
    )
}
