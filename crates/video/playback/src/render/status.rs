use gpui::{
    div, prelude::FluentBuilder, px, rgb, AnyElement, Div, IntoElement, ParentElement,
    SharedString, Styled,
};
use gpui_components::alpha;

use super::STAGE_MUTED_FOREGROUND;
use crate::{player::VideoPlayer, responsive_element::ResponsiveVideoElement};

impl VideoPlayer {
    pub(super) fn render_frame_or_placeholder(&self) -> Div {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(STAGE_MUTED_FOREGROUND))
            .text_size(px(13.0))
            .child(self.render_frame_surface())
            .when(self.should_render_status_overlay(), |this| {
                this.child(self.render_status_overlay())
            })
    }

    fn render_frame_surface(&self) -> AnyElement {
        let shell = div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0));
        if let Some(session) = self.session.as_ref() {
            let element_id = self
                .selected_source_id
                .as_ref()
                .map(|id| format!("notslack-video-playback-{}", id.as_ref()))
                .unwrap_or_else(|| "notslack-video-playback".to_string());
            return shell
                .child(ResponsiveVideoElement::new(
                    session.video(),
                    element_id,
                    self.config.frame_fit,
                ))
                .into_any_element();
        }
        shell.into_any_element()
    }

    fn render_status_overlay(&self) -> Div {
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .bg(alpha(0x030507, 0.86))
            .child(
                div()
                    .max_w(px(420.0))
                    .text_align(gpui::TextAlign::Center)
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(STAGE_MUTED_FOREGROUND))
                    .child(self.status_label()),
            )
    }

    fn status_label(&self) -> SharedString {
        if let Some(error) = self.error.as_ref() {
            return error.clone();
        }
        if let Some(format) = self.unsupported_format.as_ref() {
            return format!("{format} is not a video log source.").into();
        }
        if self.loading_source_id.is_some() {
            return "Opening video...".into();
        }
        if self.selected_source().is_some() {
            "Video unavailable.".into()
        } else {
            "No video selected.".into()
        }
    }

    fn should_render_status_overlay(&self) -> bool {
        if self.error.is_some() || self.unsupported_format.is_some() || self.session.is_none() {
            return true;
        }
        self.session
            .as_ref()
            .is_some_and(|session| !session.has_seen_frame())
    }
}
