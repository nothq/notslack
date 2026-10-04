use std::sync::Arc;

use gpui::{Image, ObjectFit, Role};

use super::super::{consume_slack_search_action_key, slack_search_overlay_palette};
use crate::ui::surface::{
    div, img, px, rgb, AnyElement, Context, Div, FluentBuilder, InteractiveElement, IntoElement,
    ParentElement, SlackAttachmentSelection, SlackInlineVideoFrame, SlackMessageActionTarget,
    SlackSearchAttachmentPresentation, SlackSearchAttachmentRow, StatefulInteractiveElement,
    Styled, SurfaceState,
};

impl SurfaceState {
    pub(super) fn render_slack_search_attachment(
        &self,
        attachment: &SlackSearchAttachmentRow,
        target: Arc<SlackMessageActionTarget>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = match attachment.presentation {
            SlackSearchAttachmentPresentation::Audio => self.render_slack_inline_audio_attachment(
                &attachment.attachment,
                attachment.host.clone(),
                cx,
            ),
            SlackSearchAttachmentPresentation::Video { width, height } => self
                .render_slack_inline_video_frame(
                    SlackInlineVideoFrame {
                        id: format!(
                            "slack-search-video-preview-{}",
                            attachment.attachment.attachment_id
                        ),
                        selection: SlackAttachmentSelection::from_row(&attachment.attachment),
                        host: attachment.host.clone(),
                        preview_image: self.slack_attachment_preview_image_by_key(
                            attachment.attachment.preview_cache_key.as_deref(),
                        ),
                        width,
                        height,
                        radius: 8.0,
                        object_fit: ObjectFit::Contain,
                        duration_label: attachment.attachment.recording_duration_label.clone(),
                    },
                    cx,
                )
                .into_any_element(),
            SlackSearchAttachmentPresentation::Summary => self
                .render_slack_search_attachment_summary(attachment, target, cx)
                .into_any_element(),
        };
        div()
            .h(px(attachment.presentation.height()))
            .max_w_full()
            .flex_none()
            .ml(px(44.0))
            .mt(px(11.0))
            .child(content)
            .into_any_element()
    }

    fn render_slack_search_attachment_summary(
        &self,
        attachment: &SlackSearchAttachmentRow,
        target: Arc<SlackMessageActionTarget>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = crate::ui::surface::slack_palette(self.appearance_mode);
        let overlay_palette = slack_search_overlay_palette(self.appearance_mode);
        let keyboard_target = target.clone();
        let preview = attachment
            .attachment
            .preview_cache_key
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        div()
            .id(format!(
                "slack-search-attachment-summary-{}",
                attachment.attachment.attachment_id
            ))
            .h(px(61.0))
            .w(px(300.0))
            .flex_none()
            .role(Role::Button)
            .aria_label(format!(
                "Open message with attachment {}",
                attachment.attachment.title
            ))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .px(px(12.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(rgb(palette.attachment_border))
            .bg(rgb(palette.attachment_bg))
            .hover(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_search_result(&target, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_search_result(&keyboard_target, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(slack_search_attachment_preview(
                preview,
                palette.composer_chip_bg,
            ))
            .child(slack_search_attachment_metadata(
                attachment,
                palette.attachment_text,
                palette.attachment_muted_text,
            ))
    }
}

fn slack_search_attachment_preview(image: Option<Arc<Image>>, background: u32) -> Div {
    div()
        .size(px(36.0))
        .flex_none()
        .rounded(px(6.0))
        .overflow_hidden()
        .bg(rgb(background))
        .when_some(image, |this, image| {
            this.child(img(image).size(px(36.0)).rounded(px(6.0)))
        })
}

fn slack_search_attachment_metadata(
    attachment: &SlackSearchAttachmentRow,
    text: u32,
    muted_text: u32,
) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(14.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(text))
                .child(attachment.attachment.title.clone()),
        )
        .child(
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(12.0))
                .text_color(rgb(muted_text))
                .child(attachment.subtitle.clone()),
        )
}
