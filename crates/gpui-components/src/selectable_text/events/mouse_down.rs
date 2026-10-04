use gpui::{App, DispatchPhase, Hitbox, MouseDownEvent, SharedString, TextLayout, Window};
use std::{
    cell::Cell,
    ops::Range,
    rc::Rc,
    sync::{atomic::Ordering, Arc},
};

use super::super::{
    clipboard::{clear_clipboard_selection, clear_legacy_clipboard_selection},
    document::begin_selectable_text_document_selection,
    geometry::{link_range_index_at_index, selection_index_for_position, text_index_for_position},
    state::{SelectableTextState, SelectedTextRange, SELECTED_TEXT_ID},
    SelectableText, SelectableTextPosition,
};

struct MouseDownContext {
    hitbox: Hitbox,
    text_layout: TextLayout,
    mouse_down_index: Rc<Cell<Option<usize>>>,
    mouse_down_link_index: Rc<Cell<Option<usize>>>,
    mouse_dragged: Rc<Cell<bool>>,
    selected_range: SelectedTextRange,
    link_ranges: Arc<[Range<usize>]>,
    text: SharedString,
    document_position: Option<SelectableTextPosition>,
    selection_id: u64,
}

impl SelectableText {
    pub(in crate::selectable_text) fn bind_mouse_down(
        &self,
        hitbox: &Hitbox,
        text_layout: &TextLayout,
        state: &SelectableTextState,
        window: &mut Window,
    ) {
        let context = MouseDownContext {
            hitbox: hitbox.clone(),
            text_layout: text_layout.clone(),
            mouse_down_index: state.mouse_down_index.clone(),
            mouse_down_link_index: state.mouse_down_link_index.clone(),
            mouse_dragged: state.mouse_dragged.clone(),
            selected_range: state.selected_range.clone(),
            link_ranges: self.link_ranges.clone(),
            text: self.text.clone(),
            document_position: self.document_position.clone(),
            selection_id: state.selection_id,
        };
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            handle_mouse_down(&context, event, phase, window, cx);
        });
    }
}

fn handle_mouse_down(
    context: &MouseDownContext,
    event: &MouseDownEvent,
    phase: DispatchPhase,
    window: &mut Window,
    cx: &mut App,
) {
    if phase != DispatchPhase::Capture || !event.is_focusing() {
        return;
    }
    let Some((index, link_index)) = hovered_indices(context, event, window) else {
        clear_unhovered_legacy_selection(context, window, cx);
        return;
    };
    window.blur();
    initialize_mouse_selection(context, index, link_index);
    capture_mouse_selection(context, index, event, window, cx);
    cx.stop_propagation();
    window.refresh();
}

fn hovered_indices(
    context: &MouseDownContext,
    event: &MouseDownEvent,
    window: &Window,
) -> Option<(usize, usize)> {
    context.hitbox.is_hovered(window).then(|| {
        (
            selection_index_for_position(&context.text, &context.text_layout, event.position),
            text_index_for_position(&context.text, &context.text_layout, event.position),
        )
    })
}

fn clear_unhovered_legacy_selection(context: &MouseDownContext, window: &mut Window, cx: &mut App) {
    if context.document_position.is_some()
        || SELECTED_TEXT_ID.load(Ordering::Relaxed) != context.selection_id
    {
        return;
    }
    SELECTED_TEXT_ID.store(0, Ordering::Relaxed);
    context.mouse_down_index.set(None);
    context.mouse_down_link_index.set(None);
    context.mouse_dragged.set(false);
    *context.selected_range.borrow_mut() = None;
    clear_clipboard_selection(context.selection_id, cx);
    window.refresh();
}

fn initialize_mouse_selection(context: &MouseDownContext, index: usize, link_index: usize) {
    context.mouse_down_index.set(Some(index));
    context
        .mouse_down_link_index
        .set(link_range_index_at_index(&context.link_ranges, link_index));
    context.mouse_dragged.set(false);
    context.selected_range.borrow_mut().replace(index..index);
}

fn capture_mouse_selection(
    context: &MouseDownContext,
    index: usize,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(document_position) = context.document_position.as_ref() {
        SELECTED_TEXT_ID.store(0, Ordering::Relaxed);
        clear_legacy_clipboard_selection(cx);
        let root_hitbox = begin_selectable_text_document_selection(
            window.window_handle().window_id(),
            document_position.clone(),
            index,
            event.position,
            cx,
        );
        if let Some(root_hitbox) = root_hitbox {
            window.capture_pointer(root_hitbox);
        }
    } else {
        SELECTED_TEXT_ID.store(context.selection_id, Ordering::Relaxed);
        clear_legacy_clipboard_selection(cx);
        window.capture_pointer(context.hitbox.id);
    }
}
