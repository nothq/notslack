use std::ops::Range;

use gpui::{rgb, Hsla};

use super::clip_highlights;
use crate::text_input::TextInputHighlight;

fn highlight(range: Range<usize>) -> TextInputHighlight {
    TextInputHighlight {
        range,
        color: Hsla::from(rgb(0xff0000)),
        monospace: true,
        ..Default::default()
    }
}

fn ranges(highlights: &[TextInputHighlight]) -> Vec<Range<usize>> {
    highlights
        .iter()
        .map(|highlight| highlight.range.clone())
        .collect()
}

#[test]
fn highlight_spanning_a_reserved_range_is_split_around_it() {
    let clipped = clip_highlights(&[highlight(0..10)], &[3..5]);
    assert_eq!(ranges(&clipped), vec![0..3, 5..10]);
    assert!(clipped
        .iter()
        .all(|piece| piece.monospace && piece.color == Hsla::from(rgb(0xff0000))));
}

#[test]
fn highlight_inside_a_reserved_range_is_dropped() {
    assert!(clip_highlights(&[highlight(3..5)], &[2..6]).is_empty());
    assert!(clip_highlights(&[highlight(2..6)], &[2..6]).is_empty());
}

#[test]
fn highlight_edges_overlapping_reserved_ranges_are_trimmed() {
    let clipped = clip_highlights(&[highlight(2..8)], &[0..3, 7..9]);
    assert_eq!(ranges(&clipped), vec![3..7]);
}

#[test]
fn highlights_outside_reserved_ranges_are_kept() {
    let clipped = clip_highlights(&[highlight(0..2), highlight(6..8)], &[3..5]);
    assert_eq!(ranges(&clipped), vec![0..2, 6..8]);
    let touching = clip_highlights(&[highlight(0..3), highlight(5..8)], &[3..5]);
    assert_eq!(ranges(&touching), vec![0..3, 5..8]);
}

#[test]
fn several_reserved_ranges_split_one_highlight_in_order() {
    let clipped = clip_highlights(&[highlight(0..10)], &[2..3, 5..6, 8..9]);
    assert_eq!(ranges(&clipped), vec![0..2, 3..5, 6..8, 9..10]);
}
