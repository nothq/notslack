mod mouse_down;
mod mouse_up;

use gpui::{
    App, CursorStyle, DispatchPhase, Hitbox, MouseMoveEvent, Pixels, Point, TextLayout, Window,
};
use std::{rc::Rc, sync::atomic::Ordering};

use super::{
    clipboard::set_clipboard_selection,
    document::selectable_text_document_fragment_range,
    geometry::{
        link_range_index_at_index, selection_index_for_position, sorted_range,
        text_index_for_position,
    },
    paint::paint_text_selection,
    state::{SelectableTextClipboardSelection, SelectableTextState, SELECTED_TEXT_ID},
    SelectableText,
};

impl SelectableText {
    pub(super) fn bind_hover_cursor(
        &self,
        hitbox: &Hitbox,
        text_layout: &TextLayout,
        window: &mut Window,
    ) {
        if !hitbox.is_hovered(window) {
            return;
        }
        let cursor = if self.link_handler.is_some()
            && self
                .link_index_at_position(text_layout, window.mouse_position())
                .is_some()
        {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        };
        window.set_cursor_style(cursor, hitbox);
    }

    pub(super) fn bind_mouse_move(
        &self,
        text_layout: &TextLayout,
        state: &SelectableTextState,
        window: &mut Window,
    ) {
        if self.document_position.is_some() {
            return;
        }
        let text_layout = text_layout.clone();
        let mouse_down_index = state.mouse_down_index.clone();
        let mouse_dragged = state.mouse_dragged.clone();
        let selected_range = state.selected_range.clone();
        let owner = Rc::downgrade(&state.owner);
        let text = self.text.clone();
        let selection_id = state.selection_id;
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            let Some(anchor) = mouse_down_index.get() else {
                return;
            };
            if phase != DispatchPhase::Capture
                || !event.dragging()
                || SELECTED_TEXT_ID.load(Ordering::Relaxed) != selection_id
            {
                return;
            }
            let index = selection_index_for_position(&text, &text_layout, event.position);
            mouse_dragged.set(mouse_dragged.get() || index != anchor);
            let range = sorted_range(anchor, index);
            selected_range.borrow_mut().replace(range.clone());
            set_clipboard_selection(
                SelectableTextClipboardSelection {
                    selection_id,
                    window_id: window.window_handle().window_id(),
                    owner: owner.clone(),
                    text: text.clone(),
                    range,
                },
                cx,
            );
            cx.stop_propagation();
            window.refresh();
        });
    }

    pub(super) fn paint_selection(
        &self,
        text_layout: &TextLayout,
        state: &SelectableTextState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(position) = self.document_position.as_ref() {
            if let Some(range) = selectable_text_document_fragment_range(
                window.window_handle().window_id(),
                position,
                self.text.len(),
                cx,
            ) {
                paint_text_selection(
                    text_layout,
                    &range,
                    self.selection_color,
                    window.text_style().text_align,
                    window,
                );
            }
            return;
        }
        if SELECTED_TEXT_ID.load(Ordering::Relaxed) != state.selection_id {
            *state.selected_range.borrow_mut() = None;
            return;
        }
        if let Some(range) = state.selected_range.borrow().as_ref() {
            paint_text_selection(
                text_layout,
                range,
                self.selection_color,
                window.text_style().text_align,
                window,
            );
        }
    }

    fn link_index_at_position(
        &self,
        text_layout: &TextLayout,
        position: Point<Pixels>,
    ) -> Option<usize> {
        let index = text_index_for_position(&self.text, text_layout, position);
        link_range_index_at_index(&self.link_ranges, index)
    }
}
