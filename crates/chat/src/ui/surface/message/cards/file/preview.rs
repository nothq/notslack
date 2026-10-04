use std::sync::Arc;

use super::super::video::SlackInlineVideoFrame;
use crate::ui::surface::{
    slack_palette, SlackAttachmentRenderKind, SlackAttachmentRow, SlackAttachmentSelection,
    SlackMediaHostId, SlackMessageRenderContext, SurfaceState,
};
use crate::ui::{
    div, img, px, rgb, Context, Div, FluentBuilder, Image, InteractiveElement, KeyDownEvent,
    ParentElement, StatefulInteractiveElement, Styled,
};
use crate::ui::{SlackAttachmentMediaKind, SlackAttachmentPreviewSize};
use gpui::{ObjectFit, Role, Stateful, StyledImage};

struct SlackAttachmentPreviewPresentation {
    image: Option<Arc<Image>>,
    loaded: bool,
    has_preview: bool,
    collapsible: bool,
    collapsed: bool,
    max_width: f32,
    max_height: f32,
    size: Option<SlackAttachmentPreviewSize>,
    recording_duration_label: Option<gpui::SharedString>,
}

impl SurfaceState {
    pub(crate) fn render_slack_message_attachment_card(
        &self,
        attachment_row: &SlackAttachmentRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        if attachment_row.kind == SlackAttachmentRenderKind::SharedMessage {
            return self.render_slack_shared_message_attachment(attachment_row, render_context, cx);
        }
        if attachment_row.kind == SlackAttachmentRenderKind::WebsitePreview {
            return self.render_slack_legacy_attachment_card(attachment_row, cx);
        }
        let presentation = self.slack_attachment_preview_presentation(attachment_row);
        div()
            .w_full()
            .max_w_full()
            .when(!attachment_row.collapsible_file_preview, |this| {
                this.max_w(px(presentation.max_width))
            })
            .flex()
            .flex_col()
            .items_start()
            .gap(px(if attachment_row.collapsible_file_preview {
                4.0
            } else {
                3.0
            }))
            .when_some(
                self.render_slack_attachment_card_header(
                    attachment_row,
                    presentation.collapsible,
                    presentation.collapsed,
                    cx,
                ),
                |this, header| this.child(header),
            )
            .when(
                presentation.has_preview && !presentation.collapsed,
                |this| {
                    this.child(self.render_slack_attachment_preview(
                        attachment_row,
                        &presentation,
                        render_context,
                        cx,
                    ))
                },
            )
            .when(!presentation.has_preview, |this| {
                this.child(super::slack_attachment_without_preview(
                    attachment_row,
                    self.appearance_mode,
                ))
            })
    }

    fn slack_attachment_preview_presentation(
        &self,
        attachment_row: &SlackAttachmentRow,
    ) -> SlackAttachmentPreviewPresentation {
        let image =
            self.slack_attachment_preview_image_by_key(attachment_row.preview_cache_key.as_deref());
        let has_preview = attachment_row.preview_cache_key.is_some();
        let collapsible = attachment_row.collapsible_file_preview && has_preview;
        SlackAttachmentPreviewPresentation {
            loaded: image.is_some(),
            image,
            has_preview,
            collapsible,
            collapsed: collapsible
                && self
                    .slack_collapsed_attachment_ids
                    .contains(attachment_row.attachment_id.as_ref()),
            max_width: attachment_row.preview_max_width as f32,
            max_height: attachment_row.preview_max_height as f32,
            size: attachment_row.preview_size,
            recording_duration_label: attachment_row.recording_duration_label.clone(),
        }
    }

    fn render_slack_attachment_preview(
        &self,
        attachment_row: &SlackAttachmentRow,
        presentation: &SlackAttachmentPreviewPresentation,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .max_w(px(936.0))
            .when_some(presentation.size, |this, size| {
                this.h(px(size.height() as f32))
            })
            .child(self.render_slack_attachment_preview_button(
                attachment_row,
                presentation,
                render_context,
                cx,
            ))
    }

    fn render_slack_attachment_preview_button(
        &self,
        attachment_row: &SlackAttachmentRow,
        presentation: &SlackAttachmentPreviewPresentation,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        if let Some(video) = self.render_slack_attachment_video_preview(
            attachment_row,
            presentation,
            render_context,
            cx,
        ) {
            return video;
        }

        let palette = slack_palette(self.appearance_mode);
        let selection = SlackAttachmentSelection::from_row(attachment_row);
        let keyboard_selection = selection.clone();
        div()
            .id(slack_attachment_preview_id(attachment_row))
            .max_w_full()
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(rgb(palette.attachment_bg))
            .role(Role::Button)
            .aria_label(format!("Open preview for {}", attachment_row.title))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_slack_attachment_expanded(&selection, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_attachment_preview_key(&keyboard_selection, event, window, cx);
            }))
            .relative()
            .when_some(presentation.size, |this, size| {
                this.w(px(size.width() as f32)).h(px(size.height() as f32))
            })
            .when(presentation.size.is_none(), |this| {
                this.w(px(presentation.max_width))
                    .h(px(presentation.max_height))
            })
            .flex()
            .items_center()
            .justify_center()
            .when(!presentation.loaded, |this| {
                this.child(
                    self.render_slack_attachment_preview_placeholder(
                        attachment_row.title.clone(),
                        cx,
                    ),
                )
            })
            .when_some(presentation.image.clone(), |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Contain))
            })
    }

    fn handle_slack_attachment_preview_key(
        &mut self,
        selection: &SlackAttachmentSelection,
        event: &KeyDownEvent,
        window: &mut crate::ui::Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified()
            || !matches!(event.keystroke.key.as_str(), "enter" | "space")
        {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        self.toggle_slack_attachment_expanded(selection, cx);
    }

    fn render_slack_attachment_video_preview(
        &self,
        attachment_row: &SlackAttachmentRow,
        presentation: &SlackAttachmentPreviewPresentation,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let media = attachment_row.attachment.media.as_ref()?;
        if media.kind() != SlackAttachmentMediaKind::Video {
            return None;
        }
        let (width, height) = presentation
            .size
            .map_or((presentation.max_width, presentation.max_height), |size| {
                (size.width() as f32, size.height() as f32)
            });
        Some(self.render_slack_inline_video_frame(
            SlackInlineVideoFrame {
                id: format!("slack-attachment-preview-{}", attachment_row.attachment_id),
                selection: SlackAttachmentSelection::from_row(attachment_row),
                host: SlackMediaHostId::message(
                    render_context,
                    attachment_row.attachment_id.clone(),
                ),
                preview_image: presentation.image.clone(),
                width,
                height,
                radius: 8.0,
                object_fit: ObjectFit::Contain,
                duration_label: presentation.recording_duration_label.clone(),
            },
            cx,
        ))
    }
}

fn slack_attachment_preview_id(attachment: &SlackAttachmentRow) -> String {
    format!("slack-attachment-preview-{}", attachment.attachment_id)
}
