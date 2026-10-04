use std::collections::HashSet;

use crate::ui::surface::{SlackAttachmentRow, SlackMessageRow, SlackSearchRow, SurfaceState};

impl SurfaceState {
    pub(crate) fn slack_media_attachment_summaries(
        &self,
    ) -> Vec<crate::model::ChatMediaAttachmentSummary> {
        let mut summaries = Vec::new();
        let mut seen = HashSet::new();
        if self.slack_search_results_open {
            append_slack_search_media_attachment_summaries(
                &self.slack_search_rows,
                &mut seen,
                &mut summaries,
            );
        }
        append_slack_media_attachment_summaries(
            &self.slack_message_rows,
            &mut seen,
            &mut summaries,
        );
        if let Some(panel) = self.slack_thread_panel.as_ref() {
            append_slack_media_attachment_summaries(
                std::slice::from_ref(&panel.parent_row),
                &mut seen,
                &mut summaries,
            );
            append_slack_media_attachment_summaries(&panel.reply_rows, &mut seen, &mut summaries);
        }
        summaries.retain(|summary| {
            self.resolve_slack_attachment_selection_by_id(&summary.attachment_id)
                .is_some()
        });
        summaries
    }
}

fn append_slack_media_attachment_summaries(
    rows: &[SlackMessageRow],
    seen: &mut HashSet<String>,
    summaries: &mut Vec<crate::model::ChatMediaAttachmentSummary>,
) {
    for message in rows {
        for attachment in &message.attachments {
            append_slack_media_attachment_row_summary(attachment, seen, summaries);
        }
        append_slack_media_attachment_summaries(&message.replies, seen, summaries);
    }
}

fn append_slack_search_media_attachment_summaries(
    rows: &[SlackSearchRow],
    seen: &mut HashSet<String>,
    summaries: &mut Vec<crate::model::ChatMediaAttachmentSummary>,
) {
    for attachment in rows.iter().flat_map(|row| row.attachments.iter()) {
        append_slack_media_attachment_row_summary(&attachment.attachment, seen, summaries);
    }
}

fn append_slack_media_attachment_row_summary(
    attachment: &SlackAttachmentRow,
    seen: &mut HashSet<String>,
    summaries: &mut Vec<crate::model::ChatMediaAttachmentSummary>,
) {
    append_slack_media_attachment_summary(
        attachment.attachment_id.as_ref(),
        attachment.title.as_ref(),
        attachment.attachment.media.as_ref(),
        seen,
        summaries,
    );
    if let Some(shared) = attachment.shared_message.as_ref() {
        for file in shared.files.iter() {
            append_slack_media_attachment_summary(
                file.attachment_id.as_ref(),
                file.title.as_ref(),
                file.attachment.media.as_ref(),
                seen,
                summaries,
            );
        }
    }
}

pub(super) fn slack_search_rows_contain_attachment_id(
    rows: &[SlackSearchRow],
    attachment_id: &str,
) -> bool {
    rows.iter().any(|row| {
        row.attachments.iter().any(|attachment| {
            let attachment = &attachment.attachment;
            attachment.attachment_id.as_ref() == attachment_id
                || attachment.shared_message.as_ref().is_some_and(|shared| {
                    shared
                        .files
                        .iter()
                        .any(|file| file.attachment_id.as_ref() == attachment_id)
                })
        })
    })
}

pub(super) fn slack_rows_contain_attachment_id(
    rows: &[SlackMessageRow],
    attachment_id: &str,
) -> bool {
    rows.iter().any(|row| {
        row.attachments.iter().any(|attachment| {
            attachment.attachment_id.as_ref() == attachment_id
                || attachment.shared_message.as_ref().is_some_and(|shared| {
                    shared
                        .files
                        .iter()
                        .any(|file| file.attachment_id.as_ref() == attachment_id)
                })
        }) || slack_rows_contain_attachment_id(&row.replies, attachment_id)
    })
}

fn append_slack_media_attachment_summary(
    attachment_id: &str,
    title: &str,
    media: Option<&crate::model::SlackAttachmentMedia>,
    seen: &mut HashSet<String>,
    summaries: &mut Vec<crate::model::ChatMediaAttachmentSummary>,
) {
    let Some(media) = media else {
        return;
    };
    if !seen.insert(attachment_id.to_string()) {
        return;
    }
    summaries.push(crate::model::ChatMediaAttachmentSummary {
        attachment_id: attachment_id.to_string(),
        file_id: media.file_id().to_string(),
        title: title.to_string(),
        kind: media.kind(),
    });
}
