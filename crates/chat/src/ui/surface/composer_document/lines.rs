use std::ops::Range;

use super::{ContiguousTextEdit, SlackComposerLineBlock};

pub(super) fn plain_line_blocks(text: &str) -> Vec<SlackComposerLineBlock> {
    vec![SlackComposerLineBlock::Section; logical_line_count(text)]
}

pub(super) fn logical_line_count(text: &str) -> usize {
    text.as_bytes()
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        .checked_add(1)
        .expect("Slack composer line count overflowed")
}

pub(super) fn logical_line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(logical_line_count(text));
    let mut line_start = 0;
    for (offset, byte) in text.as_bytes().iter().enumerate() {
        if *byte == b'\n' {
            ranges.push(line_start..offset);
            line_start = offset + 1;
        }
    }
    ranges.push(line_start..text.len());
    ranges
}

pub(super) fn line_index_at_cursor(text: &str, offset: usize) -> usize {
    assert!(
        offset <= text.len() && text.is_char_boundary(offset),
        "Slack composer line offset must follow UTF-8 boundaries"
    );
    text.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

pub(super) fn affected_line_indices(text: &str, selection: &Range<usize>) -> (usize, usize) {
    let first_line = line_index_at_cursor(text, selection.start);
    let last_line = if selection.is_empty() {
        first_line
    } else {
        let last_character_start = text[..selection.end]
            .char_indices()
            .next_back()
            .map(|(offset, _)| offset)
            .expect("non-empty Slack composer selection omitted its final character");
        line_index_at_cursor(text, last_character_start)
    };
    (first_line, last_line)
}

pub(super) fn remap_line_blocks(
    old_text: &str,
    new_text: &str,
    old_blocks: &[SlackComposerLineBlock],
    edit: &ContiguousTextEdit,
) -> Vec<SlackComposerLineBlock> {
    assert_eq!(
        old_blocks.len(),
        logical_line_count(old_text),
        "Slack composer line blocks must match the document line count"
    );
    let replacement_block = old_blocks[line_index_at_cursor(old_text, edit.old_range.start)];
    let new_end = edit.old_range.start + edit.replacement_len;
    logical_line_ranges(new_text)
        .into_iter()
        .map(|line| {
            if line.start < edit.old_range.start {
                old_blocks[line_index_at_cursor(old_text, line.start)]
            } else if line.start < new_end {
                replacement_block
            } else {
                let old_offset = edit.old_range.end + (line.start - new_end);
                old_blocks[line_index_at_cursor(old_text, old_offset)]
            }
        })
        .collect()
}
