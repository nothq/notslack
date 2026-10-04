use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::atoms::{
    atom_enclosing, ceil_caret_boundary, floor_caret_boundary, next_caret_boundary,
    previous_caret_boundary,
};
use super::TextInput;

pub(super) fn is_grapheme_boundary(text: &str, offset: usize) -> bool {
    offset <= text.len()
        && (offset == text.len()
            || text
                .grapheme_indices(true)
                .any(|(boundary, _)| boundary == offset))
}

fn floor_grapheme_boundary_in(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if offset == text.len() {
        return offset;
    }
    text.grapheme_indices(true)
        .take_while(|(boundary, _)| *boundary <= offset)
        .map(|(boundary, _)| boundary)
        .last()
        .unwrap_or(0)
}

/// Return the greatest Unicode grapheme boundary at or before `offset`.
pub fn floor_grapheme_boundary(text: &str, offset: usize) -> usize {
    floor_grapheme_boundary_in(text, offset)
}

fn ceil_grapheme_boundary_in(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    if is_grapheme_boundary(text, offset) {
        return offset;
    }
    text.grapheme_indices(true)
        .find_map(|(boundary, _)| (boundary > offset).then_some(boundary))
        .unwrap_or(text.len())
}

/// Return the least Unicode grapheme boundary at or after `offset`.
pub fn ceil_grapheme_boundary(text: &str, offset: usize) -> usize {
    ceil_grapheme_boundary_in(text, offset)
}

fn utf8_offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for character in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += character.len_utf16();
        utf8_offset += character.len_utf8();
    }
    utf8_offset
}

pub(super) fn utf8_range_from_utf16(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    let start = utf8_offset_from_utf16(text, range_utf16.start);
    let end = utf8_offset_from_utf16(text, range_utf16.end);
    start.min(end)..start.max(end)
}

impl TextInput {
    pub(super) fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    pub(super) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub(super) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        utf8_range_from_utf16(self.content.as_ref(), range_utf16)
    }

    /// The greatest offset at or before `offset` the caret may rest at,
    /// treating each atom as one unit.
    pub(super) fn floor_grapheme_boundary(&self, offset: usize) -> usize {
        floor_caret_boundary(self.content.as_ref(), &self.atoms, offset)
    }

    pub(super) fn normalize_selection_after_composition(&mut self) {
        if self.selected_range.is_empty() {
            let cursor =
                ceil_caret_boundary(self.content.as_ref(), &self.atoms, self.cursor_offset());
            self.selected_range = cursor..cursor;
            self.selection_reversed = false;
            return;
        }
        let start = floor_caret_boundary(
            self.content.as_ref(),
            &self.atoms,
            self.selected_range.start,
        );
        let end = ceil_caret_boundary(self.content.as_ref(), &self.atoms, self.selected_range.end);
        self.selected_range = start..end;
    }

    pub(super) fn previous_boundary(&self, offset: usize) -> usize {
        previous_caret_boundary(self.content.as_ref(), &self.atoms, offset)
    }

    pub(super) fn next_boundary(&self, offset: usize) -> usize {
        next_caret_boundary(self.content.as_ref(), &self.atoms, offset)
    }

    pub(super) fn previous_word_boundary(&self, offset: usize) -> usize {
        let offset = self.floor_grapheme_boundary(offset);
        if let Some(atom) = atom_enclosing(&self.atoms, offset.saturating_sub(1)) {
            return atom.range.start;
        }
        let mut previous_segment_start = 0;
        for (start, segment) in self.content.split_word_bound_indices() {
            if start >= offset {
                break;
            }
            if segment.chars().all(char::is_whitespace) {
                continue;
            }
            let end = start + segment.len();
            if offset <= end {
                return start;
            }
            previous_segment_start = start;
        }
        previous_segment_start
    }

    pub(super) fn next_word_boundary(&self, offset: usize) -> usize {
        let offset = self.floor_grapheme_boundary(offset);
        if let Some(atom) = atom_enclosing(&self.atoms, offset) {
            return atom.range.end;
        }
        for (start, segment) in self.content.split_word_bound_indices() {
            let end = start + segment.len();
            if end <= offset || segment.chars().all(char::is_whitespace) {
                continue;
            }
            return end;
        }
        self.content.len()
    }

    pub(super) fn word_range_at(&self, offset: usize) -> Range<usize> {
        if self.content.is_empty() {
            return 0..0;
        }
        let offset = self.floor_grapheme_boundary(offset);
        if let Some(atom) = atom_enclosing(&self.atoms, offset) {
            return atom.range.clone();
        }
        let probe = if offset == self.content.len() {
            self.previous_boundary(self.content.len())
        } else {
            offset
        };
        self.content
            .split_word_bound_indices()
            .find_map(|(start, segment)| {
                let end = start + segment.len();
                (probe >= start && probe < end).then_some(start..end)
            })
            .unwrap_or(probe..self.next_boundary(probe))
    }

    pub(super) fn hard_line_start(&self, offset: usize) -> usize {
        let offset = self.floor_grapheme_boundary(offset);
        self.content[..offset]
            .rfind('\n')
            .map_or(0, |newline| newline + 1)
    }

    pub(super) fn hard_line_end(&self, offset: usize) -> usize {
        let offset = self.floor_grapheme_boundary(offset);
        self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |newline| offset + newline)
    }

    pub(super) fn hard_line_range_at(&self, offset: usize) -> Range<usize> {
        let start = self.hard_line_start(offset);
        let end = self.hard_line_end(offset);
        start..if end < self.content.len() {
            end + 1
        } else {
            end
        }
    }
}
