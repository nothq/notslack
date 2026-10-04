use std::{ops::Range, sync::Arc};

use gpui::{point, Bounds, Pixels, Point, SharedString};
use unicode_segmentation::UnicodeSegmentation;

use super::layout::atom_index_at_content_position;
use super::offsets::{ceil_grapheme_boundary, floor_grapheme_boundary, is_grapheme_boundary};
use super::{TextInput, TextInputAtom, TextInputGhost};

#[cfg(test)]
mod tests;

/// Validate the atom and ghost invariants against `content`.
pub(super) fn assert_atoms_fit(
    content: &str,
    atoms: &[TextInputAtom],
    ghost: Option<&TextInputGhost>,
) {
    let mut previous_end = 0;
    for atom in atoms {
        assert_atom_fits(content, atom);
        assert!(
            previous_end <= atom.range.start,
            "text input atoms must be sorted and non-overlapping"
        );
        previous_end = atom.range.end;
    }
    if let Some(ghost) = ghost {
        assert_ghost_fits(content, atoms, ghost);
    }
}

fn assert_atom_fits(content: &str, atom: &TextInputAtom) {
    assert!(
        atom.range.start < atom.range.end
            && atom.range.end <= content.len()
            && content.is_char_boundary(atom.range.start)
            && content.is_char_boundary(atom.range.end),
        "text input atom range must be non-empty, fit the content, and follow UTF-8 boundaries"
    );
    assert!(
        !content[atom.range.clone()].contains('\n'),
        "text input atom range must not span a line break"
    );
    assert!(
        !atom.display.is_empty() && !atom.display.contains('\n'),
        "text input atom display text must be a non-empty single line"
    );
    assert!(
        atom.prefix_len <= atom.display.len() && atom.display.is_char_boundary(atom.prefix_len),
        "text input atom prefix length must follow a UTF-8 boundary of its display text"
    );
}

fn assert_ghost_fits(content: &str, atoms: &[TextInputAtom], ghost: &TextInputGhost) {
    assert!(
        ghost.offset <= content.len() && content.is_char_boundary(ghost.offset),
        "text input ghost offset must fit the content and follow a UTF-8 boundary"
    );
    assert!(
        atom_enclosing(atoms, ghost.offset).is_none(),
        "text input ghost text must not sit inside an atom"
    );
    assert!(
        !ghost.text.contains('\n'),
        "text input ghost text must be a single line"
    );
}

/// The atom whose range strictly contains `offset`, if any.
pub(super) fn atom_enclosing(atoms: &[TextInputAtom], offset: usize) -> Option<&TextInputAtom> {
    atoms
        .iter()
        .find(|atom| atom.range.start < offset && offset < atom.range.end)
}

/// Whether `offset` is the start or the end of an atom.
pub(super) fn is_atom_boundary(atoms: &[TextInputAtom], offset: usize) -> bool {
    atoms
        .iter()
        .any(|atom| atom.range.start == offset || atom.range.end == offset)
}

/// Move an offset strictly inside an atom to the atom's start.
pub(super) fn snap_to_atom_start(atoms: &[TextInputAtom], offset: usize) -> usize {
    atom_enclosing(atoms, offset).map_or(offset, |atom| atom.range.start)
}

/// Move an offset strictly inside an atom to the atom's end.
pub(super) fn snap_to_atom_end(atoms: &[TextInputAtom], offset: usize) -> usize {
    atom_enclosing(atoms, offset).map_or(offset, |atom| atom.range.end)
}

/// Whether the caret may rest at `offset`: an atom edge, or a Unicode
/// grapheme boundary outside every atom.
pub(super) fn is_caret_boundary(text: &str, atoms: &[TextInputAtom], offset: usize) -> bool {
    offset <= text.len()
        && (is_atom_boundary(atoms, offset)
            || (is_grapheme_boundary(text, offset) && atom_enclosing(atoms, offset).is_none()))
}

/// The greatest caret boundary at or before `offset`.
pub(super) fn floor_caret_boundary(text: &str, atoms: &[TextInputAtom], offset: usize) -> usize {
    let offset = offset.min(text.len());
    if let Some(atom) = atom_enclosing(atoms, offset) {
        return atom.range.start;
    }
    if is_atom_boundary(atoms, offset) {
        return offset;
    }
    snap_to_atom_start(atoms, floor_grapheme_boundary(text, offset))
}

/// The least caret boundary at or after `offset`.
pub(super) fn ceil_caret_boundary(text: &str, atoms: &[TextInputAtom], offset: usize) -> usize {
    let offset = offset.min(text.len());
    if let Some(atom) = atom_enclosing(atoms, offset) {
        return atom.range.end;
    }
    if is_atom_boundary(atoms, offset) {
        return offset;
    }
    snap_to_atom_end(atoms, ceil_grapheme_boundary(text, offset))
}

/// The greatest caret boundary strictly before `offset`, or 0.
pub(super) fn previous_caret_boundary(text: &str, atoms: &[TextInputAtom], offset: usize) -> usize {
    let grapheme = text
        .grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0);
    let atom_edge = atom_edges(atoms)
        .filter(|edge| *edge < offset)
        .max()
        .unwrap_or(0);
    snap_to_atom_start(atoms, grapheme.max(atom_edge))
}

/// The least caret boundary strictly after `offset`, or the text length.
pub(super) fn next_caret_boundary(text: &str, atoms: &[TextInputAtom], offset: usize) -> usize {
    let grapheme = text
        .grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(text.len());
    let atom_edge = atom_edges(atoms)
        .filter(|edge| *edge > offset)
        .min()
        .unwrap_or(text.len());
    snap_to_atom_end(atoms, grapheme.min(atom_edge))
}

