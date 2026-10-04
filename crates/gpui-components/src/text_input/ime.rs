use std::ops::Range;

use gpui::{point, Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::layout::content_position_for_offset;
use super::offsets::utf8_range_from_utf16;
use super::{
    sanitize_for_mode, TextInput, TextInputPreMutationAction, TextInputPreMutationDecision,
    TextInputReplacementOrigin,
};

/// A UTF-8 range and the mode-sanitized text replacing it.
type SanitizedReplacement = (Range<usize>, String);

impl TextInput {
    pub(super) fn active_replacement_range(&self) -> Range<usize> {
        self.marked_range
            .clone()
            .unwrap_or_else(|| self.selected_range.clone())
    }

    pub(super) fn dispatch_pre_mutation_action(
        &mut self,
        action: TextInputPreMutationAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputPreMutationDecision {
        self.callbacks
            .on_pre_mutation_action
            .clone()
            .map_or(TextInputPreMutationDecision::Continue, |handler| {
                handler(action, window, cx)
            })
    }

    pub(super) fn preflight_text_deletion(
        &mut self,
        range: Range<usize>,
        origin: TextInputReplacementOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let text = sanitize_for_mode(self.mode, "");
        self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Replace {
                snapshot: self.snapshot(),
                range,
                text,
                origin,
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
    }

    pub(super) fn delete_utf8_range_with_origin(
        &mut self,
        range: Range<usize>,
        origin: TextInputReplacementOrigin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preflight_text_deletion(range.clone(), origin, window, cx) {
            return;
        }
        if self.selected_range.is_empty() {
            let target = if origin == TextInputReplacementOrigin::DeleteBackward {
                range.start
            } else {
                range.end
            };
            self.select_to(target, cx);
        }
        self.apply_utf8_replacement(self.selected_range.clone(), String::new(), window, cx);
    }

    pub(super) fn apply_utf8_replacement(
        &mut self,
        range: Range<usize>,
        new_text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_selection = self.selected_range.clone();
        let previous_selection_reversed = self.selection_reversed;
        let was_composing = self.marked_range.is_some();
        let text_changed = self.content[range.clone()] != new_text;
        if text_changed {
            self.push_undo_snapshot();
            self.splice_content(range.clone(), &new_text);
        }
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.desired_x = None;
        self.invalidate_layout();
        self.refresh_layout_for_last_width(window, cx);
        self.reset_caret_blink();
        if text_changed {
            self.emit_change(window, cx);
        } else if was_composing {
            self.emit_state_change(window, cx);
        }
        self.emit_selection_change_if_needed(
            previous_selection,
            previous_selection_reversed,
            window,
            cx,
        );
        cx.notify();
    }

    fn prepare_marked_text_replacement(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<SanitizedReplacement> {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let new_text = sanitize_for_mode(self.mode, new_text);
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Replace {
                snapshot: self.snapshot(),
                range: range.clone(),
                text: new_text.clone(),
                origin: TextInputReplacementOrigin::ImeMarked,
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            self.pre_mutation_ime_active = !new_text.is_empty();
            return None;
        }
        self.pre_mutation_ime_active = false;
        Some((range, new_text))
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pre_mutation_ime_active = false;
        if self.marked_range.take().is_some() {
            let previous_selection = self.selected_range.clone();
            let previous_selection_reversed = self.selection_reversed;
            self.normalize_selection_after_composition();
            self.invalidate_layout();
            self.refresh_layout_for_last_width(window, cx);
            self.emit_state_change(window, cx);
            self.emit_selection_change_if_needed(
                previous_selection,
                previous_selection_reversed,
                window,
                cx,
            );
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let origin = if self.marked_range.is_some() || self.pre_mutation_ime_active {
            TextInputReplacementOrigin::ImeCommit
        } else {
            TextInputReplacementOrigin::Typing
        };
        self.pre_mutation_ime_active = false;
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let new_text = sanitize_for_mode(self.mode, new_text);
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Replace {
                snapshot: self.snapshot(),
                range: range.clone(),
                text: new_text.clone(),
                origin,
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            return;
        }
        self.apply_utf8_replacement(range, new_text, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_selection = self.selected_range.clone();
        let previous_selection_reversed = self.selection_reversed;
        let Some((range, new_text)) =
            self.prepare_marked_text_replacement(range_utf16, new_text, window, cx)
        else {
            return;
        };
        let text_changed = self.content[range.clone()] != new_text;
        if text_changed || (!new_text.is_empty() && self.marked_range.is_none()) {
            self.push_undo_snapshot();
        }
        if text_changed {
            self.splice_content(range.clone(), &new_text);
        }
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| utf8_range_from_utf16(&new_text, range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let cursor = range.start + new_text.len();
                cursor..cursor
            });
        self.selection_reversed = false;
        self.desired_x = None;
        self.invalidate_layout();
        self.refresh_layout_for_last_width(window, cx);
        self.reset_caret_blink();
        if text_changed {
            self.emit_change(window, cx);
        } else {
            self.emit_state_change(window, cx);
        }
        self.emit_selection_change_if_needed(
            previous_selection,
            previous_selection_reversed,
            window,
            cx,
        );
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start =
            content_position_for_offset(range.start, self.layout_lines(), self.style.line_height)?;
        let end =
            content_position_for_offset(range.end, self.layout_lines(), self.style.line_height)
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

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        !self.disabled
    }
}
