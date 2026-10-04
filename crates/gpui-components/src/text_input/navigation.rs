use gpui::{point, px, Context, Modifiers, Window};

use super::layout::{content_position_for_offset, index_for_content_position};
use super::{TextInput, TextInputReplacementOrigin, TextInputVerticalBoundary};

mod delete;

impl TextInput {
    pub(super) fn handle_backspace(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let cursor = self.cursor_offset();
        if self.selected_range.is_empty() && cursor == 0 {
            if self.preflight_text_deletion(
                cursor..cursor,
                TextInputReplacementOrigin::DeleteBackward,
                window,
                cx,
            ) {
                return true;
            }
            if let Some(on_backspace_at_start) = self.callbacks.on_backspace_at_start.clone() {
                on_backspace_at_start(self.snapshot(), modifiers, window, cx);
            } else if self.content.is_empty() {
                if let Some(on_backspace_when_empty) =
                    self.callbacks.on_backspace_when_empty.clone()
                {
                    on_backspace_when_empty(window, cx);
                }
            }
            return true;
        }
        let range = if self.selected_range.is_empty() {
            self.previous_boundary(cursor)..cursor
        } else {
            self.selected_range.clone()
        };
        self.delete_utf8_range_with_origin(
            range,
            TextInputReplacementOrigin::DeleteBackward,
            window,
            cx,
        );
        true
    }

    pub(super) fn handle_delete(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let cursor = self.cursor_offset();
        if self.selected_range.is_empty() && cursor == self.content.len() {
            if self.preflight_text_deletion(
                cursor..cursor,
                TextInputReplacementOrigin::DeleteForward,
                window,
                cx,
            ) {
                return true;
            }
            if let Some(on_delete_at_end) = self.callbacks.on_delete_at_end.clone() {
                on_delete_at_end(self.snapshot(), modifiers, window, cx);
            }
            return true;
        }
        let range = if self.selected_range.is_empty() {
            cursor..self.next_boundary(cursor)
        } else {
            self.selected_range.clone()
        };
        self.delete_utf8_range_with_origin(
            range,
            TextInputReplacementOrigin::DeleteForward,
            window,
            cx,
        );
        true
    }

    pub(super) fn handle_left(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.desired_x = None;
        if let Some(on_left) = self.callbacks.on_left_before_default.clone() {
            if on_left(self.snapshot(), modifiers, window, cx) {
                return true;
            }
        }
        if self.can_leave_selection(modifiers.shift) && self.cursor_offset() == 0 {
            if let Some(on_left_at_start) = self.callbacks.on_left_at_start.clone() {
                on_left_at_start(self.snapshot(), modifiers, window, cx);
                return true;
            }
        }
        if modifiers.shift {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        } else if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
        true
    }

