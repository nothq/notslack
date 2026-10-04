use std::{ops::Range, sync::Arc};

use gpui::{font, px, Font, Pixels, SharedString, UnderlineStyle, Window, WrappedLine};

use super::display::{build_display_text, DisplayOptions};
use super::highlight::clip_highlights;
use super::{layout_visual_line_count, TextLayoutCache, TextLayoutLine};
use crate::text_input::{
    obscured_text, TextDisplayOffset, TextInput, TextInputHighlight, TextInputMode,
    TextInputWrapMode,
};

mod runs;

use runs::text_runs_for_text;

pub(in crate::text_input) fn hard_line_count(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count() + 1
}

pub(in crate::text_input) fn shape_text_layout(
    input: &TextInput,
    width: Pixels,
    render_placeholder: bool,
    window: &mut Window,
) -> TextLayoutCache {
    let shaping = shaping_text(input, render_placeholder);
    let color = if render_placeholder {
        input.style.placeholder
    } else {
        input.style.text
    };
    let mut input_font = input
        .style
        .font_family
        .as_ref()
        .map_or_else(Font::default, |family| font(family.clone()));
    input_font.weight = input.font_weight;
    let marked_range = (!render_placeholder && !input.obscured)
        .then(|| input.marked_range.clone())
        .flatten()
        .map(|range| display_range(shaping.display_offsets.as_deref(), range));
    let highlights = shaping_highlights(input, &shaping, render_placeholder);
    let runs = text_runs_for_text(
        shaping.source.as_ref(),
        input_font,
        color,
        marked_range,
        Some(&highlights),
    );
    let wrap_width = shaping_wrap_width(input, width);
    let shaped = window
        .text_system()
        .shape_text(
            shaping.source.clone(),
            input.style.font_size,
            &runs,
            wrap_width,
            None,
        )
        .expect("text input shaping failed");
    let lines = shape_layout_lines(
        input,
        shaped.into_vec(),
        shaping.display_offsets,
        shaping.ghost,
    );
    let visual_line_count = layout_visual_line_count(&lines);
    let content_width = text_layout_content_width(&lines, input.style.line_height);

    TextLayoutCache {
        revision: input.layout_revision,
        width,
        render_placeholder,
        lines: lines.into(),
        visual_line_count,
        content_width,
    }
}

fn text_layout_content_width(lines: &[TextLayoutLine], line_height: Pixels) -> Pixels {
    lines
        .iter()
        .map(|line| line.line.size(line_height).width)
        .fold(px(0.0), |width, line_width| width.max(line_width))
}

fn shaping_wrap_width(input: &TextInput, width: Pixels) -> Option<Pixels> {
    match input.mode {
        TextInputMode::SingleLine => None,
        TextInputMode::Multiline { .. } => match input.wrap_mode {
            TextInputWrapMode::SoftWrap => Some(width.max(px(0.0))),
            TextInputWrapMode::NoWrap => None,
        },
    }
}

/// The text handed to the shaper, with the mapping back to the content and
/// the display ranges owned by atoms and ghost text.
struct ShapingText {
    source: SharedString,
    display_offsets: Option<Arc<[TextDisplayOffset]>>,
    /// Display range of each atom, in atom order.
    atoms: Vec<Range<usize>>,
    ghost: Option<Range<usize>>,
}

impl ShapingText {
    fn plain(source: SharedString) -> Self {
        Self {
            source,
            display_offsets: None,
            atoms: Vec::new(),
            ghost: None,
        }
    }
}

fn shaping_text(input: &TextInput, render_placeholder: bool) -> ShapingText {
    if render_placeholder {
        return ShapingText::plain(input.placeholder.clone());
    }
    if input.obscured {
        let (source, offsets) = obscured_text(input.content.as_ref());
        return ShapingText {
            source: source.into(),
            display_offsets: Some(offsets),
            atoms: Vec::new(),
            ghost: None,
        };
    }
    // Notion hides the completion while an IME composition is open, so the
    // marked text is the only thing after the caret.
    let ghost = input
        .ghost
        .as_ref()
        .filter(|_| input.marked_range.is_none());
    let soft_break_hyphens = input.wrap_mode == TextInputWrapMode::SoftWrap
        && input.wrap_at_hyphens
        && input.content.contains('-')
        && matches!(input.mode, TextInputMode::Multiline { .. });
    if input.atoms.is_empty() && ghost.is_none() && !soft_break_hyphens {
        return ShapingText::plain(input.content.clone());
    }
    let display = build_display_text(
        input.content.as_ref(),
        DisplayOptions {
            atoms: &input.atoms,
            ghost,
            soft_break_hyphens,
        },
    );
    ShapingText {
        source: display.text.into(),
        display_offsets: Some(display.offsets),
        atoms: display.atoms,
        ghost: display.ghost,
    }
}

/// The caller highlights mapped into display space, cut around the atom and
/// ghost ranges, plus the runs those ranges own.
fn shaping_highlights(
    input: &TextInput,
    shaping: &ShapingText,
    render_placeholder: bool,
) -> Vec<TextInputHighlight> {
    if render_placeholder {
        return Vec::new();
    }
    let mapped = input
        .highlights
        .iter()
        .cloned()
        .map(|mut highlight| {
            highlight.range = display_range(shaping.display_offsets.as_deref(), highlight.range);
            highlight
        })
        .collect::<Vec<_>>();
    let mut reserved = shaping.atoms.clone();
    reserved.extend(shaping.ghost.clone());
    reserved.sort_by_key(|range| range.start);
    let mut highlights = clip_highlights(&mapped, &reserved);
    for (index, range) in shaping.atoms.iter().enumerate() {
        append_atom_highlights(&mut highlights, input, index, range);
    }
    if let (Some(ghost), Some(range)) = (input.ghost.as_ref(), shaping.ghost.clone()) {
        highlights.push(TextInputHighlight {
            range,
            color: ghost.color,
            ..TextInputHighlight::default()
        });
    }
    highlights.sort_by_key(|highlight| highlight.range.start);
    highlights
}

