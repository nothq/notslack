use gpui::{
    point, px, Bounds, Context, Modifiers, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, ScrollDelta, ScrollWheelEvent, Window,
};

use super::{TextInput, TextInputPointerSelection, TextInputPointerSelectionPhase};

/// The pointer position and held modifiers of the mouse event that moves a selection.
#[derive(Clone, Copy)]
struct SelectionPointer {
    position: Point<Pixels>,
    modifiers: Modifiers,
}

impl TextInput {
    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle, cx);
        if let Some(on_focus) = self.callbacks.on_focus.clone() {
            on_focus(window, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
        if event.click_count == 1
            && !event.modifiers.shift
            && self.handle_atom_click(event.position, window, cx)
        {
            return;
        }
        let previous_selection = self.selected_range.clone();
        let previous_selection_reversed = self.selection_reversed;
        self.apply_mouse_down_selection(event, cx);
        self.emit_selection_change_if_needed(
            previous_selection,
            previous_selection_reversed,
            window,
            cx,
        );
        self.emit_pointer_selection(
            TextInputPointerSelectionPhase::Begin,
            SelectionPointer {
                position: event.position,
                modifiers: event.modifiers,
            },
            window,
            cx,
        );
    }

    fn apply_mouse_down_selection(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let previous_anchor = self.selection_anchor_offset();
        let pointer_offset =
            self.floor_grapheme_boundary(self.index_for_mouse_position(event.position));
        self.is_selecting = true;
        self.desired_x = None;
        if event.click_count >= 3 {
            let range = self.hard_line_range_at(pointer_offset);
            self.selected_range = range;
            self.selection_reversed = false;
            self.reset_caret_blink();
            cx.notify();
        } else if event.click_count == 2 {
            let range = self.word_range_at(pointer_offset);
            self.selected_range = range;
            self.selection_reversed = false;
            self.reset_caret_blink();
            cx.notify();
        } else if event.modifiers.shift {
            self.select_to(pointer_offset, cx);
        } else {
            self.move_to(pointer_offset, cx)
        }
        self.pointer_selection_anchor = Some(if event.modifiers.shift {
            previous_anchor
        } else {
            self.selection_anchor_offset()
        });
    }

    /// Hand a click on an atom to the caller instead of moving the caret.
    /// Returns whether the click was consumed.
    fn handle_atom_click(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.atom_at_window_position(position) else {
            return false;
        };
        let Some(on_atom_click) = self.callbacks.on_atom_click.clone() else {
            return false;
        };
        let bounds = self
            .window_bounds_for_atom(index)
            .unwrap_or_else(|| Bounds::from_corners(position, position));
        on_atom_click(index, bounds, window, cx);
        true
    }

    pub(super) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_pointer_selection(TextInputPointerSelectionPhase::End, event, window, cx);
    }

    pub(super) fn on_mouse_up_out(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_pointer_selection(
            TextInputPointerSelectionPhase::EndOutside,
            event,
            window,
            cx,
        );
    }

    fn finish_pointer_selection(
        &mut self,
        phase: TextInputPointerSelectionPhase,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pointer_selection_anchor.is_some() {
            self.emit_pointer_selection(
                phase,
                SelectionPointer {
                    position: event.position,
                    modifiers: event.modifiers,
                },
                window,
                cx,
            );
        }
        self.is_selecting = false;
        self.pointer_selection_anchor = None;
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_selecting {
            if self.sync_hovered_atom(Some(event.position)) {
                cx.notify();
            }
            return;
        }
        {
            let previous_selection = self.selected_range.clone();
            let previous_selection_reversed = self.selection_reversed;
            self.select_to(self.index_for_mouse_position(event.position), cx);
            self.emit_selection_change_if_needed(
                previous_selection,
                previous_selection_reversed,
                window,
                cx,
            );
            self.emit_pointer_selection(
                TextInputPointerSelectionPhase::Update,
                SelectionPointer {
                    position: event.position,
                    modifiers: event.modifiers,
                },
                window,
                cx,
            );
            cx.stop_propagation();
        }
    }

    pub(super) fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pixel_delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta,
            ScrollDelta::Lines(delta) => point(px(delta.x * 20.0), px(delta.y * 20.0)),
        };
        if pixel_delta.x == Pixels::ZERO && pixel_delta.y == Pixels::ZERO {
            return;
        }
        let previous = point(self.scroll_x, self.scroll_y);
        self.scroll_x = (self.scroll_x - pixel_delta.x).clamp(px(0.0), self.max_scroll_x());
        self.scroll_y = (self.scroll_y - pixel_delta.y).clamp(px(0.0), self.max_scroll_y());
        if point(self.scroll_x, self.scroll_y) != previous {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn emit_pointer_selection(
        &self,
        phase: TextInputPointerSelectionPhase,
        pointer: SelectionPointer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectionPointer {
            position,
            modifiers,
        } = pointer;
        let (Some(anchor), Some(on_pointer_selection)) = (
            self.pointer_selection_anchor,
            self.callbacks.on_pointer_selection.clone(),
        ) else {
            return;
        };
        on_pointer_selection(
            TextInputPointerSelection {
                phase,
                position,
                anchor,
                modifiers,
                snapshot: self.snapshot(),
            },
            window,
            cx,
        );
    }
}
