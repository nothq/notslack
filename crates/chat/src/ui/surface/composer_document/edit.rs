use std::ops::Range;

use super::lines::{affected_line_indices, plain_line_blocks, remap_line_blocks};
use super::normalize::{normalize_entity_runs, normalize_link_runs, normalize_runs};
use super::remap::{contiguous_text_edit, remap_entity_runs, remap_link_runs, shift_after_edit};
use super::{
    ContiguousTextEdit, SlackComposerCursorStyle, SlackComposerDocument, SlackComposerFormatAction,
    SlackComposerInlineFormat, SlackComposerInlineStyle, SlackComposerLineBlock,
    SlackComposerLinkEdit, SlackComposerLinkRun, SlackComposerLinkUrl, SlackComposerStyleRun,
};

impl SlackComposerDocument {
    pub(crate) fn reset_to_text_if_changed(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        self.runs.clear();
        self.links.clear();
        self.entities.clear();
        self.line_blocks = plain_line_blocks(text);
        self.cursor_style = None;
        self.highlights_dirty = true;
        self.advance_revision();
    }

    pub(crate) fn clear(&mut self) {
        let changed = !self.text.is_empty()
            || !self.runs.is_empty()
            || !self.links.is_empty()
            || !self.entities.is_empty()
            || self
                .line_blocks
                .iter()
                .any(|block| *block != SlackComposerLineBlock::Section)
            || self.cursor_style.is_some();
        self.text.clear();
        self.runs.clear();
        self.links.clear();
        self.entities.clear();
        self.line_blocks.clear();
        self.line_blocks.push(SlackComposerLineBlock::Section);
        self.cursor_style = None;
        self.highlights_dirty = true;
        if changed {
            self.advance_revision();
        }
    }

    pub(crate) fn apply_text_edit(&mut self, text: &str) {
        let Some(edit) = contiguous_text_edit(&self.text, text) else {
            return;
        };
        let replacement_style = (edit.replacement_len > 0).then(|| self.replacement_style(&edit));
        let new_end = edit.old_range.start + edit.replacement_len;
        let updated_line_blocks = remap_line_blocks(&self.text, text, &self.line_blocks, &edit);
        let updated_links = remap_link_runs(&self.links, &edit);
        let updated_entities = remap_entity_runs(&self.entities, &edit);
        let updated_runs = self.remapped_style_runs(&edit, new_end, replacement_style);
        let cursor_style = self.cursor_style_after_edit(&edit, new_end, replacement_style);

        self.text.clear();
        self.text.push_str(text);
        self.runs = normalize_runs(updated_runs, &self.text);
        self.links = normalize_link_runs(updated_links, &self.text);
        self.entities = normalize_entity_runs(updated_entities, &self.text);
        self.line_blocks = updated_line_blocks;
        self.cursor_style = cursor_style;
        self.highlights_dirty = true;
        self.advance_revision();
    }

    fn remapped_style_runs(
        &self,
        edit: &ContiguousTextEdit,
        new_end: usize,
        replacement_style: Option<SlackComposerInlineStyle>,
    ) -> Vec<SlackComposerStyleRun> {
        let mut updated = Vec::with_capacity(self.runs.len() + 2);
        for run in &self.runs {
            if run.range.end <= edit.old_range.start {
                updated.push(run.clone());
                continue;
            }
            if run.range.start >= edit.old_range.end {
                updated.push(SlackComposerStyleRun {
                    range: shift_after_edit(run.range.start, edit.old_range.end, new_end)
                        ..shift_after_edit(run.range.end, edit.old_range.end, new_end),
                    style: run.style,
                });
                continue;
            }
            if run.range.start < edit.old_range.start {
                updated.push(SlackComposerStyleRun {
                    range: run.range.start..edit.old_range.start,
                    style: run.style,
                });
            }
            if run.range.end > edit.old_range.end {
                updated.push(SlackComposerStyleRun {
                    range: new_end..shift_after_edit(run.range.end, edit.old_range.end, new_end),
                    style: run.style,
                });
            }
        }
        if let Some(style) = replacement_style.filter(|style| !style.is_empty()) {
            updated.push(SlackComposerStyleRun {
                range: edit.old_range.start..new_end,
                style,
            });
        }
        updated
    }

    fn cursor_style_after_edit(
        &self,
        edit: &ContiguousTextEdit,
        new_end: usize,
        replacement_style: Option<SlackComposerInlineStyle>,
    ) -> Option<SlackComposerCursorStyle> {
        if let Some(style) = replacement_style {
            Some(SlackComposerCursorStyle {
                offset: new_end,
                style,
            })
        } else {
            self.cursor_style.and_then(|cursor| {
                (edit.old_range.start <= cursor.offset && cursor.offset <= edit.old_range.end)
                    .then_some(SlackComposerCursorStyle {
                        offset: edit.old_range.start,
                        style: cursor.style,
                    })
            })
        }
    }

