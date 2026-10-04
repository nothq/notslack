use std::{ops::Range, sync::Arc};

use gpui::{fill, point, px, size, Bounds, Hsla, PaintQuad, Pixels, Point, WrappedLine};

use super::{text_input_caret_width, TextDisplayOffset, TextInputAtom};

mod display;
mod highlight;
mod shaping;

pub(super) use highlight::highlight_background_quads;
pub(super) use shaping::{content_offset_at_display, hard_line_count, shape_text_layout};

pub(super) struct TextLayoutLine {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) line: WrappedLine,
    display_offsets: Option<Arc<[TextDisplayOffset]>>,
    /// Display range of the ghost text on this line, if it falls here.
    ghost: Option<Range<usize>>,
}

impl TextLayoutLine {
    fn display_offset_for_content(&self, offset: usize) -> usize {
        let offset = offset.min(self.end - self.start);
        let Some(offsets) = self.display_offsets.as_ref() else {
            return offset;
        };
        offsets
            .binary_search_by_key(&offset, |boundary| boundary.content)
            .map(|index| offsets[index].display)
            .expect("text offset must follow a UTF-8 character boundary")
    }

    fn content_offset_for_display(&self, offset: usize) -> usize {
        let Some(offsets) = self.display_offsets.as_ref() else {
            return offset.min(self.end - self.start);
        };
        content_offset_at_display(offsets, offset)
    }

    fn display_len(&self) -> usize {
        self.line.len()
    }

    /// Ghost text continues the content at its offset, so a background that
    /// reaches that offset covers the completion too.
    fn extend_display_end_over_ghost(&self, end: usize) -> usize {
        match &self.ghost {
            Some(ghost) if ghost.start == end => ghost.end,
            _ => end,
        }
    }
}

pub(super) struct TextLayoutCache {
    revision: u64,
    pub(super) width: Pixels,
    render_placeholder: bool,
    pub(super) lines: Arc<[TextLayoutLine]>,
    pub(super) visual_line_count: usize,
    pub(super) content_width: Pixels,
}

#[derive(Clone, Copy)]
pub(super) struct TextViewport {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) line_height: Pixels,
    pub(super) scroll_x: Pixels,
    pub(super) scroll_y: Pixels,
}

impl TextLayoutCache {
    pub(super) fn matches(&self, revision: u64, width: Pixels, render_placeholder: bool) -> bool {
        self.revision == revision
            && self.width == width
            && self.render_placeholder == render_placeholder
    }
}

pub(super) fn layout_visual_line_count(lines: &[TextLayoutLine]) -> usize {
    lines
        .iter()
        .map(|line| line.line.wrap_boundaries().len() + 1)
        .sum::<usize>()
        .max(1)
}

pub(super) fn content_position_for_offset(
    offset: usize,
    lines: &[TextLayoutLine],
    line_height: Pixels,
) -> Option<Point<Pixels>> {
    let mut line_top = px(0.0);
    for line in lines {
        if offset >= line.start && offset <= line.end {
            let local = offset.saturating_sub(line.start).min(line.end - line.start);
            let display_offset = line.display_offset_for_content(local);
            return line
                .line
                .position_for_index(display_offset, line_height)
                .map(|position| point(position.x, line_top + position.y));
        }
        line_top += line.line.size(line_height).height;
    }
    let line = lines.last()?;
    let display_offset = line.display_offset_for_content(line.end - line.start);
    line.line
        .position_for_index(display_offset, line_height)
        .map(|position| {
            point(
                position.x,
                line_top - line.line.size(line_height).height + position.y,
            )
        })
}

pub(super) fn index_for_content_position(
    position: Point<Pixels>,
    lines: &[TextLayoutLine],
    line_height: Pixels,
) -> usize {
    if lines.is_empty() || position.y < px(0.0) {
        return 0;
    }
    let mut line_top = px(0.0);
    for line in lines {
        let line_bottom = line_top + line.line.size(line_height).height;
        if position.y < line_bottom {
            let local_position = point(position.x, position.y - line_top);
            let local = line
                .line
                .closest_index_for_position(local_position, line_height)
                .unwrap_or_else(|index| index)
                .min(line.display_len());
            return line.start + line.content_offset_for_display(local);
        }
        line_top = line_bottom;
    }
    lines.last().map_or(0, |line| line.end)
}

