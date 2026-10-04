use std::ops::Range;

use gpui::{fill, point, px, Bounds, Hsla, PaintQuad, Pixels};

use super::{TextLayoutLine, TextViewport};
use crate::text_input::TextInputHighlight;

#[cfg(test)]
mod tests;

/// Cut `highlights` around `reserved`, keeping only the parts that fall
/// outside every reserved range. Both inputs are sorted, non-overlapping
/// display ranges; atoms and ghost text own their runs outright, so a caller
/// highlight may not colour them.
pub(super) fn clip_highlights(
    highlights: &[TextInputHighlight],
    reserved: &[Range<usize>],
) -> Vec<TextInputHighlight> {
    if reserved.is_empty() {
        return highlights.to_vec();
    }
    let mut clipped = Vec::with_capacity(highlights.len());
    for highlight in highlights {
        let mut start = highlight.range.start;
        for range in reserved {
            if range.end <= start {
                continue;
            }
            if range.start >= highlight.range.end {
                break;
            }
            if start < range.start {
                clipped.push(TextInputHighlight {
                    range: start..range.start,
                    ..highlight.clone()
                });
            }
            start = start.max(range.end);
        }
        if start < highlight.range.end {
            clipped.push(TextInputHighlight {
                range: start..highlight.range.end,
                ..highlight.clone()
            });
        }
    }
    clipped
}

pub(in crate::text_input) fn highlight_background_quads(
    highlights: &[TextInputHighlight],
    lines: &[TextLayoutLine],
    viewport: TextViewport,
) -> Vec<PaintQuad> {
    let mut quads = Vec::new();
    let mut highlight_index = 0usize;
    let mut line_top = viewport.bounds.top() - viewport.scroll_y;
    for line in lines {
        while highlights
            .get(highlight_index)
            .is_some_and(|highlight| highlight.range.end <= line.start)
        {
            highlight_index += 1;
        }
        append_line_highlight_background_quads(
            &mut quads,
            &highlights[highlight_index..],
            line,
            viewport,
            line_top,
        );
        line_top += line.line.size(viewport.line_height).height;
    }
    quads
}

fn append_line_highlight_background_quads(
    quads: &mut Vec<PaintQuad>,
    highlights: &[TextInputHighlight],
    line: &TextLayoutLine,
    viewport: TextViewport,
    line_top: Pixels,
) {
    for highlight in highlights {
        if highlight.range.start >= line.end {
            break;
        }
        let Some(color) = highlight.background else {
            continue;
        };
        let Some(range) = line_highlight_display_range(highlight, line) else {
            continue;
        };
        append_visual_highlight_background_quads(
            quads,
            line,
            range,
            HighlightBackgroundQuadContext {
                viewport,
                line_top,
                color,
                corner_radius: highlight.background_corner_radius,
                padding_x: highlight.background_padding_x,
                inset_y: highlight.background_inset_y,
            },
        );
    }
}

fn line_highlight_display_range(
    highlight: &TextInputHighlight,
    line: &TextLayoutLine,
) -> Option<Range<usize>> {
    let start = highlight
        .range
        .start
        .max(line.start)
        .saturating_sub(line.start);
    let end = highlight.range.end.min(line.end).saturating_sub(line.start);
    if start >= end {
        return None;
    }
    let display_end = line.display_offset_for_content(end);
    Some(line.display_offset_for_content(start)..line.extend_display_end_over_ghost(display_end))
}

fn append_visual_highlight_background_quads(
    quads: &mut Vec<PaintQuad>,
    line: &TextLayoutLine,
    range: Range<usize>,
    context: HighlightBackgroundQuadContext,
) {
    let mut visual_start = 0usize;
    for visual_index in 0..=line.line.wrap_boundaries().len() {
        let visual_end = line
            .line
            .wrap_boundaries()
            .get(visual_index)
            .map_or(line.display_len(), |boundary| {
                line.line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index
            });
        let start = range.start.max(visual_start);
        let end = range.end.min(visual_end);
        if start < end {
            if let Some(quad) = highlight_background_quad_for_visual_range(
                line,
                start..end,
                visual_start,
                visual_index,
                context,
            ) {
                quads.push(quad);
            }
        }
        visual_start = visual_end;
    }
}

#[derive(Clone, Copy)]
struct HighlightBackgroundQuadContext {
    viewport: TextViewport,
    line_top: Pixels,
    color: Hsla,
    corner_radius: Pixels,
    padding_x: Pixels,
    inset_y: Pixels,
}

fn highlight_background_quad_for_visual_range(
    line: &TextLayoutLine,
    range: Range<usize>,
    visual_start: usize,
    visual_index: usize,
    context: HighlightBackgroundQuadContext,
) -> Option<PaintQuad> {
    let viewport = context.viewport;
    let top = context.line_top + viewport.line_height * visual_index + context.inset_y;
    let bottom = context.line_top + viewport.line_height * (visual_index + 1) - context.inset_y;
    if bottom <= viewport.bounds.top() || top >= viewport.bounds.bottom() || bottom <= top {
        return None;
    }
    let start_x = if range.start == visual_start {
        px(0.0)
    } else {
        line.line
            .position_for_index(range.start, viewport.line_height)
            .map_or(px(0.0), |position| position.x)
    };
    let end_x = line
        .line
        .position_for_index(range.end, viewport.line_height)
        .map_or(start_x, |position| position.x);
    let left = (viewport.bounds.left() + start_x - viewport.scroll_x - context.padding_x)
        .max(viewport.bounds.left());
    let right = (viewport.bounds.left() + end_x - viewport.scroll_x + context.padding_x)
        .min(viewport.bounds.right());
    (left < right).then(|| {
        fill(
            Bounds::from_corners(
                point(left, top.max(viewport.bounds.top())),
                point(right, bottom.min(viewport.bounds.bottom())),
            ),
            context.color,
        )
        .corner_radii(context.corner_radius)
    })
}
