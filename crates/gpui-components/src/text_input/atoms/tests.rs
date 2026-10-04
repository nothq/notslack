use std::ops::Range;

use gpui::{rgb, SharedString};

use super::{
    ceil_caret_boundary, floor_caret_boundary, is_caret_boundary, next_caret_boundary,
    offset_after_edit, previous_caret_boundary, range_after_edit, replaced_range,
};
use crate::text_input::offsets::is_grapheme_boundary;
use crate::text_input::TextInputAtom;

const BULLET: &str = "\u{2023}";

fn atom(range: Range<usize>) -> TextInputAtom {
    TextInputAtom {
        range,
        display: SharedString::from("@A"),
        prefix_len: 1,
        color: rgb(0xffffff).into(),
        prefix_color: rgb(0x888888).into(),
        hover_underline: true,
    }
}

/// "ab" (0..2), two bullets (2..8), "cd" (8..10) with one atom over both bullets.
fn two_bullet_text() -> (String, Vec<TextInputAtom>) {
    (format!("ab{BULLET}{BULLET}cd"), vec![atom(2..8)])
}

#[test]
fn floor_snaps_offsets_inside_an_atom_to_its_start() {
    let (text, atoms) = two_bullet_text();
    assert_eq!(floor_caret_boundary(&text, &atoms, 5), 2);
    assert_eq!(floor_caret_boundary(&text, &atoms, 2), 2);
    assert_eq!(floor_caret_boundary(&text, &atoms, 8), 8);
    assert_eq!(floor_caret_boundary(&text, &atoms, 9), 9);
    assert_eq!(floor_caret_boundary(&text, &atoms, 42), 10);
}

#[test]
fn ceil_snaps_offsets_inside_an_atom_to_its_end() {
    let (text, atoms) = two_bullet_text();
    assert_eq!(ceil_caret_boundary(&text, &atoms, 5), 8);
    assert_eq!(ceil_caret_boundary(&text, &atoms, 2), 2);
    assert_eq!(ceil_caret_boundary(&text, &atoms, 8), 8);
    assert_eq!(ceil_caret_boundary(&text, &atoms, 1), 1);
}

#[test]
fn previous_and_next_boundaries_step_over_a_whole_atom() {
    let (text, atoms) = two_bullet_text();
    assert_eq!(previous_caret_boundary(&text, &atoms, 8), 2);
    assert_eq!(previous_caret_boundary(&text, &atoms, 9), 8);
    assert_eq!(previous_caret_boundary(&text, &atoms, 2), 1);
    assert_eq!(previous_caret_boundary(&text, &atoms, 0), 0);
    assert_eq!(next_caret_boundary(&text, &atoms, 2), 8);
    assert_eq!(next_caret_boundary(&text, &atoms, 1), 2);
    assert_eq!(next_caret_boundary(&text, &atoms, 8), 9);
    assert_eq!(next_caret_boundary(&text, &atoms, 10), 10);
}

#[test]
fn caret_boundaries_exclude_atom_interiors() {
    let (text, atoms) = two_bullet_text();
    assert!(is_caret_boundary(&text, &atoms, 1));
    assert!(is_caret_boundary(&text, &atoms, 2));
    assert!(is_caret_boundary(&text, &atoms, 8));
    assert!(is_caret_boundary(&text, &atoms, 10));
    assert!(!is_caret_boundary(&text, &atoms, 5));
    assert!(!is_caret_boundary(&text, &atoms, 11));
}

#[test]
fn atom_edges_are_caret_boundaries_even_inside_a_grapheme() {
    // A combining acute accent joins the bullet into one grapheme cluster.
    let text = format!("{BULLET}\u{0301}x");
    let atoms = vec![atom(0..3)];
    assert!(!is_grapheme_boundary(&text, 3));
    assert!(is_caret_boundary(&text, &atoms, 3));
    assert_eq!(floor_caret_boundary(&text, &atoms, 3), 3);
    assert_eq!(previous_caret_boundary(&text, &atoms, 5), 3);
    assert_eq!(next_caret_boundary(&text, &atoms, 0), 3);
    assert_eq!(next_caret_boundary(&text, &atoms, 3), 5);
}

#[test]
fn ranges_before_an_edit_stay_and_ranges_after_it_shift() {
    assert_eq!(range_after_edit(&(0..3), &(5..7), 1), Some(0..3));
    assert_eq!(range_after_edit(&(8..11), &(5..7), 1), Some(7..10));
    assert_eq!(range_after_edit(&(8..11), &(5..5), 4), Some(12..15));
}

#[test]
fn ranges_touched_by_an_edit_are_dropped() {
    assert_eq!(range_after_edit(&(4..7), &(5..6), 0), None);
    assert_eq!(range_after_edit(&(4..7), &(6..9), 0), None);
    assert_eq!(range_after_edit(&(4..7), &(2..5), 1), None);
    assert_eq!(range_after_edit(&(4..7), &(5..5), 1), None);
}

#[test]
fn insertions_at_an_atom_edge_keep_the_atom_whole() {
    assert_eq!(range_after_edit(&(4..7), &(4..4), 2), Some(6..9));
    assert_eq!(range_after_edit(&(4..7), &(7..7), 2), Some(4..7));
}

#[test]
fn offsets_follow_edits() {
    assert_eq!(offset_after_edit(3, &(5..7), 1), Some(3));
    assert_eq!(offset_after_edit(5, &(5..7), 1), Some(5));
    assert_eq!(offset_after_edit(7, &(5..7), 1), Some(6));
    assert_eq!(offset_after_edit(6, &(5..7), 1), None);
}

#[test]
fn replaced_range_finds_the_differing_span_on_char_boundaries() {
    assert_eq!(
        replaced_range("hello world", "hello brave world"),
        (6..6, 6)
    );
    assert_eq!(replaced_range("aXb", "ab"), (1..2, 0));
    assert_eq!(replaced_range("same", "same"), (4..4, 0));
    assert_eq!(replaced_range("ab", "abc"), (2..2, 1));
    assert_eq!(replaced_range("abc", ""), (0..3, 0));
    assert_eq!(replaced_range("a\u{e9}b", "a\u{e8}b"), (1..3, 2));
}