/// The atom whose display glyphs cover a position in content space.
pub(super) fn atom_index_at_content_position(
    position: Point<Pixels>,
    lines: &[TextLayoutLine],
    atoms: &[TextInputAtom],
    line_height: Pixels,
) -> Option<usize> {
    let offset = index_for_content_position(position, lines, line_height);
    atoms.iter().position(|atom| {
        atom.range.start <= offset
            && offset <= atom.range.end
            && atom_covers_content_position(atom, position, lines, line_height)
    })
}

fn atom_covers_content_position(
    atom: &TextInputAtom,
    position: Point<Pixels>,
    lines: &[TextLayoutLine],
    line_height: Pixels,
) -> bool {
    let Some(start) = content_position_for_offset(atom.range.start, lines, line_height) else {
        return false;
    };
    let Some(end) = content_position_for_offset(atom.range.end, lines, line_height) else {
        return false;
    };
    if start.y != end.y {
        // The atom wrapped: accept anywhere in the band of lines it occupies.
        return position.y >= start.y && position.y < end.y + line_height;
    }
    position.y >= start.y
        && position.y < start.y + line_height
        && position.x >= start.x
        && position.x < end.x
}

pub(super) fn selection_quads(
    selected_range: &Range<usize>,
    lines: &[TextLayoutLine],
    viewport: TextViewport,
    selection: Hsla,
) -> Vec<PaintQuad> {
    if selected_range.is_empty() {
        return Vec::new();
    }
    let mut quads = Vec::new();
    let mut quad_context = SelectionQuadContext {
        viewport,
        selection,
        line_top: viewport.bounds.top() - viewport.scroll_y,
    };
    for line in lines {
        let selected_start = selected_range
            .start
            .max(line.start)
            .saturating_sub(line.start);
        let selected_end = selected_range.end.min(line.end).saturating_sub(line.start);
        if selected_start < selected_end {
            let selected_start = line.display_offset_for_content(selected_start);
            let selected_end = line.display_offset_for_content(selected_end);
            let mut visual_start = 0usize;
            for visual_index in 0..=line.line.wrap_boundaries().len() {
                let visual_end = line
                    .line
                    .wrap_boundaries()
                    .get(visual_index)
                    .map_or(line.display_len(), |boundary| {
                        line.line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index
                    });
                let start = selected_start.max(visual_start);
                let end = selected_end.min(visual_end);
                if start < end {
                    if let Some(quad) = selection_quad_for_visual_range(
                        line,
                        start..end,
                        visual_start,
                        visual_index,
                        quad_context,
                    ) {
                        quads.push(quad);
                    }
                }
                visual_start = visual_end;
            }
        }
        quad_context.line_top += line.line.size(viewport.line_height).height;
    }
    quads
}

#[derive(Clone, Copy)]
struct SelectionQuadContext {
    viewport: TextViewport,
    selection: Hsla,
    line_top: Pixels,
}

fn selection_quad_for_visual_range(
    line: &TextLayoutLine,
    range: Range<usize>,
    visual_start: usize,
    visual_index: usize,
    context: SelectionQuadContext,
) -> Option<PaintQuad> {
    let viewport = context.viewport;
    let top = context.line_top + viewport.line_height * visual_index;
    let bottom = top + viewport.line_height;
    if bottom <= viewport.bounds.top() || top >= viewport.bounds.bottom() {
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
    let left = (viewport.bounds.left() + start_x - viewport.scroll_x).max(viewport.bounds.left());
    let right = (viewport.bounds.left() + end_x - viewport.scroll_x).min(viewport.bounds.right());
    (left < right).then(|| {
        fill(
            Bounds::from_corners(
                point(left, top.max(viewport.bounds.top())),
                point(right, bottom.min(viewport.bounds.bottom())),
            ),
            context.selection,
        )
    })
}

pub(super) fn cursor_quad(
    offset: usize,
    lines: &[TextLayoutLine],
    viewport: TextViewport,
    caret: Hsla,
) -> Option<PaintQuad> {
    let position = content_position_for_offset(offset, lines, viewport.line_height)?;
    let caret_width = text_input_caret_width();
    let left = viewport.bounds.left() + position.x - viewport.scroll_x;
    let top = viewport.bounds.top() + position.y - viewport.scroll_y;
    if left < viewport.bounds.left()
        || left >= viewport.bounds.right()
        || top < viewport.bounds.top()
        || top >= viewport.bounds.bottom()
    {
        return None;
    }
    Some(fill(
        Bounds::new(
            point(left.min(viewport.bounds.right() - caret_width), top),
            size(
                caret_width,
                viewport.line_height.min(viewport.bounds.bottom() - top),
            ),
        ),
        caret,
    ))
}
