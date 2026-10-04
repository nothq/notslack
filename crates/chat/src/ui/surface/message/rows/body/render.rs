use gpui::{SharedString, StyledText};
use gpui_components::selectable_text::SelectableText;

use crate::ui::surface::message::rows::SlackMessageSelectionContext;
use crate::ui::surface::{
    slack_palette, SlackMessageBody, SlackMessageBodyBlock, SlackMessageBodyBlockKind,
    SlackMessageBodyTarget, SlackMessageBodyText, SlackMessageRenderContext, SlackMessageRow,
    SlackPalette, SurfaceState,
};
use crate::ui::SlackTableRow;
use crate::ui::{
    div, px, rgb, AnyElement, AppearanceMode, Context, Div, FluentBuilder, FontWeight, IntoElement,
    ParentElement, Styled,
};

mod block_kit;
mod style;

use block_kit::{
    render_slack_block_kit_actions, render_slack_block_kit_context, render_slack_block_kit_section,
};
pub(in crate::ui::surface::message) use style::SlackPreparedMessageBodyStyle;
use style::{styled_slack_message_body_block, styled_slack_message_body_text};

/// How body text is styled, and the selection document its text joins, if any.
#[derive(Clone, Copy)]
pub(super) struct SlackMessageBodyTextContext<'a> {
    pub(super) style: SlackPreparedMessageBodyStyle,
    pub(super) selection: Option<&'a SlackMessageSelectionContext>,
}

fn render_slack_message_body_block_text(
    block: &SlackMessageBodyBlock,
    text_context: SlackMessageBodyTextContext<'_>,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let SlackMessageBodyTextContext {
        style,
        selection: selection_context,
    } = text_context;
    let styled_text = styled_slack_message_body_block(block, style);
    let selectable_text = slack_message_selectable_text(
        block.text.clone(),
        styled_text,
        block.element_ids.for_context(style.render_context),
        selection_context,
    );
    with_slack_message_link_clicks(selectable_text, &block.links, &block.link_ranges, cx)
}

fn render_slack_message_body_text(
    text: &SlackMessageBodyText,
    block_kind: SlackMessageBodyBlockKind,
    element_id: SharedString,
    text_context: SlackMessageBodyTextContext<'_>,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let SlackMessageBodyTextContext {
        style,
        selection: selection_context,
    } = text_context;
    let styled_text = styled_slack_message_body_text(text, block_kind, style);
    let selectable_text = slack_message_selectable_text(
        text.text.clone(),
        styled_text,
        element_id,
        selection_context,
    );
    with_slack_message_link_clicks(selectable_text, &text.links, &text.link_ranges, cx)
}

fn slack_message_selectable_text(
    text: SharedString,
    styled_text: StyledText,
    element_id: SharedString,
    selection_context: Option<&SlackMessageSelectionContext>,
) -> SelectableText {
    let document_position =
        selection_context.map(|context| context.next_position(element_id.clone()));
    SelectableText::new(element_id, text, styled_text, rgb(0xb0d4fc).into())
        .when_some(document_position, SelectableText::in_document)
}

fn with_slack_message_link_clicks(
    selectable_text: SelectableText,
    links: &std::sync::Arc<[crate::ui::surface::SlackMessageBodyLink]>,
    link_ranges: &std::sync::Arc<[std::ops::Range<usize>]>,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    if links.is_empty() {
        return selectable_text.into_any_element();
    }
    let links = links.clone();
    let surface = cx.entity().downgrade();
    selectable_text
        .on_link_click(link_ranges.clone(), move |range_index, _window, cx| {
            let Some(link) = links.get(range_index) else {
                return;
            };
            cx.stop_propagation();
            match &link.target {
                SlackMessageBodyTarget::Url(target) => {
                    surface
                        .update(cx, |surface, cx| {
                            surface.open_slack_link(target.as_ref(), cx);
                        })
                        .ok();
                }
                SlackMessageBodyTarget::User(user_id) => {
                    surface
                        .update(cx, |surface, cx| {
                            surface.open_slack_profile(user_id.as_ref(), cx);
                        })
                        .ok();
                }
                SlackMessageBodyTarget::Channel(channel_id) => {
                    surface
                        .update(cx, |surface, cx| {
                            surface.select_slack_conversation(channel_id.as_ref(), cx);
                        })
                        .ok();
                }
            }
        })
        .into_any_element()
}

pub(in crate::ui::surface::message) fn slack_prepared_message_body(
    surface: &SurfaceState,
    body: &SlackMessageBody,
    edited_label: Option<&SharedString>,
    style: SlackPreparedMessageBodyStyle,
    cx: &mut Context<SurfaceState>,
) -> Div {
    slack_prepared_message_body_with_selection(
        surface,
        body,
        edited_label,
        SlackMessageBodyTextContext {
            style,
            selection: None,
        },
        cx,
    )
}

fn slack_prepared_message_body_with_selection(
    surface: &SurfaceState,
    body: &SlackMessageBody,
    edited_label: Option<&SharedString>,
    text_context: SlackMessageBodyTextContext<'_>,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let palette = slack_palette(text_context.style.appearance_mode);
    let last_block_index = body.blocks.len().checked_sub(1);
    div()
        .min_w(px(0.0))
        .max_w(px(1230.0))
        .flex()
        .flex_col()
        .gap(px(3.0))
        .children(body.blocks.iter().enumerate().map(|(block_index, block)| {
            let content = slack_prepared_message_block_content(
                block,
                edited_label.filter(|_| last_block_index == Some(block_index)),
                text_context,
                surface,
                cx,
            );
            slack_prepared_message_block(block, content, palette)
        }))
}