    pub(super) fn move_or_select_to(
        &mut self,
        offset: usize,
        selecting: bool,
        cx: &mut Context<Self>,
    ) {
        self.desired_x = None;
        if selecting {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    pub(super) fn handle_right(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.desired_x = None;
        if let Some(on_right) = self.callbacks.on_right_before_default.clone() {
            if on_right(self.snapshot(), modifiers, window, cx) {
                return true;
            }
        }
        if self.can_leave_selection(modifiers.shift) && self.cursor_offset() == self.content.len() {
            if let Some(on_right_at_end) = self.callbacks.on_right_at_end.clone() {
                on_right_at_end(self.snapshot(), modifiers, window, cx);
                return true;
            }
        }
        if modifiers.shift {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        } else if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
        true
    }

    pub(super) fn handle_up(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !modifiers.shift {
            if let Some(on_up) = self.callbacks.on_up.clone() {
                on_up(window, cx);
                return true;
            }
        }
        if self.can_leave_selection(modifiers.shift) {
            if let Some(on_up_at_first_visual_line) =
                self.callbacks.on_up_at_first_visual_line.clone()
            {
                if let Some(boundary) = self.vertical_boundary(true) {
                    if on_up_at_first_visual_line(boundary, modifiers, window, cx) {
                        return true;
                    }
                }
            }
        }
        self.handle_vertical(-1, modifiers.shift, cx)
    }

    pub(super) fn handle_down(
        &mut self,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !modifiers.shift {
            if let Some(on_down) = self.callbacks.on_down.clone() {
                on_down(window, cx);
                return true;
            }
        }
        if self.can_leave_selection(modifiers.shift) {
            if let Some(on_down_at_last_visual_line) =
                self.callbacks.on_down_at_last_visual_line.clone()
            {
                if let Some(boundary) = self.vertical_boundary(false) {
                    if on_down_at_last_visual_line(boundary, modifiers, window, cx) {
                        return true;
                    }
                }
            }
        }
        self.handle_vertical(1, modifiers.shift, cx)
    }

    fn can_leave_selection(&self, selecting: bool) -> bool {
        selecting || self.selected_range.is_empty()
    }

    fn vertical_boundary(&mut self, first: bool) -> Option<TextInputVerticalBoundary> {
        let cursor = content_position_for_offset(
            self.cursor_offset(),
            self.layout_lines(),
            self.style.line_height,
        )
        .or_else(|| self.content.is_empty().then(|| point(px(0.0), px(0.0))))?;
        let on_outer_line = if first {
            cursor.y < self.style.line_height
        } else {
            cursor.y + self.style.line_height >= self.style.line_height * self.measured_line_count()
        };
        if !on_outer_line {
            return None;
        }
        let bounds = self.last_bounds?;
        let desired_x = self.desired_x.unwrap_or(cursor.x);
        self.desired_x = Some(desired_x);
        Some(TextInputVerticalBoundary {
            snapshot: self.snapshot(),
            window_x: bounds.left() + desired_x - self.scroll_x,
        })
    }

    fn handle_vertical(&mut self, direction: i32, selecting: bool, cx: &mut Context<Self>) -> bool {
        let Some(cursor) = content_position_for_offset(
            self.cursor_offset(),
            self.layout_lines(),
            self.style.line_height,
        ) else {
            return true;
        };
        let x = self.desired_x.unwrap_or(cursor.x);
        self.desired_x = Some(x);
        let next = point(x, cursor.y + self.style.line_height * direction as f32);
        let offset = index_for_content_position(next, self.layout_lines(), self.style.line_height);
        if selecting {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        true
    }

    pub(super) fn move_or_select_to_document_edge(
        &mut self,
        start: bool,
        selecting: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        self.desired_x = None;
        let offset = if start { 0 } else { self.content.len() };
        self.move_or_select_to(offset, selecting, cx);
        true
    }

    fn visual_line_edge(&self, start: bool) -> usize {
        let cursor_offset = self.cursor_offset();
        let fallback = if start {
            self.hard_line_start(cursor_offset)
        } else {
            self.hard_line_end(cursor_offset)
        };
        if !self.soft_wraps() {
            return fallback;
        }
        let Some(bounds) = self.last_bounds else {
            return fallback;
        };
        let Some(cursor) =
            content_position_for_offset(cursor_offset, self.layout_lines(), self.style.line_height)
        else {
            return fallback;
        };
        let x = if start {
            px(0.0)
        } else {
            bounds.size.width.max(px(0.0))
        };
        index_for_content_position(
            point(x, cursor.y),
            self.layout_lines(),
            self.style.line_height,
        )
    }

    pub(super) fn move_or_select_to_visual_line_edge(
        &mut self,
        start: bool,
        selecting: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        self.desired_x = None;
        let offset = self.visual_line_edge(start);
        self.move_or_select_to(offset, selecting, cx);
        true
    }

    pub(super) fn delete_to_visual_line_edge(
        &mut self,
        start: bool,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let origin = if start {
            TextInputReplacementOrigin::DeleteBackward
        } else {
            TextInputReplacementOrigin::DeleteForward
        };
        if !self.selected_range.is_empty() {
            self.delete_utf8_range_with_origin(self.selected_range.clone(), origin, window, cx);
            return true;
        }
        let cursor = self.cursor_offset();
        if self.handle_visual_line_document_edge_delete(start, modifiers, window, cx) {
            return true;
        }
        let edge = self.visual_line_edge(start);
        let edge = if edge == cursor {
            if start {
                self.previous_boundary(cursor)
            } else {
                self.next_boundary(cursor)
            }
        } else {
            edge
        };
        let range = if start { edge..cursor } else { cursor..edge };
        self.delete_utf8_range_with_origin(range, origin, window, cx);
        true
    }
}
