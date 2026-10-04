use std::sync::Arc;

use gpui::Role;

use super::super::consume_slack_search_action_key;
use crate::ui::surface::{
    div, img, px, rgb, slack_palette, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    ParentElement, SlackMessageActionTarget, SlackPalette, SlackSearchRow,
    StatefulInteractiveElement, Styled, SurfaceState,
};

impl SurfaceState {
    pub(super) fn render_slack_search_thread_action(
        &self,
        row: &SlackSearchRow,
        target: Arc<SlackMessageActionTarget>,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(36.0))
            .flex_none()
            .ml(px(44.0))
            .mt(px(12.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(self.render_slack_search_thread_avatars(row, palette))
            .when_some(row.reply_count, |this, reply_count| {
                this.child(slack_search_reply_count(reply_count, palette.link))
            })
            .when_some(row.latest_reply_label.clone(), |this, label| {
                this.child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child(label),
                )
            })
            .child(self.render_slack_search_thread_button(row, target, palette.link, cx))
    }

    fn render_slack_search_thread_avatars(
        &self,
        row: &SlackSearchRow,
        palette: SlackPalette,
    ) -> Div {
        div().flex().items_center().children(
            row.reply_avatar_image_urls
                .iter()
                .take(4)
                .enumerate()
                .map(|(index, url)| {
                    let image = self.slack_remote_images.get(url.as_ref()).cloned();
                    div()
                        .size(px(24.0))
                        .flex_none()
                        .when(index > 0, |this| this.ml(px(-4.0)))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(palette.composer_bg))
                        .overflow_hidden()
                        .bg(rgb(palette.composer_chip_bg))
                        .when_some(image, |this, image| {
                            this.child(img(image).size(px(24.0)).rounded(px(6.0)))
                        })
                }),
        )
    }

    fn render_slack_search_thread_button(
        &self,
        row: &SlackSearchRow,
        target: Arc<SlackMessageActionTarget>,
        link_color: u32,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let keyboard_target = target.clone();
        div()
            .id(format!("{}-view-thread", row.result_element_id))
            .role(Role::Button)
            .aria_label("View thread")
            .focusable()
            .tab_stop(true)
            .ml(px(4.0))
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(link_color))
            .cursor_pointer()
            .hover(|style| style.underline())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.slack_reaction_picker = None;
                this.activate_slack_search_result_thread(&target, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.slack_reaction_picker = None;
                    this.activate_slack_search_result_thread(&keyboard_target, cx);
                }
            }))
            .child("View thread")
    }
}

fn slack_search_reply_count(reply_count: u32, text_color: u32) -> Div {
    div()
        .text_size(px(14.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(text_color))
        .child(format!(
            "{reply_count} {}",
            if reply_count == 1 { "reply" } else { "replies" }
        ))
}
