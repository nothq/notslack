use std::{ops::Range, sync::Arc};

use crate::ui::surface::{
    slack_palette, SlackReactionPickerEmoji, SlackReactionPickerEmojiPresentation,
    SlackReactionPickerIdentity, SlackReactionPickerListRow, SlackReactionPickerState,
    SurfaceState,
};
use crate::ui::{
    div, img, list, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ListSizingBehavior, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{canvas, point, Role, ShapedLine, SharedString, TextAlign};

use super::{skin_tone::slack_reaction_picker_glyph, SLACK_REACTION_PICKER_LIST_HEIGHT};

const SLACK_REACTION_PICKER_CELL_WIDTH: f32 = 36.0;
const SLACK_REACTION_PICKER_CELL_HEIGHT: f32 = 32.0;
const SLACK_REACTION_PICKER_GRID_HEIGHT: f32 = 33.0;
const SLACK_REACTION_PICKER_GRID_LEFT_PADDING: f32 = 15.0;
const SLACK_REACTION_PICKER_GLYPH_SIZE: f32 = 22.0;

struct SlackReactionPickerGlyphSlice {
    cell_offset: usize,
    byte_range: Range<usize>,
}

struct SlackReactionPickerPreparedGlyph {
    cell_offset: usize,
    line: ShapedLine,
}

type SlackReactionPickerGlyphSlices = (SharedString, Vec<SlackReactionPickerGlyphSlice>);

impl SurfaceState {
    pub(super) fn render_slack_reaction_picker_list(
        &self,
        picker: &SlackReactionPickerState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let view = cx.entity();
        let identity = picker.identity();
        list(picker.list_state.clone(), move |index, _window, cx| {
            view.update(cx, |this, cx| {
                let (emojis, row) = {
                    let picker = this
                        .slack_reaction_picker
                        .as_ref()
                        .filter(|picker| picker.identity == identity)
                        .expect("rendered Slack reaction picker target should remain current");
                    let row = picker
                        .rows
                        .get(index)
                        .expect("Slack reaction picker row index should exist")
                        .clone();
                    (picker.emojis.clone(), row)
                };
                this.render_slack_reaction_picker_list_row(&identity, &emojis, row, cx)
            })
        })
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .w_full()
        .h(px(SLACK_REACTION_PICKER_LIST_HEIGHT))
        .bg(rgb(palette.main_bg))
        .into_any_element()
    }

    fn render_slack_reaction_picker_list_row(
        &mut self,
        identity: &SlackReactionPickerIdentity,
        emojis: &Arc<[SlackReactionPickerEmoji]>,
        row: SlackReactionPickerListRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match row {
            SlackReactionPickerListRow::Heading(label) => div()
                .h(px(27.0))
                .px(px(21.0))
                .flex()
                .items_center()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(slack_palette(self.appearance_mode).main_text))
                .child(label)
                .into_any_element(),
            SlackReactionPickerListRow::EmojiGrid(range) => {
                self.render_slack_reaction_picker_emoji_grid(identity, emojis, range, cx)
            }
            SlackReactionPickerListRow::Empty(label) => div()
                .h(px(52.0))
                .px(px(21.0))
                .flex()
                .items_center()
                .text_size(px(13.0))
                .text_color(rgb(slack_palette(self.appearance_mode).main_secondary_text))
                .child(label)
                .into_any_element(),
        }
    }

    fn render_slack_reaction_picker_emoji_grid(
        &mut self,
        identity: &SlackReactionPickerIdentity,
        emojis: &Arc<[SlackReactionPickerEmoji]>,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let grid_id = range.start;
        let visible_image_urls = range
            .clone()
            .filter_map(|index| emojis.get(index))
            .filter_map(SlackReactionPickerEmoji::remote_image_url)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in visible_image_urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        let glyph_row = slack_reaction_picker_glyph_row(
            emojis,
            range.clone(),
            self.slack_preferred_skin_tone_selection(),
        );
        let grid = div()
            .id(("slack-reaction-picker-grid", grid_id))
            .role(Role::Grid)
            .aria_label("Emoji reactions")
            .relative()
            .h(px(SLACK_REACTION_PICKER_GRID_HEIGHT))
            .pl(px(SLACK_REACTION_PICKER_GRID_LEFT_PADDING))
            .flex()
            .items_start()
            .children(range.map(|index| {
                self.render_slack_reaction_picker_emoji_cell(
                    identity,
                    index,
                    emojis
                        .get(index)
                        .expect("Slack reaction picker emoji index should exist"),
                    cx,
                )
            }));
        match glyph_row {
            Some(glyph_row) => grid.child(glyph_row),
            None => grid,
        }
        .into_any_element()
    }

    fn render_slack_reaction_picker_emoji_cell(
        &self,
        identity: &SlackReactionPickerIdentity,
        index: usize,
        emoji: &SlackReactionPickerEmoji,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let click_identity = identity.clone();
        let keyboard_identity = identity.clone();
        let reaction_name = emoji.name.clone();
        let keyboard_reaction_name = reaction_name.clone();
        let skin_tone_support = emoji.skin_tone_support;
        let cell = div()
            .id(("slack-reaction-picker-emoji", index))
            .role(Role::GridCell)
            .aria_label(emoji.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .w(px(SLACK_REACTION_PICKER_CELL_WIDTH))
            .h(px(SLACK_REACTION_PICKER_CELL_HEIGHT))
            .flex_none()
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.reaction_picker_focus_bg)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_reaction_picker_emoji(
                    &click_identity,
                    &reaction_name,
                    skin_tone_support,
                    cx,
                );
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_reaction_picker_cell_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_reaction_picker_emoji(
                        &keyboard_identity,
                        &keyboard_reaction_name,
                        skin_tone_support,
                        cx,
                    );
                }
            }));
        self.render_slack_reaction_picker_cell_content(cell, emoji)
    }

    fn render_slack_reaction_picker_cell_content(
        &self,
        cell: gpui::Stateful<gpui::Div>,
        emoji: &SlackReactionPickerEmoji,
    ) -> gpui::Stateful<gpui::Div> {
        let SlackReactionPickerEmojiPresentation::RemoteImage { url } = &emoji.presentation else {
            return cell;
        };
        match self.slack_remote_images.get(url.as_ref()).cloned() {
            Some(image) => cell.child(
                img(image)
                    .size(px(SLACK_REACTION_PICKER_GLYPH_SIZE))
                    .into_any_element(),
            ),
            None => cell,
        }
    }
}

