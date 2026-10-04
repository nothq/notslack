use std::sync::Arc;

use gpui::{
    div, list, prelude::FluentBuilder, px, Context, Div, InteractiveElement, IntoElement,
    ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement, SharedString, Styled,
};
use theme::{ActiveTheme, ThemeColors};

use crate::{
    player::{VideoPlayer, PLAYLIST_ROW_HEIGHT_PX},
    source::VideoPlayerSource,
};

impl VideoPlayer {
    pub(crate) fn render_playlist(&self, cx: &mut Context<Self>) -> Div {
        let sources = Arc::<[VideoPlayerSource]>::from(self.sources.clone());
        let selected_id = self.selected_source_id.clone();
        let view = cx.entity();
        let colors = cx.theme().colors();
        div()
            .w(px(240.0))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .flex()
            .flex_col()
            .child(render_playlist_header(colors))
            .child(
                list(
                    self.playlist_list_state.clone(),
                    move |index, _window, cx| {
                        let sources = sources.clone();
                        let selected_id = selected_id.clone();
                        view.update(cx, |this, cx| {
                            let source = sources
                                .get(index)
                                .expect("video playlist row index should exist");
                            this.render_playlist_row(source, selected_id.as_ref(), cx)
                                .into_any_element()
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full(),
            )
    }

    fn render_playlist_row(
        &self,
        source: &VideoPlayerSource,
        selected_id: Option<&SharedString>,
        cx: &mut Context<Self>,
    ) -> Div {
        let active = selected_id.is_some_and(|id| id.as_ref() == source.id.as_ref());
        let source_id = source.id.clone();
        let colors = cx.theme().colors();
        div()
            .h(px(PLAYLIST_ROW_HEIGHT_PX))
            .flex_none()
            .px(px(12.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(colors.border)
            .when(active, |this| this.bg(colors.element_selected))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.select_source(source_id.clone(), cx);
                }),
            )
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(2.0))
            .child(render_playlist_row_title(source, active, colors))
            .child(render_playlist_row_subtitle(source, colors))
    }
}

fn render_playlist_header(colors: &ThemeColors) -> Div {
    div()
        .h(px(42.0))
        .flex_none()
        .px(px(14.0))
        .flex()
        .items_center()
        .border_b_1()
        .border_color(colors.border)
        .text_size(px(12.0))
        .line_height(px(16.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child("Video log")
}

fn render_playlist_row_title(
    source: &VideoPlayerSource,
    active: bool,
    colors: &ThemeColors,
) -> Div {
    div()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(13.0))
        .line_height(px(16.0))
        .font_weight(if active {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::MEDIUM
        })
        .text_color(if active {
            colors.text_accent
        } else {
            colors.text
        })
        .child(source.title.clone())
}

fn render_playlist_row_subtitle(source: &VideoPlayerSource, colors: &ThemeColors) -> Div {
    div()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(11.0))
        .line_height(px(14.0))
        .text_color(colors.text_muted)
        .child(source.subtitle.clone())
}
