use super::super::{
    div, px, rgb, slack_palette, AnyElement, Context, Div, FontWeight, IntoElement, ParentElement,
    Styled, SurfaceState,
};

mod composer;
mod list;
mod status;

const SLACK_ALL_THREADS_HEADER_HEIGHT: f32 = 49.0;

impl SurfaceState {
    pub(super) fn render_slack_all_threads_surface(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(palette.main_bg))
            .child(slack_all_threads_header(
                palette.main_border,
                palette.main_text,
            ))
            .child(self.render_slack_all_threads_content(cx))
    }

    fn render_slack_all_threads_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_all_threads_rows.is_empty() {
            if self.slack_all_threads_loading {
                return self.render_slack_all_threads_loading().into_any_element();
            }
            if let Some(error) = self.slack_all_threads_error.as_deref() {
                return self
                    .render_slack_all_threads_error(error, cx)
                    .into_any_element();
            }
            return self.render_slack_all_threads_empty().into_any_element();
        }
        self.render_slack_all_threads_list(cx)
    }
}

fn slack_all_threads_header(border: u32, text: u32) -> Div {
    div()
        .h(px(SLACK_ALL_THREADS_HEADER_HEIGHT))
        .flex_none()
        .px(px(18.0))
        .border_b_1()
        .border_color(rgb(border))
        .flex()
        .items_center()
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(text))
                .child("Threads"),
        )
}

fn all_threads_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
