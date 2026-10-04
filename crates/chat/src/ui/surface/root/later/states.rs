use crate::ui::surface::{
    div, img, px, rgb, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackLaterRow, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use crate::ui::{SlackAttachment, SlackLaterFilter};
use gpui::Role;

use super::helpers::later_action_key;

impl SurfaceState {
    pub(super) fn render_slack_later_detail_loading(
        &self,
        row: &SlackLaterRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("Later", row.type_label.clone(), cx))
            .child(self.render_slack_later_thread_loading_skeleton(&palette))
            .into_any_element()
    }

    pub(super) fn render_slack_later_detail_hydration_error(
        &self,
        row: &SlackLaterRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("Later", row.type_label.clone(), cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .px(px(32.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .child(
                        div().text_size(px(14.0)).text_color(rgb(0xf2d8d6)).child(
                            row.preview
                                .clone()
                                .unwrap_or_else(|| "Couldn’t load this saved item.".into()),
                        ),
                    )
                    .child(self.render_slack_later_hydration_retry(row, cx)),
            )
            .into_any_element()
    }

    fn render_slack_later_hydration_retry(
        &self,
        row: &SlackLaterRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = row.key.clone();
        let keyboard_key = key.clone();
        div()
            .id("slack-later-item-retry")
            .role(Role::Button)
            .aria_label("Retry loading saved item")
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .bg(rgb(0x1264a3))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_later_row(key.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event, _, cx| {
                if later_action_key(event) {
                    cx.stop_propagation();
                    this.select_slack_later_row(keyboard_key.clone(), cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child("Retry")
    }

    pub(super) fn render_slack_later_file_detail(
        &self,
        attachment: &SlackAttachment,
        row: &SlackLaterRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let image = attachment
            .preview_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("File", None, cx))
            .child(
                div()
                    .id("slack-later-file-detail-scroll")
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .p(px(24.0))
                    .overflow_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.0))
                    .when_some(image, |this, image| {
                        this.child(
                            div()
                                .max_w(px(720.0))
                                .max_h(px(520.0))
                                .rounded(px(8.0))
                                .overflow_hidden()
                                .child(img(image).max_w_full().max_h(px(520.0))),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf8f8f8))
                            .child(row.title.clone()),
                    )
                    .when_some(row.preview.clone(), |this, preview| {
                        this.child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(0xb9babd))
                                .child(preview),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_slack_later_skeleton(&self) -> AnyElement {
        div()
            .size_full()
            .px(px(12.0))
            .py(px(16.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .children((0..5).map(|index| {
                div()
                    .id(format!("slack-later-skeleton-{index}"))
                    .h(px(68.0))
                    .rounded(px(8.0))
                    .bg(rgb(0x24272c))
            }))
            .into_any_element()
    }

    pub(super) fn render_slack_later_empty(&self) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(32.0))
            .text_size(px(15.0))
            .text_color(rgb(0xb9babd))
            .child(match self.slack_later_filter {
                SlackLaterFilter::Saved => "No items in progress.",
                SlackLaterFilter::Archived => "No archived items.",
                SlackLaterFilter::Completed => "No completed items.",
            })
            .into_any_element()
    }

    pub(super) fn render_slack_later_error(
        &self,
        error: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .px(px(32.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0xf2d8d6))
                    .child(error.to_string()),
            )
            .child(
                div()
                    .id("slack-later-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading saved items")
                    .focusable()
                    .tab_stop(true)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(6.0))
                    .bg(rgb(0x1264a3))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_later(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if later_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.retry_slack_later(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Retry"),
            )
            .into_any_element()
    }
}
