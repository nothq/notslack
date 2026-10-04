use super::super::super::{
    div, px, rgb, slack_palette, Context, Div, FontWeight, InteractiveElement, ParentElement,
    Styled, SurfaceState,
};
use gpui::{Role, StatefulInteractiveElement};

impl SurfaceState {
    pub(super) fn render_slack_all_threads_loading(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .text_color(rgb(palette.main_secondary_text))
            .child("Loading threads…")
    }

    pub(super) fn render_slack_all_threads_empty(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(div().text_size(px(42.0)).child("🌱"))
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Tend to your threads"),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child("Threads you’re involved in will be collected right here."),
            )
    }

    pub(super) fn render_slack_all_threads_error(
        &self,
        error: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .child(slack_all_threads_error_message(
                error,
                palette.main_text,
                palette.main_secondary_text,
            ))
            .child(
                div()
                    .id("slack-all-threads-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading Slack threads")
                    .focusable()
                    .tab_stop(true)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(8.0))
                    .bg(rgb(palette.composer_chip_bg))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_all_threads(cx);
                    }))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Retry"),
            )
    }
}

fn slack_all_threads_error_message(error: &str, text: u32, secondary_text: u32) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(text))
                .child("Something’s fishy! Slack can’t seem to load your threads."),
        )
        .child(
            div()
                .max_w(px(560.0))
                .text_size(px(13.0))
                .text_color(rgb(secondary_text))
                .child(error.to_string()),
        )
}
