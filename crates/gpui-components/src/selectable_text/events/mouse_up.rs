use crate::text_input::floor_grapheme_boundary;
use gpui::{App, DispatchPhase, MouseUpEvent, SharedString, TextLayout, Window};
use std::{
    cell::Cell,
    ops::Range,
    rc::{Rc, Weak},
    sync::{atomic::Ordering, Arc},
};

use super::super::{
    clipboard::{clear_clipboard_selection, set_clipboard_selection},
    document::{
        clear_selectable_text_document_selection, selectable_text_document_selection_status,
    },
    geometry::{link_range_index_at_index, selection_index_for_position, sorted_range},
    state::{
        SelectableTextClipboardSelection, SelectableTextLinkHandler, SelectableTextState,
        SelectedTextRange, SELECTED_TEXT_ID,
    },
    SelectableText, SelectableTextPosition,
};

struct MouseUpContext {
    text_layout: TextLayout,
    mouse_down_index: Rc<Cell<Option<usize>>>,
    mouse_down_link_index: Rc<Cell<Option<usize>>>,
    mouse_dragged: Rc<Cell<bool>>,
    selected_range: SelectedTextRange,
    link_ranges: Arc<[Range<usize>]>,
    link_handler: Option<SelectableTextLinkHandler>,
    owner: Weak<()>,
    text: SharedString,
    document_position: Option<SelectableTextPosition>,
    selection_id: u64,
}

struct CompletedSelection {
    range: Range<usize>,
    dragged: bool,
    is_empty: bool,
    mouse_down_link: Option<usize>,
}

impl SelectableText {
    pub(in crate::selectable_text) fn bind_mouse_up(
        &self,
        text_layout: &TextLayout,
        state: &SelectableTextState,
        window: &mut Window,
    ) {
        let context = MouseUpContext {
            text_layout: text_layout.clone(),
            mouse_down_index: state.mouse_down_index.clone(),
            mouse_down_link_index: state.mouse_down_link_index.clone(),
            mouse_dragged: state.mouse_dragged.clone(),
            selected_range: state.selected_range.clone(),
            link_ranges: self.link_ranges.clone(),
            link_handler: self.link_handler.clone(),
            owner: Rc::downgrade(&state.owner),
            text: self.text.clone(),
            document_position: self.document_position.clone(),
            selection_id: state.selection_id,
        };
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            handle_mouse_up(&context, event, phase, window, cx);
        });
    }
}

fn handle_mouse_up(
    context: &MouseUpContext,
    event: &MouseUpEvent,
    phase: DispatchPhase,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(anchor) = context.mouse_down_index.get() else {
        return;
    };
    if phase != DispatchPhase::Capture || !event.is_focusing() || !selection_is_active(context) {
        return;
    }
    let completed = complete_mouse_selection(context, anchor, event, window, cx);
    let link_index = completed_link_index(context, &completed, event);
    if let Some(link_index) = link_index {
        clear_completed_selection(context, window, cx);
        if let Some(handler) = context.link_handler.as_ref() {
            handler(link_index, window, cx);
        }
    } else if completed.is_empty {
        clear_completed_selection(context, window, cx);
    } else if context.document_position.is_none() {
        persist_legacy_selection(context, completed.range, window, cx);
    }
    cx.stop_propagation();
    window.refresh();
}

fn selection_is_active(context: &MouseUpContext) -> bool {
    context.document_position.is_some()
        || SELECTED_TEXT_ID.load(Ordering::Relaxed) == context.selection_id
}

fn complete_mouse_selection(
    context: &MouseUpContext,
    anchor: usize,
    event: &MouseUpEvent,
    window: &Window,
    cx: &App,
) -> CompletedSelection {
    context.mouse_down_index.set(None);
    let mouse_down_link = context.mouse_down_link_index.take();
    let local_dragged = context.mouse_dragged.replace(false);
    let index = selection_index_for_position(&context.text, &context.text_layout, event.position);
    let range = sorted_range(anchor, index);
    let (dragged, is_empty) = context
        .document_position
        .as_ref()
        .and_then(|position| {
            selectable_text_document_selection_status(
                window.window_handle().window_id(),
                position,
                cx,
            )
        })
        .unwrap_or((local_dragged, range.is_empty()));
    CompletedSelection {
        range,
        dragged,
        is_empty,
        mouse_down_link,
    }
}

fn completed_link_index(
    context: &MouseUpContext,
    completed: &CompletedSelection,
    event: &MouseUpEvent,
) -> Option<usize> {
    if completed.dragged || !completed.is_empty {
        return None;
    }
    let mouse_up_link = context
        .text_layout
        .index_for_position(event.position)
        .ok()
        .map(|index| floor_grapheme_boundary(context.text.as_ref(), index))
        .and_then(|index| link_range_index_at_index(&context.link_ranges, index));
    (completed.mouse_down_link == mouse_up_link)
        .then_some(completed.mouse_down_link)
        .flatten()
}

fn clear_completed_selection(context: &MouseUpContext, window: &Window, cx: &mut App) {
    *context.selected_range.borrow_mut() = None;
    if let Some(document_position) = context.document_position.as_ref() {
        clear_selectable_text_document_selection(
            window.window_handle().window_id(),
            &document_position.document_id,
            cx,
        );
    } else {
        clear_clipboard_selection(context.selection_id, cx);
    }
}

fn persist_legacy_selection(
    context: &MouseUpContext,
    range: Range<usize>,
    window: &Window,
    cx: &mut App,
) {
    context.selected_range.borrow_mut().replace(range.clone());
    set_clipboard_selection(
        SelectableTextClipboardSelection {
            selection_id: context.selection_id,
            window_id: window.window_handle().window_id(),
            owner: context.owner.clone(),
            text: context.text.clone(),
            range,
        },
        cx,
    );
}
