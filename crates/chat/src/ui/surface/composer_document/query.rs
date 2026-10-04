use std::ops::Range;

use super::lines::{affected_line_indices, line_index_at_cursor};
use super::{
    ContiguousTextEdit, SlackComposerDocument, SlackComposerEntityRun, SlackComposerFormatAction,
    SlackComposerInlineFormat, SlackComposerInlineStyle, SlackComposerLineBlock,
    SlackComposerLinkRun,
};

impl SlackComposerDocument {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn has_rendered_highlights(&self) -> bool {
        !self.runs.is_empty()
            || !self.links.is_empty()
            || !self.entities.is_empty()
            || self
                .line_blocks
                .contains(&SlackComposerLineBlock::Preformatted)
    }

    pub(crate) fn has_message_content(&self) -> bool {
        !self.text.trim().is_empty()
    }

    pub(crate) fn same_draft_state(&self, other: &Self) -> bool {
        self.text == other.text
            && self.runs == other.runs
            && self.links == other.links
            && self.entities == other.entities
            && self.line_blocks == other.line_blocks
            && self.cursor_style == other.cursor_style
    }

    pub(crate) fn supports(action: SlackComposerFormatAction) -> bool {
        action == SlackComposerFormatAction::Link
            || SlackComposerInlineFormat::from_action(action).is_some()
            || SlackComposerLineBlock::from_action(action).is_some()
    }

    pub(crate) fn format_is_active(
        &self,
        action: SlackComposerFormatAction,
        selection: &Range<usize>,
    ) -> bool {
        self.assert_valid_range(selection);
        if action == SlackComposerFormatAction::Link {
            return !selection.is_empty()
                && self.links.iter().any(|link| {
                    link.range.start <= selection.start && selection.end <= link.range.end
                });
        }
        if let Some(format) = SlackComposerInlineFormat::from_action(action) {
            return if selection.is_empty() {
                self.style_at_cursor(selection.start).contains(format)
            } else {
                self.range_has_format(selection, format)
            };
        }
        let Some(block) = SlackComposerLineBlock::from_action(action) else {
            return false;
        };
        let (first_line, last_line) = affected_line_indices(&self.text, selection);
        self.line_blocks[first_line..=last_line]
            .iter()
            .all(|current| *current == block)
    }

    pub(super) fn replacement_style(&self, edit: &ContiguousTextEdit) -> SlackComposerInlineStyle {
        if let Some(cursor) = self.cursor_style {
            if cursor.offset == edit.old_range.start || cursor.offset == edit.old_range.end {
                return cursor.style;
            }
        }
        if edit.old_range.is_empty() {
            self.style_at_cursor(edit.old_range.start)
        } else {
            self.style_at(edit.old_range.start)
        }
    }

    pub(super) fn style_at_cursor(&self, offset: usize) -> SlackComposerInlineStyle {
        if let Some(cursor) = self.cursor_style {
            if cursor.offset == offset {
                return cursor.style;
            }
        }
        if offset == 0 {
            self.style_at(0)
        } else {
            self.style_at(offset - 1)
        }
    }

    pub(super) fn style_at(&self, offset: usize) -> SlackComposerInlineStyle {
        self.runs
            .iter()
            .find(|run| run.range.start <= offset && offset < run.range.end)
            .map_or_else(SlackComposerInlineStyle::default, |run| run.style)
    }

    pub(super) fn link_at(&self, offset: usize) -> Option<&SlackComposerLinkRun> {
        self.links
            .iter()
            .find(|link| link.range.start <= offset && offset < link.range.end)
    }

    pub(super) fn entity_at(&self, offset: usize) -> Option<&SlackComposerEntityRun> {
        let insertion_index = self
            .entities
            .partition_point(|entity| entity.range.start <= offset);
        insertion_index
            .checked_sub(1)
            .and_then(|index| self.entities.get(index))
            .filter(|entity| offset < entity.range.end)
    }

    pub(super) fn offset_is_inside_entity(&self, offset: usize) -> bool {
        self.entity_at(offset)
            .is_some_and(|entity| entity.range.start < offset)
    }

    pub(super) fn line_block_at_offset(&self, offset: usize) -> SlackComposerLineBlock {
        self.line_blocks[line_index_at_cursor(&self.text, offset)]
    }

    pub(super) fn range_has_format(
        &self,
        range: &Range<usize>,
        format: SlackComposerInlineFormat,
    ) -> bool {
        let mut covered_until = range.start;
        for run in self
            .runs
            .iter()
            .filter(|run| run.range.start < range.end && run.range.end > range.start)
        {
            if run.range.start > covered_until || !run.style.contains(format) {
                return false;
            }
            covered_until = covered_until.max(run.range.end);
            if covered_until >= range.end {
                return true;
            }
        }
        false
    }

    pub(super) fn assert_valid_range(&self, range: &Range<usize>) {
        assert!(
            range.start <= range.end
                && range.end <= self.text.len()
                && self.text.is_char_boundary(range.start)
                && self.text.is_char_boundary(range.end),
            "Slack composer selection must follow UTF-8 boundaries"
        );
    }

    pub(super) fn advance_revision(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Slack composer document revision overflowed");
    }
}
