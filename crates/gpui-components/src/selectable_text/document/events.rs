use gpui::{
    px, App, DispatchPhase, Hitbox, MouseDownEvent, MouseMoveEvent, MouseUpEvent, SharedString,
    Window, WindowId,
};
use std::sync::atomic::Ordering;

use super::{
    super::{
        clipboard::clear_legacy_clipboard_selection,
        state::{SelectableTextAutoscrollHandler, SelectableTextClipboard, SELECTED_TEXT_ID},
    },
    frame::prune_unselected_selectable_text_document_fragments,
    selection::{
        clear_selectable_text_document_selection, update_selectable_text_document_pointer,
    },
};

pub(in crate::selectable_text) fn bind_selectable_text_document_events(
    document_id: SharedString,
    hitbox: Hitbox,
    autoscroll_handler: Option<SelectableTextAutoscrollHandler>,
    window: &mut Window,
) {
    bind_document_mouse_down(document_id.clone(), window);
    bind_document_mouse_move(document_id.clone(), autoscroll_handler, window);
    bind_document_mouse_up(document_id, hitbox, window);
}

fn bind_document_mouse_down(document_id: SharedString, window: &mut Window) {
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !event.is_focusing() {
            return;
        }
        SELECTED_TEXT_ID.store(0, Ordering::Relaxed);
        clear_legacy_clipboard_selection(cx);
        clear_selectable_text_document_selection(
            window.window_handle().window_id(),
            &document_id,
            cx,
        );
    });
}

fn bind_document_mouse_move(
    document_id: SharedString,
    autoscroll_handler: Option<SelectableTextAutoscrollHandler>,
    window: &mut Window,
) {
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !event.dragging() {
            return;
        }
        let Some(autoscroll) = update_selectable_text_document_pointer(
            window.window_handle().window_id(),
            &document_id,
            event.position,
            false,
            cx,
        ) else {
            return;
        };
        if autoscroll != px(0.0) {
            if let Some(handler) = autoscroll_handler.as_ref() {
                handler(autoscroll, window, cx);
            }
        }
        cx.stop_propagation();
        window.refresh();
    });
}

fn bind_document_mouse_up(document_id: SharedString, hitbox: Hitbox, window: &mut Window) {
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || !event.is_focusing() {
            return;
        }
        if update_selectable_text_document_pointer(
            window.window_handle().window_id(),
            &document_id,
            event.position,
            true,
            cx,
        )
        .is_some()
        {
            window.refresh();
        }
        if window.captured_hitbox() == Some(hitbox.id) {
            window.release_pointer();
        }
    });
}

pub(in crate::selectable_text) fn finish_selectable_text_document_frame(
    window_id: WindowId,
    document_id: &SharedString,
    autoscroll_handler: Option<&SelectableTextAutoscrollHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    prune_unselected_selectable_text_document_fragments(window_id, document_id, cx);
    let pointer_position = cx
        .global::<SelectableTextClipboard>()
        .windows
        .get(&window_id)
        .and_then(|window| window.selection.as_ref())
        .filter(|selection| selection.document_id == *document_id && selection.dragging)
        .map(|selection| selection.pointer_position);
    let Some(pointer_position) = pointer_position else {
        return;
    };
    let Some(autoscroll) = update_selectable_text_document_pointer(
        window_id,
        document_id,
        pointer_position,
        false,
        cx,
    ) else {
        return;
    };
    if autoscroll == px(0.0) {
        return;
    }
    if let Some(handler) = autoscroll_handler {
        handler(autoscroll, window, cx);
        window.request_animation_frame();
        window.refresh();
    }
}
