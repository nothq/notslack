use super::super::super::super::{
    div, px, rgb, slack_palette, Div, FontWeight, ParentElement, SlackPalette, SlackSearchRow,
    SlackSearchTextHighlightKind, Styled, SurfaceState,
};
use crate::ui::AppearanceMode;
use gpui::{HighlightStyle, StyledText, TextStyle};

impl SurfaceState {
    pub(super) fn styled_slack_search_body(
        &self,
        row: &SlackSearchRow,
        font_size: f32,
        line_height: f32,
    ) -> StyledText {
        let palette = slack_palette(self.appearance_mode);
        let (match_color, match_background) = match self.appearance_mode {
            AppearanceMode::Light => (0x1d1c1d, 0xffe7a0),
            AppearanceMode::Dark => (0xffb700, 0x4d3f19),
        };
        let default_style = TextStyle {
            color: rgb(palette.main_text).into(),
            font_family: "Lato".into(),
            font_size: px(font_size).into(),
            line_height: px(line_height).into(),
            ..Default::default()
        };
        StyledText::new(row.body_preview.clone()).with_default_highlights(
            &default_style,
            row.text_highlights.iter().map(|highlight| {
                let style = match highlight.kind {
                    SlackSearchTextHighlightKind::Link => HighlightStyle {
                        color: Some(rgb(palette.link).into()),
                        ..Default::default()
                    },
                    SlackSearchTextHighlightKind::QueryMatch => HighlightStyle {
                        color: Some(rgb(match_color).into()),
                        background_color: Some(rgb(match_background).into()),
                        ..Default::default()
                    },
                    SlackSearchTextHighlightKind::LinkAndQueryMatch => HighlightStyle {
                        color: Some(rgb(palette.link).into()),
                        background_color: Some(rgb(match_background).into()),
                        ..Default::default()
                    },
                };
                (highlight.range.clone(), style)
            }),
        )
    }
}

pub(super) fn slack_search_result_metadata(row: &SlackSearchRow, palette: &SlackPalette) -> Div {
    div()
        .h(px(20.0))
        .min_w(px(0.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .flex_none()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.message_author))
                .child(row.author.clone()),
        )
        .child(
            div()
                .flex_grow(1.0)
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(14.0))
                .text_color(rgb(palette.main_muted_text))
                .child(row.conversation_label.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(13.0))
                .text_color(rgb(palette.main_secondary_text))
                .child(row.full_timestamp_label.clone()),
        )
}
