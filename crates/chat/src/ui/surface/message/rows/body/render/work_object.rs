//! Slack's Work Object card, such as a Jira issue: the app's name over a
//! bordered card with the record's icon, title and subtitle, its labelled
//! fields, and a footer with when it was shared and its buttons.

use crate::model::{SlackWorkObject, SlackWorkObjectValue};
use crate::ui::surface::{slack_palette, SlackMessageBodyBlock, SlackPalette, SurfaceState};
use crate::ui::{
    div, px, rgb, AnyElement, AppearanceMode, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{ObjectFit, StyledImage};

use super::block_kit::render_slack_block_kit_actions;
use super::SlackPreparedMessageBodyStyle;

const CARD_MAX_WIDTH: f32 = 600.0;

pub(super) fn render_slack_work_object(
    block: &SlackMessageBodyBlock,
    style: SlackPreparedMessageBodyStyle,
    surface: &SurfaceState,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let Some(card) = block.work_object.as_deref() else {
        return div().into_any_element();
    };
    let palette = slack_palette(style.appearance_mode);
    let base_id = block.element_ids.for_context(style.render_context);
    div()
        .max_w(px(CARD_MAX_WIDTH))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(work_object_header(card, palette, surface))
        .child(
            div()
                .flex()
                .flex_col()
                .rounded(px(8.0))
                .border_1()
                .border_color(rgb(palette.attachment_border))
                .child(
                    div()
                        .p(px(16.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .child(work_object_title(card, &base_id, palette, surface, cx))
                        .when(!card.fields.is_empty(), |this| {
                            this.child(work_object_fields(card, style.appearance_mode, surface))
                        }),
                )
                .when(card.as_of.is_some() || !block.actions.is_empty(), |this| {
                    this.child(work_object_footer(block, card, style, palette, cx))
                }),
        )
        .into_any_element()
}

fn work_object_header(
    card: &SlackWorkObject,
    palette: SlackPalette,
    surface: &SurfaceState,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .when_some(card.app_icon_url.as_deref(), |this, url| {
            this.child(work_object_image(surface, url, 20.0, 4.0))
        })
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.main_text))
                .child(card.app_name.clone()),
        )
}

fn work_object_title(
    card: &SlackWorkObject,
    base_id: &str,
    palette: SlackPalette,
    surface: &SurfaceState,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let title = div()
        .id(format!("{base_id}.work-object-title"))
        .text_size(px(15.0))
        .line_height(px(22.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(palette.main_text))
        .child(card.title.clone());
    let title = match card.url.clone() {
        Some(url) => title
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(palette.link)).underline())
            .on_click(cx.listener(move |surface, _, _, cx| {
                surface.open_slack_link(&url, cx);
            }))
            .into_any_element(),
        None => title.into_any_element(),
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.0))
        .when_some(card.icon_url.as_deref(), |this, url| {
            this.child(work_object_image(surface, url, 36.0, 6.0))
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .child(title)
                .when_some(card.subtitle.clone(), |this, subtitle| {
                    this.child(
                        div()
                            .text_size(px(13.0))
                            .line_height(px(18.0))
                            .text_color(rgb(palette.main_secondary_text))
                            .child(subtitle),
                    )
                }),
        )
}

fn work_object_fields(
    card: &SlackWorkObject,
    appearance_mode: AppearanceMode,
    surface: &SurfaceState,
) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(32.0))
        .gap_y(px(12.0))
        .children(card.fields.iter().map(|field| {
            div()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.main_secondary_text))
                        .child(field.label.clone()),
                )
                .child(
                    div().flex().flex_wrap().gap(px(6.0)).children(
                        field
                            .values
                            .iter()
                            .map(|value| work_object_value(value, appearance_mode, surface)),
                    ),
                )
        }))
}

fn work_object_value(
    value: &SlackWorkObjectValue,
    appearance_mode: AppearanceMode,
    surface: &SurfaceState,
) -> Div {
    let palette = slack_palette(appearance_mode);
    let row = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(palette.main_text))
        .when_some(value.icon_url.as_deref(), |this, url| {
            this.child(work_object_image(surface, url, 18.0, 9.0))
        })
        .child(value.text.clone());
    match value.color.as_deref().map(slack_tag_color) {
        Some(color) => row
            .px(px(6.0))
            .rounded(px(4.0))
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .bg(gpui::rgba((color << 8) | 0x33))
            .text_color(rgb(color)),
        None => row,
    }
}

fn work_object_footer(
    block: &SlackMessageBodyBlock,
    card: &SlackWorkObject,
    style: SlackPreparedMessageBodyStyle,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .px(px(16.0))
        .py(px(12.0))
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .border_t_1()
        .border_color(rgb(palette.attachment_border))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(palette.main_secondary_text))
                .when_some(card.as_of.as_deref(), |this, as_of| {
                    this.child(format!("As of {as_of}"))
                }),
        )
        .when(!block.actions.is_empty(), |this| {
            this.child(render_slack_block_kit_actions(block, style, cx))
        })
}

fn work_object_image(surface: &SurfaceState, url: &str, size: f32, radius: f32) -> Div {
    let image = surface.slack_attachment_preview_image_by_key(Some(url));
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(radius))
        .overflow_hidden()
        .when_some(image, |this, image| {
            this.child(
                crate::ui::img(image)
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
        })
}

/// Slack's tag colors, by name.
fn slack_tag_color(name: &str) -> u32 {
    match name {
        "lagoon" | "blue" | "horchata" => 0x1d9bd1,
        "forest" | "green" | "jade" => 0x2bac76,
        "red" | "pink" | "cherry" => 0xe01e5a,
        "yellow" | "orange" | "gold" | "mango" => 0xecb22e,
        "purple" | "grape" | "lilac" => 0x9b6bd3,
        _ => 0x8c949d,
    }
}
