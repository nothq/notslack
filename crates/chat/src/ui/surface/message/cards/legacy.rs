use std::sync::Arc;

use crate::ui::surface::{
    slack_palette, SlackAttachmentRow, SlackMessageRenderContext, SurfaceState,
};
use crate::ui::SlackAttachment;
use crate::ui::{
    div, img, px, relative, rgb, Context, Div, FluentBuilder, FontWeight, Image,
    InteractiveElement, MouseButton, MouseDownEvent, ParentElement, Styled,
};
use gpui::{ObjectFit, StyledImage};

use super::super::card_helpers::{slack_attachment_shows_title, slack_attachment_source_label};
use super::super::rows::{slack_prepared_message_body, SlackPreparedMessageBodyStyle};

impl SurfaceState {
    pub(super) fn render_slack_legacy_attachment_card(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let attachment = &attachment_row.attachment;
        let preview_image =
            self.slack_attachment_preview_image_by_key(attachment_row.preview_cache_key.as_deref());
        let palette = slack_palette(self.appearance_mode);
        let link_url = attachment.link_url.clone();
        div()
            .w(px(600.0))
            .max_w_full()
            .flex()
            .items_stretch()
            .when(!link_url.is_empty(), |this| this.cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    if !link_url.is_empty() {
                        this.open_slack_link(link_url.as_str(), cx);
                    }
                }),
            )
            .child(
                div()
                    .w(px(4.0))
                    .flex_none()
                    .rounded(px(2.0))
                    .bg(rgb(palette.attachment_border)),
            )
            .child(
                div()
                    .w(px(0.0))
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .px(px(12.0))
                    .flex()
                    .flex_col()
                    .when_some(
                        self.render_slack_legacy_attachment_author(attachment),
                        |this, author| this.child(author).gap(px(6.0)),
                    )
                    .child(self.render_slack_legacy_attachment_main(attachment_row, cx))
                    .when_some(preview_image, |this, image| {
                        this.child(self.render_slack_legacy_attachment_preview(attachment, image))
                    }),
            )
    }

    fn render_slack_legacy_attachment_preview(
        &self,
        attachment: &SlackAttachment,
        image: Arc<Image>,
    ) -> Div {
        let preview = div()
            .mt(px(8.0))
            .max_w(px(360.0))
            .max_h(px(500.0))
            .overflow_hidden()
            .rounded(px(4.0));
        let Some((width, height)) = slack_legacy_attachment_preview_size(attachment) else {
            return preview.child(img(image).max_w(px(360.0)).max_h(px(500.0)));
        };
        preview
            .w(px(width))
            .h(px(height))
            .child(img(image).size_full().object_fit(ObjectFit::Cover))
    }

    fn render_slack_legacy_attachment_author(&self, attachment: &SlackAttachment) -> Option<Div> {
        let metadata = attachment.legacy_metadata()?;
        let source_label = if metadata.service_name.is_empty() {
            slack_attachment_source_label(attachment)
        } else {
            metadata.service_name.clone()
        };
        let service_icon = metadata.service_icon_url.as_deref();
        if source_label.is_empty() && service_icon.is_none() {
            return None;
        }
        let palette = slack_palette(self.appearance_mode);
        Some(
            div()
                .h(px(16.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .when_some(service_icon, |this, icon_url| {
                    this.child(
                        div()
                            .size(px(16.0))
                            .flex_none()
                            .overflow_hidden()
                            .when_some(
                                self.slack_remote_images.get(icon_url).cloned(),
                                |this, image| {
                                    this.child(img(image).size(px(16.0)).rounded(px(2.0)))
                                },
                            ),
                    )
                })
                .when(!source_label.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(13.0))
                            .line_height(relative(1.23))
                            .text_color(rgb(palette.attachment_text))
                            .child(source_label),
                    )
                }),
        )
    }

    fn render_slack_legacy_attachment_main(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let attachment = &attachment_row.attachment;
        let palette = slack_palette(self.appearance_mode);
        div()
            .w_full()
            .min_w(px(0.0))
            .max_w(px(572.0))
            .flex()
            .flex_col()
            .when(slack_attachment_shows_title(attachment), |this| {
                this.when_some(
                    attachment_row.legacy_title_body.as_ref(),
                    |this, title_body| {
                        this.child(slack_prepared_message_body(
                            self,
                            title_body,
                            None,
                            SlackPreparedMessageBodyStyle::new(
                                self.appearance_mode,
                                SlackMessageRenderContext::Conversation,
                                palette.link,
                                FontWeight::BOLD,
                            ),
                            cx,
                        ))
                    },
                )
            })
            .when(!attachment.description.is_empty(), |this| {
                this.when_some(
                    attachment_row.legacy_description_body.as_ref(),
                    |this, description_body| {
                        this.child(slack_prepared_message_body(
                            self,
                            description_body,
                            None,
                            SlackPreparedMessageBodyStyle::new(
                                self.appearance_mode,
                                SlackMessageRenderContext::Conversation,
                                palette.main_text,
                                FontWeight::NORMAL,
                            ),
                            cx,
                        ))
                    },
                )
            })
    }
}

fn slack_legacy_attachment_preview_size(attachment: &SlackAttachment) -> Option<(f32, f32)> {
    let metadata = attachment.legacy_metadata()?;
    let source_width = metadata.image_width? as f32;
    let source_height = metadata.image_height? as f32;
    let scale = (360.0 / source_width).min(500.0 / source_height).min(1.0);
    Some((
        (source_width * scale).floor(),
        (source_height * scale).floor(),
    ))
}
