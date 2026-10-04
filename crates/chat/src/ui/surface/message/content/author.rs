use std::{ops::Range, sync::Arc};

use crate::ui::surface::{
    div, px, relative, rgb, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackMessageRenderContext,
    SlackMessageRow, StatefulInteractiveElement, Styled, SurfaceState, SLACK_LINE_HEIGHT,
};
use gpui::{HighlightStyle, Role, StyledText, TextStyle};
use gpui_components::selectable_text::SelectableText;

use super::super::rows::{bind_slack_profile_click, slack_message_meta};
use super::super::{slack_message_render_context_id, SlackMessageSelectionContext};

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_message_author(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        selection_context: Option<&SlackMessageSelectionContext>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(selection_context) = selection_context else {
            return self.render_slack_message_author_action(row, cx);
        };
        let palette = slack_palette(self.appearance_mode);
        let author_element_id = format!(
            "slack-message-author-text-{}-{}",
            slack_message_render_context_id(render_context),
            row.id,
        );
        let timestamp_element_id = format!(
            "slack-message-timestamp-text-{}-{}",
            slack_message_render_context_id(render_context),
            row.id,
        );
        div()
            .flex()
            .items_center()
            .min_w(px(0.0))
            .gap(px(8.0))
            .when(!row.author.is_empty(), |this| {
                this.child(self.render_slack_selectable_author(
                    row,
                    author_element_id,
                    selection_context,
                    cx,
                ))
            })
            .when(!row.timestamp.is_empty(), |this| {
                let text: gpui::SharedString = row.timestamp.clone().into();
                let styled_text = slack_message_header_styled_text(
                    text.clone(),
                    palette.main_muted_text,
                    12.0,
                    FontWeight::NORMAL,
                );
                this.child(
                    SelectableText::new(
                        timestamp_element_id.clone(),
                        text,
                        styled_text,
                        rgb(0xb0d4fc).into(),
                    )
                    .in_document(
                        selection_context.next_position_with_separator(
                            timestamp_element_id.clone().into(),
                            "\n\u{a0}\u{a0}",
                        ),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_slack_message_author_action(
        &self,
        row: &SlackMessageRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let author = div()
            .flex()
            .items_center()
            .min_w(px(0.0))
            .gap(px(8.0))
            .child(
                div()
                    .whitespace_nowrap()
                    .text_size(px(15.0))
                    .line_height(relative(SLACK_LINE_HEIGHT))
                    .font_weight(FontWeight::BLACK)
                    .text_color(rgb(palette.message_author))
                    .child(row.author.clone()),
            )
            .when(!row.timestamp.is_empty(), |this| {
                this.child(slack_message_meta(
                    row.timestamp.clone(),
                    self.appearance_mode,
                ))
            });
        bind_slack_profile_click(
            author,
            row.user_id.clone(),
            format!("slack-message-author-{}", row.id),
            format!("Open profile for {}", row.author),
            cx,
        )
    }

    fn render_slack_selectable_author(
        &self,
        row: &SlackMessageRow,
        element_id: String,
        selection_context: &SlackMessageSelectionContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let text: gpui::SharedString = row.author.clone().into();
        let styled_text = slack_message_header_styled_text(
            text.clone(),
            palette.message_author,
            15.0,
            FontWeight::BLACK,
        );
        let selectable = SelectableText::new(
            element_id.clone(),
            text.clone(),
            styled_text,
            rgb(0xb0d4fc).into(),
        )
        .in_document(selection_context.next_position(element_id.clone().into()));
        let Some(user_id) = row.user_id.clone() else {
            return slack_unlinked_selectable_author(selectable);
        };
        let keyboard_user_id = user_id.clone();
        let surface = cx.entity().downgrade();
        let linked_author = selectable.on_link_click(
            std::iter::once(0..text.len()).collect::<Arc<[Range<usize>]>>(),
            move |_, _window, cx| {
                surface
                    .update(cx, |surface, cx| {
                        surface.open_slack_profile(&user_id, cx);
                    })
                    .ok();
            },
        );
        div()
            .id(format!("{element_id}-profile"))
            .role(Role::Button)
            .aria_label(format!("Open profile for {}", row.author))
            .focusable()
            .tab_stop(true)
            .overflow_hidden()
            .whitespace_nowrap()
            .focus_visible(|style| style.border_1().border_color(rgb(0x1d9bd1)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_profile(&keyboard_user_id, cx);
            }))
            .child(linked_author)
            .into_any_element()
    }
}

fn slack_unlinked_selectable_author(selectable: SelectableText) -> AnyElement {
    div()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(selectable)
        .into_any_element()
}

fn slack_message_header_styled_text(
    text: gpui::SharedString,
    color: u32,
    font_size: f32,
    font_weight: FontWeight,
) -> StyledText {
    let style = TextStyle {
        color: rgb(color).into(),
        font_family: "Lato".into(),
        font_size: px(font_size).into(),
        line_height: relative(SLACK_LINE_HEIGHT),
        font_weight,
        ..Default::default()
    };
    StyledText::new(text)
        .with_default_highlights(&style, std::iter::empty::<(Range<usize>, HighlightStyle)>())
}
