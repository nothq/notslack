use std::ops::Range;

use gpui::{point, Bounds, Context, Modifiers, Pixels, Point, Window};

use super::layout::index_for_content_position;
use super::offsets::is_grapheme_boundary;
use super::{TextInput, TextInputSnapshot, TextInputUndoSnapshot, TextInputVisualLine};

impl TextInput {
    pub fn last_window_bounds(&self) -> Option<Bounds<Pixels>> {
        self.last_bounds
    }

    /// Return the window-space bounds occupied by a UTF-8 text range.
    ///
    /// A range spanning multiple visual lines uses the full input width after
    /// its first line, matching the rectangle exposed to the platform input
    /// method for the same range.
    pub fn window_bounds_for_byte_range(&self, range: Range<usize>) -> Option<Bounds<Pixels>> {
        assert!(
            range.start <= range.end
                && range.end <= self.content.len()
                && is_grapheme_boundary(self.content.as_ref(), range.start)
                && is_grapheme_boundary(self.content.as_ref(), range.end),
            "text input bounds range must fit the content and follow Unicode grapheme boundaries"
        );
        let bounds = self.last_bounds?;
        let start = super::layout::content_position_for_offset(
            range.start,
            self.layout_lines(),
            self.style.line_height,
        )?;
        let end = super::layout::content_position_for_offset(
            range.end,
            self.layout_lines(),
            self.style.line_height,
        )
        .unwrap_or(start);
        let line_top = bounds.top() + start.y - self.scroll_y;
        let top = line_top.clamp(bounds.top(), bounds.bottom());
        let bottom = (line_top + self.style.line_height).clamp(top, bounds.bottom());
        let start_x =
            (bounds.left() + start.x - self.scroll_x).clamp(bounds.left(), bounds.right());
        let end_x = if end.y == start.y {
            (bounds.left() + end.x - self.scroll_x).clamp(start_x, bounds.right())
        } else {
            bounds.right()
        };
        Some(Bounds::from_corners(
            point(start_x, top),
            point(end_x, bottom),
        ))
    }

    /// Resolve a window-relative position to a UTF-8 byte offset.
    pub fn byte_offset_for_window_position(&self, position: Point<Pixels>) -> Option<usize> {
        self.last_bounds?;
        Some(self.index_for_mouse_position(position))
    }

    /// Resolve a window-space x to the closest grapheme offset on an outer visual line.
    pub fn outer_visual_line_offset_for_window_x(
        &self,
        line: TextInputVisualLine,
        window_x: Pixels,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let line_index = match line {
            TextInputVisualLine::First => 0,
            TextInputVisualLine::Last => self.measured_line_count().saturating_sub(1),
        };
        let position = point(
            window_x - bounds.left() + self.scroll_x,
            self.style.line_height * line_index as f32,
        );
        let offset =
            index_for_content_position(position, self.layout_lines(), self.style.line_height);
        Some(self.floor_grapheme_boundary(offset))
    }

    /// Retain an absolute x after an external vertical-navigation selection update.
    pub fn set_vertical_navigation_window_x(&mut self, window_x: Pixels) {
        let bounds = self
            .last_bounds
            .expect("vertical navigation requires laid-out text input bounds");
        self.desired_x = Some(window_x - bounds.left() + self.scroll_x);
    }

    pub fn set_selection(&mut self, range: Range<usize>, reversed: bool, cx: &mut Context<Self>) {
        assert!(
            range.start <= range.end
                && range.end <= self.content.len()
                && self.is_caret_boundary(range.start)
                && self.is_caret_boundary(range.end),
            "text input selection must fit the content and rest on caret boundaries"
        );
        let reversed = reversed && !range.is_empty();
        if self.selected_range == range && self.selection_reversed == reversed {
            return;
        }
        self.selected_range = range;
        self.selection_reversed = reversed;
        self.desired_x = None;
        self.ensure_cursor_visible();
        self.reset_caret_blink();
        cx.notify();
    }

