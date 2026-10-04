use crate::text_input::{ceil_grapheme_boundary, floor_grapheme_boundary};
use gpui::{Pixels, Point, TextLayout};
use std::ops::Range;

pub(super) fn text_index_for_position(
    text: &str,
    text_layout: &TextLayout,
    position: Point<Pixels>,
) -> usize {
    let index = text_layout
        .index_for_position(position)
        .unwrap_or_else(|index| index);
    floor_grapheme_boundary(text, index)
}

pub(super) fn selection_index_for_position(
    text: &str,
    text_layout: &TextLayout,
    position: Point<Pixels>,
) -> usize {
    let Ok(index) = text_layout.index_for_position(position) else {
        return text_index_for_position(text, text_layout, position);
    };
    let start = floor_grapheme_boundary(text, index);
    if start == text.len() {
        return start;
    }
    let end = ceil_grapheme_boundary(text, start + 1);
    let Some(start_position) = text_layout.position_for_index(start) else {
        return start;
    };
    let Some(end_position) = text_layout.position_for_index(end) else {
        return start;
    };
    if point_distance_squared(position, end_position)
        < point_distance_squared(position, start_position)
    {
        end
    } else {
        start
    }
}

fn point_distance_squared(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let x = (a.x - b.x).as_f32();
    let y = (a.y - b.y).as_f32();
    x * x + y * y
}

pub(super) fn sorted_range(anchor: usize, index: usize) -> Range<usize> {
    anchor.min(index)..anchor.max(index)
}

pub(super) fn link_range_index_at_index(
    link_ranges: &[Range<usize>],
    index: usize,
) -> Option<usize> {
    link_ranges.iter().position(|range| range.contains(&index))
}
