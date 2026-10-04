use std::{ops::Range, sync::Arc};

use crate::text_input::{TextDisplayOffset, TextInputAtom, TextInputGhost};

#[cfg(test)]
mod tests;

/// U+200C is an invisible format character that GPUI keeps in the shaped
/// glyph stream. It gives the wrapper a zero-width boundary after a hyphen
/// without changing the canonical text.
const SOFT_BREAK: char = '\u{200c}';

/// Display-only transformations applied while building the shaped text.
#[derive(Clone, Copy)]
pub(super) struct DisplayOptions<'a> {
    /// Sorted, non-overlapping atoms validated against the content.
    pub(super) atoms: &'a [TextInputAtom],
    pub(super) ghost: Option<&'a TextInputGhost>,
    /// Insert a soft wrap opportunity after every hyphen outside atoms.
    pub(super) soft_break_hyphens: bool,
}

/// Text handed to the shaper together with the content-to-display mapping.
pub(super) struct DisplayText {
    pub(super) text: String,
    /// One entry per content char boundary, sorted by content offset with
    /// non-decreasing display offsets; boundaries strictly inside an atom map
    /// to the atom's display start.
    pub(super) offsets: Arc<[TextDisplayOffset]>,
    /// Display byte range of each atom, in atom order.
    pub(super) atoms: Vec<Range<usize>>,
    /// Display byte range of the ghost text.
    pub(super) ghost: Option<Range<usize>>,
}

/// Walk `content` once, replacing atoms with their display text, inserting
/// hyphen soft breaks, and placing the ghost text right after the content
/// boundary at its offset.
pub(super) fn build_display_text(content: &str, options: DisplayOptions<'_>) -> DisplayText {
    let mut builder = DisplayTextBuilder {
        content,
        options,
        text: String::with_capacity(content.len()),
        offsets: vec![TextDisplayOffset {
            content: 0,
            display: 0,
        }],
        atoms: Vec::with_capacity(options.atoms.len()),
        ghost: None,
        next_atom: 0,
    };
    builder.push_ghost_at(0);
    let mut offset = 0;
    while offset < content.len() {
        offset = builder.push_at(offset);
    }
    builder.finish()
}

struct DisplayTextBuilder<'a> {
    content: &'a str,
    options: DisplayOptions<'a>,
    text: String,
    offsets: Vec<TextDisplayOffset>,
    atoms: Vec<Range<usize>>,
    ghost: Option<Range<usize>>,
    next_atom: usize,
}

impl DisplayTextBuilder<'_> {
    /// Append whatever starts at the content char boundary `offset` and
    /// return the content offset that follows it.
    fn push_at(&mut self, offset: usize) -> usize {
        let atoms = self.options.atoms;
        match atoms
            .get(self.next_atom)
            .filter(|atom| atom.range.start == offset)
        {
            Some(atom) => {
                self.next_atom += 1;
                self.push_atom(atom)
            }
            None => self.push_char(offset),
        }
    }

    fn push_atom(&mut self, atom: &TextInputAtom) -> usize {
        let display_start = self.text.len();
        self.text.push_str(&atom.display);
        self.atoms.push(display_start..self.text.len());
        for (inner, _) in self.content[atom.range.clone()].char_indices().skip(1) {
            self.offsets.push(TextDisplayOffset {
                content: atom.range.start + inner,
                display: display_start,
            });
        }
        self.push_boundary(atom.range.end);
        atom.range.end
    }

    fn push_char(&mut self, offset: usize) -> usize {
        let character = self.content[offset..]
            .chars()
            .next()
            .expect("display text builder must stop at the end of the content");
        self.text.push(character);
        if self.options.soft_break_hyphens && character == '-' {
            self.text.push(SOFT_BREAK);
        }
        let end = offset + character.len_utf8();
        self.push_boundary(end);
        end
    }

    fn push_boundary(&mut self, content: usize) {
        self.offsets.push(TextDisplayOffset {
            content,
            display: self.text.len(),
        });
        self.push_ghost_at(content);
    }

    fn push_ghost_at(&mut self, content: usize) {
        let Some(ghost) = self.options.ghost.filter(|ghost| ghost.offset == content) else {
            return;
        };
        let display_start = self.text.len();
        self.text.push_str(&ghost.text);
        self.ghost = Some(display_start..self.text.len());
    }

    fn finish(self) -> DisplayText {
        assert!(
            self.next_atom == self.options.atoms.len(),
            "text input atoms must start on char boundaries of the content, in order"
        );
        assert!(
            self.options.ghost.is_none() || self.ghost.is_some(),
            "text input ghost offset must be a char boundary of the content outside every atom"
        );
        DisplayText {
            text: self.text,
            offsets: self.offsets.into(),
            atoms: self.atoms,
            ghost: self.ghost,
        }
    }
}