    pub(super) fn undo(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(on_undo_with_state) = self.callbacks.on_undo_with_state.clone() {
            on_undo_with_state(self.snapshot(), modifiers, window, cx);
            return true;
        }
        let Some(snapshot) = self.undo_stack.pop() else {
            return true;
        };
        self.redo_stack.push(self.undo_snapshot());
        self.restore_undo_snapshot(snapshot);
        self.emit_change(window, cx);
        cx.notify();
        true
    }

    pub(super) fn redo(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(on_redo_with_state) = self.callbacks.on_redo_with_state.clone() {
            on_redo_with_state(self.snapshot(), modifiers, window, cx);
            return true;
        }
        let Some(snapshot) = self.redo_stack.pop() else {
            return true;
        };
        self.undo_stack.push(self.undo_snapshot());
        self.restore_undo_snapshot(snapshot);
        self.emit_change(window, cx);
        cx.notify();
        true
    }

    pub(super) fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.floor_grapheme_boundary(offset);
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.ensure_cursor_visible();
        self.reset_caret_blink();
        cx.notify()
    }

    pub(super) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub(super) fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        index_for_content_position(
            point(
                position.x - bounds.left() + self.scroll_x,
                position.y - bounds.top() + self.scroll_y,
            ),
            self.layout_lines(),
            self.style.line_height,
        )
    }

    pub(super) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.floor_grapheme_boundary(offset);
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.ensure_cursor_visible();
        self.reset_caret_blink();
        cx.notify()
    }

    pub(super) fn emit_change(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(on_change) = self.callbacks.on_change.clone() {
            on_change(self.content.to_string(), window, cx);
        }
        self.emit_state_change(window, cx);
    }

    pub(super) fn emit_state_change(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(on_change_with_state) = self.callbacks.on_change_with_state.clone() {
            on_change_with_state(self.snapshot(), window, cx);
        }
    }

    pub(super) fn emit_selection_change_if_needed(
        &self,
        previous_selection: std::ops::Range<usize>,
        previous_selection_reversed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.handling_key_down
            || (self.selected_range == previous_selection
                && self.selection_reversed == previous_selection_reversed)
        {
            return;
        }
        if let Some(on_selection_change) = self.callbacks.on_selection_change.clone() {
            on_selection_change(self.snapshot(), window, cx);
        }
    }

    pub(super) fn selection_anchor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.end
        } else {
            self.selected_range.start
        }
    }

    /// Capture the current text, selection, cursor, and IME composition state.
    pub fn snapshot(&self) -> TextInputSnapshot {
        TextInputSnapshot {
            text: self.content.to_string(),
            selection: self.selected_range.clone(),
            cursor: self.cursor_offset(),
            is_composing: self.marked_range.is_some(),
        }
    }

    pub(super) fn push_undo_snapshot(&mut self) {
        if self.marked_range.is_some() {
            return;
        }
        let snapshot = self.undo_snapshot();
        if self
            .undo_stack
            .last()
            .is_some_and(|previous| previous.content == snapshot.content)
        {
            return;
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
        const MAX_UNDO_SNAPSHOTS: usize = 100;
        if self.undo_stack.len() > MAX_UNDO_SNAPSHOTS {
            self.undo_stack.remove(0);
        }
    }

    fn undo_snapshot(&self) -> TextInputUndoSnapshot {
        TextInputUndoSnapshot {
            content: self.content.clone(),
            selected_range: self.selected_range.clone(),
            selection_reversed: self.selection_reversed,
        }
    }

    fn restore_undo_snapshot(&mut self, snapshot: TextInputUndoSnapshot) {
        self.replace_content(snapshot.content);
        self.selected_range = snapshot.selected_range;
        self.selection_reversed = snapshot.selection_reversed;
        self.snap_selection_to_atoms();
        self.marked_range = None;
        self.desired_x = None;
        self.invalidate_layout();
        self.reset_caret_blink();
        self.ensure_cursor_visible();
    }
}