fn slack_prepared_message_block_content(
    block: &SlackMessageBodyBlock,
    trailing_edited_label: Option<&SharedString>,
    text_context: SlackMessageBodyTextContext<'_>,
    surface: &SurfaceState,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let style = text_context.style;
    match block.kind {
        SlackMessageBodyBlockKind::BlockKitSection => {
            return render_slack_block_kit_section(block, text_context, surface, cx);
        }
        SlackMessageBodyBlockKind::BlockKitContext => {
            return render_slack_block_kit_context(block, text_context, surface, cx);
        }
        SlackMessageBodyBlockKind::BlockKitDivider => {
            return div()
                .w_full()
                .max_w(px(600.0))
                .h(px(17.0))
                .border_b_1()
                .border_color(rgb(slack_palette(style.appearance_mode).attachment_border))
                .into_any_element();
        }
        SlackMessageBodyBlockKind::BlockKitActions => {
            return render_slack_block_kit_actions(block, style, cx);
        }
        SlackMessageBodyBlockKind::Paragraph
        | SlackMessageBodyBlockKind::ListItem { .. }
        | SlackMessageBodyBlockKind::Quote
        | SlackMessageBodyBlockKind::Preformatted
        | SlackMessageBodyBlockKind::BlockKitHeader => {}
    }
    let text = render_slack_message_body_block_text(block, text_context, cx);
    let Some(edited_label) = trailing_edited_label else {
        return text;
    };
    let palette = slack_palette(style.appearance_mode);
    div()
        .min_w(px(0.0))
        .flex()
        .flex_wrap()
        .items_baseline()
        .gap(px(4.0))
        .child(text)
        .child(
            div()
                .text_size(px(13.0))
                .line_height(px(22.0))
                .text_color(rgb(palette.main_secondary_text))
                .child(edited_label.clone()),
        )
        .into_any_element()
}

fn slack_prepared_message_block(
    block: &SlackMessageBodyBlock,
    content: AnyElement,
    palette: SlackPalette,
) -> Div {
    div()
        .min_w(px(0.0))
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(palette.main_text))
        .when(
            matches!(
                block.kind,
                SlackMessageBodyBlockKind::ListItem { indent } if indent > 0
            ),
            |this| {
                let SlackMessageBodyBlockKind::ListItem { indent } = block.kind else {
                    return this;
                };
                this.pl(px(f32::from(indent) * 24.0))
            },
        )
        .when(block.kind == SlackMessageBodyBlockKind::Quote, |this| {
            this.pl(px(12.0))
                .border_l_2()
                .border_color(rgb(palette.main_muted_text))
        })
        .when(
            block.kind == SlackMessageBodyBlockKind::Preformatted,
            |this| {
                this.px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(rgb(palette.attachment_border))
                    .bg(rgb(palette.attachment_bg))
            },
        )
        .child(content)
}

fn slack_message_body(
    surface: &SurfaceState,
    row: &SlackMessageRow,
    render_context: SlackMessageRenderContext,
    selection_context: Option<&SlackMessageSelectionContext>,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let appearance_mode = surface.appearance_mode;
    let palette = slack_palette(appearance_mode);
    slack_prepared_message_body_with_selection(
        surface,
        &row.body,
        row.edited_label.as_ref(),
        SlackMessageBodyTextContext {
            style: SlackPreparedMessageBodyStyle::new(
                appearance_mode,
                render_context,
                palette.main_text,
                FontWeight::NORMAL,
            ),
            selection: selection_context,
        },
        cx,
    )
}

pub(in crate::ui::surface) fn slack_message_body_block(
    surface: &SurfaceState,
    row: &SlackMessageRow,
    render_context: SlackMessageRenderContext,
    cx: &mut Context<SurfaceState>,
) -> Div {
    slack_message_body_block_with_selection(surface, row, render_context, None, cx)
}

pub(in crate::ui::surface) fn slack_message_body_block_in_document(
    surface: &SurfaceState,
    row: &SlackMessageRow,
    render_context: SlackMessageRenderContext,
    selection_context: &SlackMessageSelectionContext,
    cx: &mut Context<SurfaceState>,
) -> Div {
    slack_message_body_block_with_selection(
        surface,
        row,
        render_context,
        Some(selection_context),
        cx,
    )
}

fn slack_message_body_block_with_selection(
    surface: &SurfaceState,
    row: &SlackMessageRow,
    render_context: SlackMessageRenderContext,
    selection_context: Option<&SlackMessageSelectionContext>,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let appearance_mode = surface.appearance_mode;
    if row.table_rows.is_empty() {
        return slack_message_body(surface, row, render_context, selection_context, cx);
    }
    div()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .when(row.body.has_renderable_content(), |this| {
            this.child(slack_message_body(
                surface,
                row,
                render_context,
                selection_context,
                cx,
            ))
        })
        .when(!row.table_rows.is_empty(), |this| {
            this.child(slack_message_table(&row.table_rows, appearance_mode))
        })
}

fn slack_message_table(rows: &[SlackTableRow], appearance_mode: AppearanceMode) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .w(px(150.0))
        .overflow_hidden()
        .rounded(px(2.0))
        .border_1()
        .border_color(rgb(palette.attachment_border))
        .bg(rgb(palette.attachment_bg))
        .children(rows.iter().enumerate().map(|(index, row)| {
            div()
                .h(px(18.0))
                .w_full()
                .flex()
                .when(index > 0, |this| {
                    this.border_t_1()
                        .border_color(rgb(palette.attachment_border))
                })
                .child(
                    div()
                        .w(px(28.0))
                        .border_r_1()
                        .border_color(rgb(palette.attachment_border))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.attachment_muted_text))
                        .child(row.index.clone()),
                )
                .child(
                    div()
                        .flex_grow(1.0)
                        .min_w(px(0.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.main_text))
                        .child(row.value.clone()),
                )
        }))
}
