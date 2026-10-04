use super::cards::{
    slack_attachment_prefers_full_card, SlackCompactAttachmentShellConfig, SlackInlineVideoFrame,
};
use super::SLACK_MESSAGE_EDGE_PADDING;
use crate::ui::surface::{
    div, px, slack_inline_video_dimensions, Context, Div, IntoElement, ParentElement,
    SlackAttachmentLayoutRow, SlackAttachmentRenderKind, SlackAttachmentRow,
    SlackAttachmentSelection, SlackMediaHostId, SlackMessageRenderContext, SlackMessageRow,
    SlackShellIcon, Styled, SurfaceState,
};
use crate::ui::SlackAttachmentMediaKind;
use gpui::ObjectFit;

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_message_attachments(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .pb(px(SLACK_MESSAGE_EDGE_PADDING))
            .children(row.attachment_layout.iter().map(|layout| {
                match layout {
                    SlackAttachmentLayoutRow::Single { attachment_index } => {
                        let attachment = row
                            .attachments
                            .get(*attachment_index)
                            .expect("prepared Slack attachment index must remain valid");
                        if slack_attachment_prefers_full_card(attachment) {
                            self.render_slack_message_attachment_card(
                                attachment,
                                render_context,
                                cx,
                            )
                            .into_any_element()
                        } else if attachment
                            .attachment
                            .media
                            .as_ref()
                            .is_some_and(|media| media.kind() == SlackAttachmentMediaKind::Video)
                        {
                            self.render_slack_compact_video_attachment(
                                attachment,
                                render_context,
                                cx,
                            )
                            .into_any_element()
                        } else if attachment.kind == SlackAttachmentRenderKind::Recording {
                            self.render_slack_compact_recording_attachment(
                                attachment,
                                render_context,
                                cx,
                            )
                            .into_any_element()
                        } else {
                            self.render_slack_compact_attachment_card(attachment, cx)
                                .into_any_element()
                        }
                    }
                    SlackAttachmentLayoutRow::FileGallery(gallery) => self
                        .render_slack_file_gallery(gallery, &row.attachments, render_context, cx)
                        .into_any_element(),
                }
            }))
    }

    fn render_slack_compact_video_attachment(
        &self,
        attachment_row: &SlackAttachmentRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (width, height) = slack_inline_video_dimensions(attachment_row);
        self.render_slack_inline_video_frame(
            SlackInlineVideoFrame {
                id: format!(
                    "slack-compact-video-preview-{}",
                    attachment_row.attachment_id
                ),
                selection: SlackAttachmentSelection::from_row(attachment_row),
                host: SlackMediaHostId::message(
                    render_context,
                    attachment_row.attachment_id.clone(),
                ),
                preview_image: self.slack_attachment_preview_image_by_key(
                    attachment_row.preview_cache_key.as_deref(),
                ),
                width,
                height,
                radius: 8.0,
                object_fit: ObjectFit::Contain,
                duration_label: attachment_row.recording_duration_label.clone(),
            },
            cx,
        )
    }

    fn render_slack_compact_recording_attachment(
        &self,
        attachment_row: &SlackAttachmentRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> crate::ui::surface::AnyElement {
        let is_audio = attachment_row
            .attachment
            .media
            .as_ref()
            .map(|media| media.kind() == SlackAttachmentMediaKind::Audio)
            .unwrap_or_else(|| attachment_row.attachment.mimetype.starts_with("audio/"));
        if is_audio && attachment_row.attachment.media.is_some() {
            return self.render_slack_inline_audio_attachment(
                attachment_row,
                SlackMediaHostId::message(render_context, attachment_row.attachment_id.clone()),
                cx,
            );
        }
        self.render_slack_compact_attachment_shell(
            attachment_row,
            SlackCompactAttachmentShellConfig {
                icon: if is_audio {
                    SlackShellIcon::Mic
                } else {
                    SlackShellIcon::Video
                },
                eyebrow: if is_audio {
                    "Audio clip".to_string()
                } else {
                    "Recording".to_string()
                },
                detail: attachment_row
                    .recording_duration_label
                    .as_ref()
                    .map(ToString::to_string),
                trailing_action: None,
            },
            cx,
        )
        .into_any_element()
    }
}
