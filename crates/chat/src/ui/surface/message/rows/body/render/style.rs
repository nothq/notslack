use gpui::{
    FontStyle, HighlightStyle, SharedString, StrikethroughStyle, StyledText, TextStyle,
    UnderlineStyle,
};

use crate::ui::surface::{
    slack_palette, SlackMessageBodyBlock, SlackMessageBodyBlockKind, SlackMessageBodyText,
    SlackMessageRenderContext, SlackMessageTextStyle,
};
use crate::ui::{alpha, px, rgb, AppearanceMode, FontWeight};

#[derive(Clone, Copy)]
pub(in crate::ui::surface::message) struct SlackPreparedMessageBodyStyle {
    pub(super) appearance_mode: AppearanceMode,
    pub(super) render_context: SlackMessageRenderContext,
    base_color: u32,
    base_font_weight: FontWeight,
}

impl SlackPreparedMessageBodyStyle {
    pub(in crate::ui::surface::message) fn new(
        appearance_mode: AppearanceMode,
        render_context: SlackMessageRenderContext,
        base_color: u32,
        base_font_weight: FontWeight,
    ) -> Self {
        Self {
            appearance_mode,
            render_context,
            base_color,
            base_font_weight,
        }
    }
}

struct SlackMessageFormattedText<'a> {
    text: &'a SharedString,
    styles: &'a [crate::ui::surface::SlackMessageBodyStyleRange],
    code_ranges: &'a [std::ops::Range<usize>],
    code_font_family: &'a SharedString,
}

pub(super) fn styled_slack_message_body_block(
    block: &SlackMessageBodyBlock,
    style: SlackPreparedMessageBodyStyle,
) -> StyledText {
    styled_slack_message_text(
        SlackMessageFormattedText {
            text: &block.text,
            styles: &block.styles,
            code_ranges: &block.code_ranges,
            code_font_family: &block.code_font_family,
        },
        block.kind,
        style,
    )
}

pub(super) fn styled_slack_message_body_text(
    text: &SlackMessageBodyText,
    block_kind: SlackMessageBodyBlockKind,
    style: SlackPreparedMessageBodyStyle,
) -> StyledText {
    styled_slack_message_text(
        SlackMessageFormattedText {
            text: &text.text,
            styles: &text.styles,
            code_ranges: &text.code_ranges,
            code_font_family: &text.code_font_family,
        },
        block_kind,
        style,
    )
}

fn styled_slack_message_text(
    formatted: SlackMessageFormattedText<'_>,
    block_kind: SlackMessageBodyBlockKind,
    style: SlackPreparedMessageBodyStyle,
) -> StyledText {
    let SlackMessageFormattedText {
        text,
        styles,
        code_ranges,
        code_font_family,
    } = formatted;
    let default_style = slack_message_default_text_style(block_kind, code_font_family, style);
    StyledText::new(text.clone())
        .with_default_highlights(
            &default_style,
            styles.iter().map(|range| {
                (
                    range.range.clone(),
                    slack_message_highlight(range.style, block_kind, style.appearance_mode),
                )
            }),
        )
        .with_font_family_overrides(
            code_ranges
                .iter()
                .cloned()
                .map(|range| (range, code_font_family.clone())),
        )
}

fn slack_message_default_text_style(
    block_kind: SlackMessageBodyBlockKind,
    code_font_family: &SharedString,
    style: SlackPreparedMessageBodyStyle,
) -> TextStyle {
    TextStyle {
        color: rgb(style.base_color).into(),
        font_size: px(match block_kind {
            SlackMessageBodyBlockKind::BlockKitHeader => 18.0,
            SlackMessageBodyBlockKind::BlockKitContext => 13.0,
            _ => 15.0,
        })
        .into(),
        line_height: px(match block_kind {
            SlackMessageBodyBlockKind::BlockKitHeader => 26.0,
            SlackMessageBodyBlockKind::BlockKitContext => 18.0,
            _ => 22.0,
        })
        .into(),
        font_weight: if block_kind == SlackMessageBodyBlockKind::BlockKitHeader {
            FontWeight::BOLD
        } else {
            style.base_font_weight
        },
        font_family: match block_kind {
            SlackMessageBodyBlockKind::Preformatted => code_font_family.clone(),
            SlackMessageBodyBlockKind::Paragraph
            | SlackMessageBodyBlockKind::ListItem { .. }
            | SlackMessageBodyBlockKind::Quote
            | SlackMessageBodyBlockKind::BlockKitSection
            | SlackMessageBodyBlockKind::BlockKitHeader
            | SlackMessageBodyBlockKind::BlockKitContext
            | SlackMessageBodyBlockKind::BlockKitDivider
            | SlackMessageBodyBlockKind::BlockKitActions => "Lato".into(),
        },
        ..Default::default()
    }
}

fn slack_message_highlight(
    style: SlackMessageTextStyle,
    block_kind: SlackMessageBodyBlockKind,
    appearance_mode: AppearanceMode,
) -> HighlightStyle {
    let palette = slack_palette(appearance_mode);
    HighlightStyle {
        color: (style.link || style.mention).then(|| rgb(palette.link).into()),
        font_weight: style.bold.then_some(FontWeight::BOLD),
        font_style: style.italic.then_some(FontStyle::Italic),
        background_color: (style.code && block_kind != SlackMessageBodyBlockKind::Preformatted)
            .then(|| alpha(palette.send_disabled_border, 0.08)),
        underline: style.underline.then(|| UnderlineStyle {
            color: Some(rgb(palette.main_text).into()),
            thickness: px(1.0),
            wavy: false,
        }),
        strikethrough: style.strike.then(|| StrikethroughStyle {
            color: Some(rgb(palette.main_secondary_text).into()),
            thickness: px(1.0),
        }),
        ..Default::default()
    }
}
