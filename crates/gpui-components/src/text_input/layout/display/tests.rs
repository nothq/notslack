use std::ops::Range;

use gpui::{rgb, SharedString};

use super::{build_display_text, DisplayOptions, DisplayText};
use crate::text_input::layout::content_offset_at_display;
use crate::text_input::{TextInputAtom, TextInputGhost};

const BULLET: &str = "\u{2023}";
const SOFT_BREAK: &str = "\u{200c}";
const ELLIPSIS: &str = "\u{2026}";

fn atom(range: Range<usize>, display: &str, prefix_len: usize) -> TextInputAtom {
    TextInputAtom {
        range,
        display: SharedString::from(display.to_owned()),
        prefix_len,
        color: rgb(0xffffff).into(),
        prefix_color: rgb(0x888888).into(),
        hover_underline: true,
    }
}

fn ghost(offset: usize, text: &str) -> TextInputGhost {
    TextInputGhost {
        offset,
        text: SharedString::from(text.to_owned()),
        color: rgb(0x666666).into(),
    }
}

fn build(
    content: &str,
    atoms: &[TextInputAtom],
    ghost: Option<&TextInputGhost>,
    soft_break_hyphens: bool,
) -> DisplayText {
    build_display_text(
        content,
        DisplayOptions {
            atoms,
            ghost,
            soft_break_hyphens,
        },
    )
}

fn boundaries(display: &DisplayText) -> Vec<(usize, usize)> {
    display
        .offsets
        .iter()
        .map(|offset| (offset.content, offset.display))
        .collect()
}

#[test]
fn atom_display_replaces_its_content_range() {
    let content = format!("a{BULLET}b");
    let display = build(&content, &[atom(1..4, "@Today", 1)], None, false);
    assert_eq!(display.text, "a@Todayb");
    assert_eq!(boundaries(&display), vec![(0, 0), (1, 1), (4, 7), (5, 8)]);
    assert_eq!(display.atoms, vec![1..7]);
    assert_eq!(display.ghost, None);
}

#[test]
fn boundaries_inside_a_multi_char_atom_map_to_its_display_start() {
    let content = format!("x{BULLET}{BULLET}y");
    let display = build(&content, &[atom(1..7, "@A", 1)], None, false);
    assert_eq!(display.text, "x@Ay");
    assert_eq!(
        boundaries(&display),
        vec![(0, 0), (1, 1), (4, 1), (7, 3), (8, 4)]
    );
    assert_eq!(content_offset_at_display(&display.offsets, 1), 1);
    assert_eq!(content_offset_at_display(&display.offsets, 2), 1);
    assert_eq!(content_offset_at_display(&display.offsets, 3), 7);
}

#[test]
fn adjacent_atoms_keep_distinct_display_ranges() {
    let content = format!("{BULLET}{BULLET}");
    let display = build(
        &content,
        &[atom(0..3, "@A", 1), atom(3..6, "@B", 1)],
        None,
        false,
    );
    assert_eq!(display.text, "@A@B");
    assert_eq!(boundaries(&display), vec![(0, 0), (3, 2), (6, 4)]);
    assert_eq!(display.atoms, vec![0..2, 2..4]);
}

#[test]
fn hyphen_soft_breaks_are_inserted_outside_atoms() {
    let content = format!("a-{BULLET}-b");
    let display = build(&content, &[atom(2..5, "@X", 1)], None, true);
    assert_eq!(display.text, format!("a-{SOFT_BREAK}@X-{SOFT_BREAK}b"));
    assert_eq!(
        boundaries(&display),
        vec![(0, 0), (1, 1), (2, 5), (5, 7), (6, 11), (7, 12)]
    );
    assert_eq!(display.atoms, vec![5..7]);
}

#[test]
fn hyphens_stay_plain_without_soft_breaks() {
    let display = build("a-b", &[], None, false);
    assert_eq!(display.text, "a-b");
    assert_eq!(boundaries(&display), vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
}

#[test]
fn ghost_follows_the_boundary_at_its_offset() {
    let display = build("ab", &[], Some(&ghost(1, ELLIPSIS)), false);
    assert_eq!(display.text, format!("a{ELLIPSIS}b"));
    assert_eq!(boundaries(&display), vec![(0, 0), (1, 1), (2, 5)]);
    assert_eq!(display.ghost, Some(1..4));
    assert_eq!(content_offset_at_display(&display.offsets, 2), 1);
    assert_eq!(content_offset_at_display(&display.offsets, 4), 1);
    assert_eq!(content_offset_at_display(&display.offsets, 5), 2);
}

#[test]
fn ghost_at_the_content_end_keeps_the_final_boundary_before_it() {
    let display = build("ab", &[], Some(&ghost(2, " tab")), false);
    assert_eq!(display.text, "ab tab");
    assert_eq!(boundaries(&display), vec![(0, 0), (1, 1), (2, 2)]);
    assert_eq!(display.ghost, Some(2..6));
    assert_eq!(content_offset_at_display(&display.offsets, 6), 2);
}

#[test]
fn ghost_at_the_content_start_precedes_the_first_char() {
    let display = build("ab", &[], Some(&ghost(0, ELLIPSIS)), false);
    assert_eq!(display.text, format!("{ELLIPSIS}ab"));
    assert_eq!(boundaries(&display), vec![(0, 0), (1, 4), (2, 5)]);
    assert_eq!(display.ghost, Some(0..3));
}

#[test]
fn ghost_after_an_atom_follows_the_atom_display() {
    let display = build(
        BULLET,
        &[atom(0..3, "@Today", 1)],
        Some(&ghost(3, " tab")),
        false,
    );
    assert_eq!(display.text, "@Today tab");
    assert_eq!(boundaries(&display), vec![(0, 0), (3, 6)]);
    assert_eq!(display.atoms, vec![0..6]);
    assert_eq!(display.ghost, Some(6..10));
}