    pub(crate) fn link_edit(&self, selection: Range<usize>) -> Option<SlackComposerLinkEdit> {
        self.assert_valid_range(&selection);
        if selection.is_empty() {
            return None;
        }
        let existing = self
            .links
            .iter()
            .find(|link| link.range.start <= selection.start && selection.end <= link.range.end);
        let range = existing.map_or(selection, |link| link.range.clone());
        Some(SlackComposerLinkEdit {
            text: self.text[range.clone()].to_string(),
            range,
            url: existing.map(|link| link.url.clone()),
        })
    }

    pub(crate) fn replace_range_with_link(
        &mut self,
        range: Range<usize>,
        text: &str,
        url: SlackComposerLinkUrl,
    ) {
        self.assert_valid_range(&range);
        assert!(
            !range.is_empty() && !text.is_empty(),
            "Slack composer links require selected text"
        );
        let mut updated_text = String::with_capacity(
            self.text
                .len()
                .saturating_sub(range.len())
                .saturating_add(text.len()),
        );
        updated_text.push_str(&self.text[..range.start]);
        updated_text.push_str(text);
        updated_text.push_str(&self.text[range.end..]);
        let linked_range = range.start..range.start + text.len();
        self.apply_text_edit(&updated_text);
        self.set_link(linked_range, url);
    }

    fn set_link(&mut self, range: Range<usize>, url: SlackComposerLinkUrl) {
        self.assert_valid_range(&range);
        assert!(
            !range.is_empty(),
            "Slack composer link range must not be empty"
        );
        let mut updated = Vec::with_capacity(self.links.len() + 2);
        for link in &self.links {
            if link.range.end <= range.start || link.range.start >= range.end {
                updated.push(link.clone());
                continue;
            }
            if link.range.start < range.start {
                updated.push(SlackComposerLinkRun {
                    range: link.range.start..range.start,
                    url: link.url.clone(),
                });
            }
            if link.range.end > range.end {
                updated.push(SlackComposerLinkRun {
                    range: range.end..link.range.end,
                    url: link.url.clone(),
                });
            }
        }
        updated.push(SlackComposerLinkRun { range, url });
        let updated = normalize_link_runs(updated, &self.text);
        if self.links == updated {
            return;
        }
        self.links = updated;
        self.highlights_dirty = true;
        self.advance_revision();
    }

    pub(crate) fn toggle_format(
        &mut self,
        action: SlackComposerFormatAction,
        selection: Range<usize>,
    ) -> bool {
        self.assert_valid_range(&selection);
        if let Some(format) = SlackComposerInlineFormat::from_action(action) {
            return self.toggle_inline_format(format, selection);
        }
        let Some(block) = SlackComposerLineBlock::from_action(action) else {
            return false;
        };
        let (first_line, last_line) = affected_line_indices(&self.text, &selection);
        let enable = !self.line_blocks[first_line..=last_line]
            .iter()
            .all(|current| *current == block);
        self.line_blocks[first_line..=last_line].fill(if enable {
            block
        } else {
            SlackComposerLineBlock::Section
        });
        self.highlights_dirty = true;
        self.advance_revision();
        true
    }

    fn toggle_inline_format(
        &mut self,
        format: SlackComposerInlineFormat,
        selection: Range<usize>,
    ) -> bool {
        if selection.is_empty() {
            let mut style = self.style_at_cursor(selection.start);
            style.set(format, !style.contains(format));
            self.cursor_style = Some(SlackComposerCursorStyle {
                offset: selection.start,
                style,
            });
            self.advance_revision();
            return true;
        }

        let enable = !self.range_has_format(&selection, format);
        let mut cut_points = Vec::with_capacity(self.runs.len() * 2 + 4);
        cut_points.extend([0, selection.start, selection.end, self.text.len()]);
        for run in &self.runs {
            cut_points.extend([run.range.start, run.range.end]);
        }
        cut_points.sort_unstable();
        cut_points.dedup();

        let mut updated_runs = Vec::with_capacity(self.runs.len() + 2);
        for boundary in cut_points.windows(2) {
            let range = boundary[0]..boundary[1];
            if range.is_empty() {
                continue;
            }
            let mut style = self.style_at(range.start);
            if range.start >= selection.start && range.end <= selection.end {
                style.set(format, enable);
            }
            if !style.is_empty() {
                updated_runs.push(SlackComposerStyleRun { range, style });
            }
        }
        self.runs = normalize_runs(updated_runs, &self.text);
        self.cursor_style = None;
        self.highlights_dirty = true;
        self.advance_revision();
        true
    }
}