fn slack_reaction_picker_glyph_row(
    emojis: &Arc<[SlackReactionPickerEmoji]>,
    range: Range<usize>,
    preferred_skin_tone: Option<crate::ui::SlackSkinTone>,
) -> Option<AnyElement> {
    let (text, slices) =
        prepare_slack_reaction_picker_glyph_slices(emojis, range, preferred_skin_tone)?;
    Some(
        canvas(
            move |_bounds, window, _cx| {
                let line = window.text_system().shape_line(
                    text.clone(),
                    px(SLACK_REACTION_PICKER_GLYPH_SIZE),
                    &[window.text_style().to_run(text.len())],
                    None,
                );
                slices
                    .into_iter()
                    .map(|slice| {
                        let (_, suffix) = line.split_at(slice.byte_range.start);
                        let (line, _) =
                            suffix.split_at(slice.byte_range.end - slice.byte_range.start);
                        SlackReactionPickerPreparedGlyph {
                            cell_offset: slice.cell_offset,
                            line,
                        }
                    })
                    .collect::<Vec<_>>()
            },
            move |bounds, glyphs, window, cx| {
                for glyph in glyphs {
                    paint_slack_reaction_picker_glyph(bounds, glyph, window, cx);
                }
            },
        )
        .absolute()
        .inset_0()
        .into_any_element(),
    )
}

fn prepare_slack_reaction_picker_glyph_slices(
    emojis: &Arc<[SlackReactionPickerEmoji]>,
    range: Range<usize>,
    preferred_skin_tone: Option<crate::ui::SlackSkinTone>,
) -> Option<SlackReactionPickerGlyphSlices> {
    let mut text = String::new();
    let mut slices = Vec::new();
    for (cell_offset, index) in range.enumerate() {
        if let SlackReactionPickerEmojiPresentation::Glyph(glyph) = &emojis
            .get(index)
            .expect("Slack reaction picker emoji index should exist")
            .presentation
        {
            let start = text.len();
            text.push_str(slack_reaction_picker_glyph(
                glyph,
                emojis
                    .get(index)
                    .expect("Slack reaction picker emoji index should exist")
                    .skin_tone_support,
                preferred_skin_tone,
            ));
            slices.push(SlackReactionPickerGlyphSlice {
                cell_offset,
                byte_range: start..text.len(),
            });
        }
        text.push(' ');
    }
    if slices.is_empty() {
        return None;
    }
    Some((SharedString::from(text), slices))
}

fn paint_slack_reaction_picker_glyph(
    bounds: gpui::Bounds<gpui::Pixels>,
    glyph: SlackReactionPickerPreparedGlyph,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let cell_left = bounds.origin.x
        + px(SLACK_REACTION_PICKER_GRID_LEFT_PADDING
            + glyph.cell_offset as f32 * SLACK_REACTION_PICKER_CELL_WIDTH);
    let origin = point(
        cell_left + (px(SLACK_REACTION_PICKER_CELL_WIDTH) - glyph.line.width()) / 2.0,
        bounds.origin.y
            + px((SLACK_REACTION_PICKER_CELL_HEIGHT - SLACK_REACTION_PICKER_GLYPH_SIZE) / 2.0),
    );
    let _ = glyph.line.paint(
        origin,
        px(SLACK_REACTION_PICKER_GLYPH_SIZE),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

fn slack_reaction_picker_cell_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
