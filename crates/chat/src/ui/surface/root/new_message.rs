use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, Div, FontWeight, IntoElement, ParentElement, Styled,
};

use crate::ui::surface::{slack_palette, SurfaceState};
use crate::ui::SlackWorkspace;

mod chips;
mod destination;
mod suggestions;

const SLACK_NEW_MESSAGE_HEADER_HEIGHT: f32 = 49.0;

impl SurfaceState {
    pub(super) fn render_slack_new_message_surface(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let destination_resolved = self.slack_new_message_destination.is_some();
        let composer_presentation = self
            .slack_active_main_composer_context
            .as_ref()
            .map(|context| context.presentation.clone());
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(palette.main_bg))
            .child(self.render_slack_new_message_header())
            .child(self.render_slack_new_message_to_row(workspace, cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .flex()
                    .flex_col()
                    .when_some(self.slack_new_message_error.clone(), |this, error| {
                        this.child(self.render_slack_new_message_error(error))
                    })
                    .when(destination_resolved, |this| {
                        this.child(self.render_slack_main_body(workspace, view, cx))
                    }),
            )
            .when_some(composer_presentation, |this, presentation| {
                this.child(self.render_slack_composer_panel(&presentation, cx))
            })
            .when(self.slack_new_message_suggestions_visible(), |this| {
                this.child(self.render_slack_new_message_suggestions(cx))
            })
            .into_any_element()
    }

    fn render_slack_new_message_header(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_NEW_MESSAGE_HEADER_HEIGHT))
            .flex_none()
            .px(px(18.0))
            .flex()
            .items_center()
            .text_size(px(18.0))
            .line_height(px(24.0))
            .font_weight(FontWeight::BLACK)
            .text_color(rgb(palette.main_text))
            .child("New message")
    }

    fn render_slack_new_message_error(&self, error: String) -> Div {
        div()
            .mx(px(20.0))
            .mt(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(0x6e3c39))
            .bg(rgb(0x341d1d))
            .px(px(10.0))
            .py(px(7.0))
            .text_size(px(12.0))
            .line_height(px(17.0))
            .text_color(rgb(0xf2d8d6))
            .child(error)
    }
}

fn slack_new_message_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
