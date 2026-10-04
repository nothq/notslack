use super::video::SlackInlineVideoFrame;
use crate::ui::surface::{
    slack_icon, slack_palette, SlackAttachmentRow, SlackAttachmentSelection,
    SlackFileGalleryCellRow, SlackFileGalleryLineRow, SlackFileGalleryRow, SlackMediaHostId,
    SlackMessageRenderContext, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    div, img, px, rgb, AnyElement, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, SlackAttachmentMediaKind, StatefulInteractiveElement,
    Styled,
};
use gpui::{ObjectFit, Role, StyledImage};

struct SlackFileGalleryCell<'a> {
    attachment: &'a SlackAttachmentRow,
    cell: &'a SlackFileGalleryCellRow,
    height: u32,
    render_context: SlackMessageRenderContext,
}

impl SurfaceState {
    pub(in crate::ui::surface::message) fn render_slack_file_gallery(
        &self,
        gallery: &SlackFileGalleryRow,
        attachments: &[SlackAttachmentRow],
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let collapsed = self
            .slack_collapsed_attachment_ids
            .contains(gallery.attachment_id.as_ref());
        div()
            .w_full()
            .max_w(px(936.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(self.render_slack_file_gallery_header(gallery, collapsed, cx))
            .when(!collapsed, |this| {
                this.children(gallery.lines.iter().map(|line| {
                    self.render_slack_file_gallery_line(line, attachments, render_context, cx)
                }))
            })
    }

    fn render_slack_file_gallery_header(
        &self,
        gallery: &SlackFileGalleryRow,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let color = slack_palette(self.appearance_mode).attachment_muted_text;
        div()
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .line_height(px(19.0668))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(rgb(color))
                    .child(gallery.count_label.clone()),
            )
            .child(self.render_slack_file_gallery_toggle(gallery, collapsed, color, cx))
    }

    fn render_slack_file_gallery_toggle(
        &self,
        gallery: &SlackFileGalleryRow,
        collapsed: bool,
        color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let clicked_attachment_id = gallery.attachment_id.clone();
        let keyboard_attachment_id = clicked_attachment_id.clone();
        div()
            .id(format!(
                "slack-file-gallery-toggle-{}",
                gallery.attachment_id
            ))
            .role(Role::Button)
            .aria_label(gallery.accessibility_label.clone())
            .aria_expanded(!collapsed)
            .focusable()
            .tab_stop(true)
            .size(px(20.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_slack_attachment_collapsed(clicked_attachment_id.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.toggle_slack_attachment_collapsed(keyboard_attachment_id.as_ref(), cx);
            }))
            .child(slack_icon(
                if collapsed {
                    SlackShellIcon::AttachmentCaretRight
                } else {
                    SlackShellIcon::AttachmentCaretDown
                },
                color,
                20.0,
                cx,
            ))
    }

    fn render_slack_file_gallery_line(
        &self,
        line: &SlackFileGalleryLineRow,
        attachments: &[SlackAttachmentRow],
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .max_w(px(936.0))
            .h(px(line.height as f32))
            .flex()
            .items_start()
            .gap(px(8.0))
            .children(line.cells.iter().map(|cell| {
                let attachment = attachments
                    .get(cell.attachment_index)
                    .expect("prepared Slack gallery attachment index must remain valid");
                self.render_slack_file_gallery_cell(
                    SlackFileGalleryCell {
                        attachment,
                        cell,
                        height: line.height,
                        render_context,
                    },
                    cx,
                )
            }))
    }

    fn render_slack_file_gallery_cell(
        &self,
        gallery_cell: SlackFileGalleryCell<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let preview_image = self.slack_attachment_preview_image_by_key(
            gallery_cell.attachment.preview_cache_key.as_deref(),
        );
        if let Some(video) =
            self.render_slack_file_gallery_video_cell(&gallery_cell, preview_image.clone(), cx)
        {
            return video;
        }
        let SlackFileGalleryCell {
            attachment,
            cell,
            height,
            ..
        } = gallery_cell;

        let selection = SlackAttachmentSelection::from_row(attachment);
        let keyboard_selection = selection.clone();
        div()
            .id(slack_file_gallery_preview_id(attachment))
            .role(Role::Button)
            .aria_label(format!("Open preview for {}", attachment.title))
            .focusable()
            .tab_stop(true)
            .w(px(cell.width as f32))
            .h(px(height as f32))
            .flex_none()
            .rounded(px(8.0))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(palette.attachment_border))
            .bg(rgb(palette.attachment_bg))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_slack_attachment_expanded(&selection, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_file_gallery_preview_key(&keyboard_selection, event, window, cx);
            }))
            .when(preview_image.is_none(), |this| {
                this.child(
                    self.render_slack_attachment_preview_placeholder(attachment.title.clone(), cx),
                )
            })
            .when_some(preview_image, |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Cover))
            })
            .into_any_element()
    }

    fn handle_slack_file_gallery_preview_key(
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

    fn render_slack_file_gallery_video_cell(
        &self,
        gallery_cell: &SlackFileGalleryCell<'_>,
        preview_image: Option<std::sync::Arc<crate::ui::Image>>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let SlackFileGalleryCell {
            attachment,
            cell,
            height,
            render_context,
        } = gallery_cell;
        let media = attachment.attachment.media.as_ref()?;
        if media.kind() != SlackAttachmentMediaKind::Video {
            return None;
        }
        Some(
            self.render_slack_inline_video_frame(
                SlackInlineVideoFrame {
                    id: format!("slack-file-gallery-preview-{}", attachment.attachment_id),
                    selection: SlackAttachmentSelection::from_row(attachment),
                    host: SlackMediaHostId::message(
                        *render_context,
                        attachment.attachment_id.clone(),
                    ),
                    preview_image,
                    width: cell.width as f32,
                    height: *height as f32,
                    radius: 8.0,
                    object_fit: ObjectFit::Cover,
                    duration_label: attachment.recording_duration_label.clone(),
                },
                cx,
            )
            .into_any_element(),
        )
    }

    pub(super) fn render_slack_attachment_preview_placeholder(
        &self,
        title: gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .px(px(12.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .text_color(rgb(palette.attachment_muted_text))
            .child(slack_icon(
                SlackShellIcon::Files,
                palette.attachment_muted_text,
                24.0,
                cx,
            ))
            .child(
                div()
                    .max_w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.0))
                    .child(title),
            )
    }
}

fn slack_file_gallery_preview_id(attachment: &SlackAttachmentRow) -> String {
    format!("slack-file-gallery-preview-{}", attachment.attachment_id)
}
