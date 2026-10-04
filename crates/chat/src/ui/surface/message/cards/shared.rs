use crate::ui::surface::{
    slack_icon, slack_palette, SlackAttachmentRow, SlackMessageRenderContext,
    SlackSharedMessageAttachmentRow, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    div, img, px, rgb, AppearanceMode, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    MouseButton, MouseDownEvent, ParentElement, Styled,
};
mod preview;

impl SurfaceState {
    pub(super) fn render_slack_shared_message_attachment(
        &self,
        attachment_row: &SlackAttachmentRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let shared_message = attachment_row
            .shared_message
            .as_ref()
            .expect("shared-message attachment row should include shaped metadata");
        let palette = slack_palette(self.appearance_mode);
        let border_color = match self.appearance_mode {
            AppearanceMode::Dark => 0xd0d0d0,
            AppearanceMode::Light => palette.attachment_muted_text,
        };
        let permalink = shared_message.permalink.clone().unwrap_or_default();
        div()
            .w(px(600.0))
            .max_w_full()
            .flex()
            .items_stretch()
            .when(!permalink.is_empty(), |this| this.cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    if !permalink.is_empty() {
                        this.open_slack_link(permalink.as_ref(), cx);
                    }
                }),
            )
            .child(
                div()
                    .w(px(4.0))
                    .flex_none()
                    .rounded(px(2.0))
                    .bg(rgb(border_color)),
            )
            .child(self.render_slack_shared_message_content(
                attachment_row,
                shared_message,
                render_context,
                cx,
            ))
    }

    fn render_slack_shared_message_content(
        &self,
        attachment_row: &SlackAttachmentRow,
        shared_message: &SlackSharedMessageAttachmentRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w(px(0.0))
            .flex_grow(1.0)
            .min_w(px(0.0))
            .px(px(12.0))
            .flex()
            .flex_col()
            .child(self.render_slack_shared_message_author(shared_message))
            .child(
                div()
                    .w_full()
                    .min_w(px(0.0))
                    .text_size(px(15.0))
                    .line_height(px(22.0))
                    .text_color(rgb(palette.main_text))
                    .child(shared_message.body.clone()),
            )
            .children(
                shared_message
                    .files
                    .iter()
                    .map(|file| self.render_slack_shared_message_file(file, render_context, cx)),
            )
            .when(
                shared_message.channel_label.is_some()
                    || shared_message.timestamp_label.is_some()
                    || shared_message.permalink.is_some(),
                |this| this.child(self.render_slack_shared_message_footer(attachment_row, cx)),
            )
    }

    fn render_slack_shared_message_author(
        &self,
        shared_message: &SlackSharedMessageAttachmentRow,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let avatar_image = shared_message
            .author_avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let avatar = div()
            .size(px(16.0))
            .flex_none()
            .rounded(px(2.0))
            .overflow_hidden()
            .bg(rgb(shared_message.author_avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .when(avatar_image.is_none(), |this| {
                this.text_size(px(7.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(shared_message.author_avatar_text.clone())
            })
            .when_some(avatar_image, |this, image| {
                this.child(img(image).size(px(16.0)).rounded(px(2.0)))
            });
        div()
            .h(px(23.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(avatar)
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BLACK)
                    .line_height(px(22.0))
                    .text_color(rgb(palette.attachment_text))
                    .child(shared_message.author_name.clone()),
            )
    }

    fn render_slack_shared_message_footer(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let shared_message = attachment_row
            .shared_message
            .as_ref()
            .expect("shared-message attachment row should include shaped metadata");
        let palette = slack_palette(self.appearance_mode);
        let channel_marker = self.render_slack_shared_message_channel_marker(attachment_row, cx);
        let has_channel_marker = channel_marker.is_some();
        div()
            .mt(px(8.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .text_size(px(12.0))
            .text_color(rgb(palette.attachment_muted_text))
            .when_some(channel_marker, |this, channel_marker| {
                this.child("Posted in").child(channel_marker)
            })
            .when(
                has_channel_marker && shared_message.timestamp_label.is_some(),
                |this| this.child("|"),
            )
            .when_some(
                shared_message.timestamp_label.clone(),
                |this, timestamp_label| this.child(timestamp_label),
            )
            .when(
                shared_message.permalink.is_some()
                    && (has_channel_marker || shared_message.timestamp_label.is_some()),
                |this| this.child("|"),
            )
            .when(shared_message.permalink.is_some(), |this| {
                this.child(div().text_color(rgb(palette.link)).child("View message"))
            })
    }

    fn render_slack_shared_message_channel_marker(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let shared_message = attachment_row
            .shared_message
            .as_ref()
            .expect("shared-message attachment row should include shaped metadata");
        let channel_label = shared_message.channel_label.clone()?;
        let channel_id = attachment_row
            .attachment
            .legacy_metadata()
            .and_then(|metadata| metadata.channel_id.clone())
            .expect("shared-message attachment should include a channel ID");
        let color = slack_palette(self.appearance_mode).attachment_muted_text;
        Some(
            div()
                .h(px(18.0))
                .flex()
                .items_center()
                .gap(px(1.0))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.select_slack_conversation(&channel_id, cx);
                    }),
                )
                .child(slack_icon(SlackShellIcon::HashSmall, color, 12.0, cx))
                .child(channel_label),
        )
    }
}