fn append_atom_highlights(
    highlights: &mut Vec<TextInputHighlight>,
    input: &TextInput,
    index: usize,
    range: &Range<usize>,
) {
    let atom = &input.atoms[index];
    let prefix_end = (range.start + atom.prefix_len).min(range.end);
    if range.start < prefix_end {
        highlights.push(TextInputHighlight {
            range: range.start..prefix_end,
            color: atom.prefix_color,
            ..TextInputHighlight::default()
        });
    }
    if prefix_end < range.end {
        let underlined = atom.hover_underline && input.hovered_atom == Some(index);
        highlights.push(TextInputHighlight {
            range: prefix_end..range.end,
            color: atom.color,
            underline: underlined.then(|| UnderlineStyle {
                color: Some(atom.color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..TextInputHighlight::default()
        });
    }
}

fn shape_layout_lines(
    input: &TextInput,
    shaped: Vec<WrappedLine>,
    display_offsets: Option<Arc<[TextDisplayOffset]>>,
    ghost: Option<Range<usize>>,
) -> Vec<TextLayoutLine> {
    if let Some(display_offsets) = display_offsets {
        assert!(
            !input.obscured || shaped.len() == 1,
            "obscured single-line input must shape to exactly one hard line"
        );
        let mut display_start = 0;
        return shaped
            .into_iter()
            .map(|line| {
                let display_end = display_start + line.len();
                let content_start = content_offset_at_display(&display_offsets, display_start);
                let content_end = content_offset_at_display(&display_offsets, display_end);
                let line_offsets = local_display_offsets(
                    &display_offsets,
                    display_start,
                    display_end,
                    content_start,
                    content_end,
                );
                let layout_line = TextLayoutLine {
                    start: content_start,
                    end: content_end,
                    line,
                    display_offsets: Some(line_offsets),
                    ghost: local_ghost_range(ghost.as_ref(), display_start, display_end),
                };
                display_start = display_end.saturating_add(1);
                layout_line
            })
            .collect();
    }
    let mut start = 0usize;
    shaped
        .into_iter()
        .map(|line| {
            let end = start + line.len();
            let layout_line = TextLayoutLine {
                start,
                end,
                line,
                display_offsets: None,
                ghost: None,
            };
            start = end.saturating_add(1);
            layout_line
        })
        .collect()
}

/// The ghost range in line-local display offsets, when it lies on this line.
fn local_ghost_range(
    ghost: Option<&Range<usize>>,
    display_start: usize,
    display_end: usize,
) -> Option<Range<usize>> {
    ghost
        .filter(|ghost| ghost.start >= display_start && ghost.end <= display_end)
        .map(|ghost| ghost.start - display_start..ghost.end - display_start)
}

fn display_offset_for_content(offsets: &[TextDisplayOffset], content: usize) -> usize {
    offsets
        .binary_search_by_key(&content, |offset| offset.content)
        .map(|index| offsets[index].display)
        .unwrap_or_else(|index| offsets[index.saturating_sub(1)].display)
}

/// The content offset a display offset stands for. Several content boundaries
/// share the display offset of an atom, so both an exact hit and a position
/// inside the atom's display text resolve to the atom's own start, never to a
/// boundary inside it.
pub(in crate::text_input) fn content_offset_at_display(
    offsets: &[TextDisplayOffset],
    display: usize,
) -> usize {
    let index = offsets.partition_point(|offset| offset.display < display);
    if let Some(offset) = offsets
        .get(index)
        .filter(|offset| offset.display == display)
    {
        return offset.content;
    }
    let enclosing = offsets[index.saturating_sub(1)].display;
    let first = offsets.partition_point(|offset| offset.display < enclosing);
    offsets[first].content
}

fn display_range(offsets: Option<&[TextDisplayOffset]>, range: Range<usize>) -> Range<usize> {
    let Some(offsets) = offsets else {
        return range;
    };
    display_offset_for_content(offsets, range.start)..display_offset_for_content(offsets, range.end)
}

fn local_display_offsets(
    offsets: &[TextDisplayOffset],
    display_start: usize,
    display_end: usize,
    content_start: usize,
    content_end: usize,
) -> Arc<[TextDisplayOffset]> {
    let mut local = Vec::with_capacity(content_end.saturating_sub(content_start) + 1);
    local.push(TextDisplayOffset {
        content: 0,
        display: 0,
    });
    for offset in offsets {
        if offset.content > content_start
            && offset.content <= content_end
            && offset.display >= display_start
            && offset.display <= display_end
        {
            local.push(TextDisplayOffset {
                content: offset.content - content_start,
                display: offset.display - display_start,
            });
        }
    }
    if local
        .last()
        .is_none_or(|offset| offset.content != content_end - content_start)
    {
        local.push(TextDisplayOffset {
            content: content_end - content_start,
            display: display_end - display_start,
        });
    }
    local.into()
}