fn atom_edges(atoms: &[TextInputAtom]) -> impl Iterator<Item = usize> + '_ {
    atoms
        .iter()
        .flat_map(|atom| [atom.range.start, atom.range.end])
}

/// Where `range` lands after `edit` was replaced by `new_len` bytes: unchanged
/// before the edit, shifted after it, and dropped when the edit touched it.
pub(super) fn range_after_edit(
    range: &Range<usize>,
    edit: &Range<usize>,
    new_len: usize,
) -> Option<Range<usize>> {
    if range.end <= edit.start {
        Some(range.clone())
    } else if range.start >= edit.end {
        Some(
            shifted_after_edit(range.start, edit, new_len)
                ..shifted_after_edit(range.end, edit, new_len),
        )
    } else {
        None
    }
}

/// Where `offset` lands after `edit` was replaced by `new_len` bytes; `None`
/// when it was strictly inside the edit.
pub(super) fn offset_after_edit(
    offset: usize,
    edit: &Range<usize>,
    new_len: usize,
) -> Option<usize> {
    if offset <= edit.start {
        Some(offset)
    } else if offset >= edit.end {
        Some(shifted_after_edit(offset, edit, new_len))
    } else {
        None
    }
}

fn shifted_after_edit(offset: usize, edit: &Range<usize>, new_len: usize) -> usize {
    offset - edit.end + edit.start + new_len
}

/// The single span of `old` that differs from `new`, on char boundaries, with
/// the byte length of the text replacing it.
pub(super) fn replaced_range(old: &str, new: &str) -> (Range<usize>, usize) {
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(old_byte, new_byte)| old_byte == new_byte)
        .count();
    while !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(old_byte, new_byte)| old_byte == new_byte)
        .count();
    while !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    (prefix..old.len() - suffix, new.len() - prefix - suffix)
}

impl TextInput {
    pub fn atoms(&self) -> &[TextInputAtom] {
        &self.atoms
    }

    /// The atom under the pointer, as an index into [`Self::atoms`].
    pub fn hovered_atom(&self) -> Option<usize> {
        self.hovered_atom
    }

    /// Window-space bounds of the display text of the atom at `index`, once laid out.
    pub fn window_bounds_for_atom(&self, index: usize) -> Option<Bounds<Pixels>> {
        let atom = self.atoms.get(index)?;
        self.window_bounds_for_byte_range(atom.range.clone())
    }

    pub(super) fn is_caret_boundary(&self, offset: usize) -> bool {
        is_caret_boundary(self.content.as_ref(), &self.atoms, offset)
    }

    /// Replace `range` of the content with `new_text`, keeping the atoms and
    /// the ghost consistent with the edit.
    pub(super) fn splice_content(&mut self, range: Range<usize>, new_text: &str) {
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.retain_atoms_across_edit(&range, new_text.len());
    }

    /// Replace the whole content, keeping the atoms and the ghost consistent
    /// with the span that changed.
    pub(super) fn replace_content(&mut self, content: SharedString) {
        let (edit, new_len) = replaced_range(self.content.as_ref(), content.as_ref());
        self.content = content;
        self.retain_atoms_across_edit(&edit, new_len);
    }

    fn retain_atoms_across_edit(&mut self, edit: &Range<usize>, new_len: usize) {
        if !self.atoms.is_empty() {
            let atoms = self
                .atoms
                .iter()
                .filter_map(|atom| {
                    range_after_edit(&atom.range, edit, new_len).map(|range| TextInputAtom {
                        range,
                        ..atom.clone()
                    })
                })
                .collect::<Arc<[TextInputAtom]>>();
            if *atoms != *self.atoms {
                self.hovered_atom = None;
                self.atoms = atoms;
            }
        }
        self.ghost = self.ghost.take().and_then(|ghost| {
            offset_after_edit(ghost.offset, edit, new_len)
                .map(|offset| TextInputGhost { offset, ..ghost })
        });
    }

    /// Move the selection edges out of any atom they fall inside. Returns
    /// whether the selection changed.
    pub(super) fn snap_selection_to_atoms(&mut self) -> bool {
        let start = snap_to_atom_start(&self.atoms, self.selected_range.start);
        let range = if self.selected_range.is_empty() {
            start..start
        } else {
            start..snap_to_atom_end(&self.atoms, self.selected_range.end)
        };
        if range == self.selected_range {
            return false;
        }
        self.selected_range = range;
        true
    }

    /// The atom whose display glyphs lie strictly under a window position.
    pub(super) fn atom_at_window_position(&self, position: Point<Pixels>) -> Option<usize> {
        let bounds = self.last_bounds?;
        if self.atoms.is_empty() || !bounds.contains(&position) {
            return None;
        }
        atom_index_at_content_position(
            point(
                position.x - bounds.left() + self.scroll_x,
                position.y - bounds.top() + self.scroll_y,
            ),
            self.layout_lines(),
            &self.atoms,
            self.style.line_height,
        )
    }

    /// Track the atom under the pointer (`None` once the pointer left the
    /// input). Returns whether the hovered atom changed.
    pub(super) fn sync_hovered_atom(&mut self, position: Option<Point<Pixels>>) -> bool {
        let hovered = position
            .filter(|_| !self.is_selecting && !self.disabled)
            .and_then(|position| self.atom_at_window_position(position));
        if hovered == self.hovered_atom {
            return false;
        }
        let underline_changes = [self.hovered_atom, hovered]
            .into_iter()
            .flatten()
            .any(|index| self.atoms[index].hover_underline);
        self.hovered_atom = hovered;
        if underline_changes {
            self.invalidate_layout();
        }
        true
    }
}
