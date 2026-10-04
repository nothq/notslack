use crate::ui::surface::{
    slack_icon, slack_palette, SlackMessageBodyAction, SlackMessageBodyActionKind,
    SlackMessageBodyActionStyle, SlackMessageBodyBlock, SlackMessageBodyBlockKind,
    SlackMessageBodyContextElement, SlackMessageBodyImage, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    div, px, rgb, AnyElement, AppearanceMode, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui::{ObjectFit, Role, Stateful, StyledImage};

use super::{
    render_slack_message_body_block_text, render_slack_message_body_text,
    SlackMessageBodyTextContext, SlackPreparedMessageBodyStyle,
};

pub(super) fn render_slack_block_kit_section(
    block: &SlackMessageBodyBlock,
    text_context: SlackMessageBodyTextContext<'_>,
    surface: &SurfaceState,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let style = text_context.style;
    let base_id = block.element_ids.for_context(style.render_context);
    let text = (!block.text.is_empty())
        .then(|| render_slack_message_body_block_text(block, text_context, cx));
    let fields = (!block.fields.is_empty()).then(|| {
        div()
            .max_w(px(600.0))
            .flex()
            .flex_wrap()
            .gap_x(px(12.0))
            .gap_y(px(6.0))
            .children(block.fields.iter().enumerate().map(|(index, field)| {
                div()
                    .w(px(282.0))
                    .min_w(px(0.0))
                    .child(render_slack_message_body_text(
                        field,
                        SlackMessageBodyBlockKind::BlockKitSection,
                        format!("{base_id}.field.{index}").into(),
                        text_context,
                        cx,
                    ))
            }))
    });
    let content = div()
        .min_w(px(0.0))
        .flex_grow(1.0)
        .flex()
        .flex_col()
        .gap(px(6.0))
        .when_some(text, |this, text| this.child(text))
        .when_some(fields, |this, fields| this.child(fields));
    div()
        .max_w(px(600.0))
        .flex()
        .items_start()
        .gap(px(12.0))
        .child(content)
        .when_some(block.accessory_image.as_ref(), |this, image| {
            this.child(render_slack_block_kit_image(
                surface,
                image,
                80.0,
                80.0,
                style.appearance_mode,
            ))
        })
        .into_any_element()
}

pub(super) fn render_slack_block_kit_context(
    block: &SlackMessageBodyBlock,
    text_context: SlackMessageBodyTextContext<'_>,
    surface: &SurfaceState,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let style = text_context.style;
    let base_id = block.element_ids.for_context(style.render_context);
    div()
        .max_w(px(600.0))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.0))
        .text_color(rgb(slack_palette(style.appearance_mode).main_secondary_text))
        .children(
            block
                .context_elements
                .iter()
                .enumerate()
                .map(|(index, element)| match element {
                    SlackMessageBodyContextElement::Text(text) => render_slack_message_body_text(
                        text,
                        SlackMessageBodyBlockKind::BlockKitContext,
                        format!("{base_id}.context.{index}").into(),
                        SlackMessageBodyTextContext {
                            style: SlackPreparedMessageBodyStyle::new(
                                style.appearance_mode,
                                style.render_context,
                                slack_palette(style.appearance_mode).main_secondary_text,
                                FontWeight::NORMAL,
                            ),
                            ..text_context
                        },
                        cx,
                    ),
                    SlackMessageBodyContextElement::Image(image) => render_slack_block_kit_image(
                        surface,
                        image,
                        20.0,
                        20.0,
                        style.appearance_mode,
                    )
                    .into_any_element(),
                }),
        )
        .into_any_element()
}

fn render_slack_block_kit_image(
    surface: &SurfaceState,
    image: &SlackMessageBodyImage,
    width: f32,
    height: f32,
    appearance_mode: AppearanceMode,
) -> Div {
    let palette = slack_palette(appearance_mode);
    let preview = surface.slack_attachment_preview_image_by_key(Some(image.image_url.as_ref()));
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(4.0))
        .overflow_hidden()
        .border_1()
        .border_color(rgb(palette.attachment_border))
        .bg(rgb(palette.attachment_bg))
        .when_some(preview, |this, preview| {
            this.child(
                crate::ui::img(preview)
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
        })
}

pub(super) fn render_slack_block_kit_actions(
    block: &SlackMessageBodyBlock,
    style: SlackPreparedMessageBodyStyle,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let base_id = block.element_ids.for_context(style.render_context);
    div()
        .max_w(px(600.0))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .children(block.actions.iter().enumerate().map(|(index, action)| {
            render_slack_block_kit_action(
                action,
                format!("{base_id}.action.{index}"),
                style.appearance_mode,
                cx,
            )
        }))
        .into_any_element()
}

fn render_slack_block_kit_action(
    action: &SlackMessageBodyAction,
    element_id: String,
    appearance_mode: AppearanceMode,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let colors = slack_block_kit_action_colors(action.style, appearance_mode);
    let control = slack_block_kit_action_control(action, element_id, colors, cx);
    let Some(url) = action.url.clone() else {
        return control.into_any_element();
    };
    let keyboard_url = url.clone();
    control
        .focusable()
        .tab_stop(true)
        .cursor_pointer()
        .focus_visible(|style| style.border_1().border_color(rgb(0x1d9bd1)))
        .on_click(cx.listener(move |surface, _, _, cx| {
            surface.open_slack_link(url.as_ref(), cx);
        }))
        .on_key_down(
            cx.listener(move |surface, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                surface.open_slack_link(keyboard_url.as_ref(), cx);
            }),
        )
        .into_any_element()
}

#[derive(Clone, Copy)]
struct SlackBlockKitActionColors {
    background: u32,
    border: u32,
    text: u32,
}

fn slack_block_kit_action_colors(
    style: SlackMessageBodyActionStyle,
    appearance_mode: AppearanceMode,
) -> SlackBlockKitActionColors {
    let palette = slack_palette(appearance_mode);
    let (background, border, text) = match style {
        SlackMessageBodyActionStyle::Default => (
            palette.attachment_bg,
            palette.attachment_border,
            palette.main_text,
        ),
        SlackMessageBodyActionStyle::Primary => (0x007a5a, 0x007a5a, 0xffffff),
        SlackMessageBodyActionStyle::Danger => (palette.attachment_bg, 0xe01e5a, 0xe01e5a),
    };
    SlackBlockKitActionColors {
        background,
        border,
        text,
    }
}

fn slack_block_kit_action_control(
    action: &SlackMessageBodyAction,
    element_id: String,
    colors: SlackBlockKitActionColors,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    div()
        .id(element_id)
        .role(Role::Button)
        .aria_label(action.label.clone())
        .h(px(30.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(rgb(colors.border))
        .bg(rgb(colors.background))
        .text_size(px(13.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(colors.text))
        .when(
            action.kind != SlackMessageBodyActionKind::Overflow,
            |this| this.child(action.label.clone()),
        )
        .when(slack_block_kit_action_has_menu(action.kind), |this| {
            this.child(slack_icon(
                SlackShellIcon::ChevronDown,
                colors.text,
                14.0,
                cx,
            ))
        })
        .when(
            action.kind == SlackMessageBodyActionKind::Overflow,
            |this| this.child(slack_icon(SlackShellIcon::More, colors.text, 16.0, cx)),
        )
}

fn slack_block_kit_action_has_menu(kind: SlackMessageBodyActionKind) -> bool {
    matches!(
        kind,
        SlackMessageBodyActionKind::Select
            | SlackMessageBodyActionKind::DatePicker
            | SlackMessageBodyActionKind::TimePicker
    )
}
