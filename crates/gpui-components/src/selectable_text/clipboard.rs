use gpui::{App, ClipboardItem, KeystrokeEvent, Window, WindowId};
use std::{collections::HashMap, sync::atomic::Ordering};

use super::{
    document::selectable_text_document_clipboard_text,
    state::{SelectableTextClipboard, SelectableTextClipboardSelection, SELECTED_TEXT_ID},
};

pub(super) fn ensure_selectable_text_clipboard(cx: &mut App) {
    if cx.has_global::<SelectableTextClipboard>() {
        return;
    }
    let subscription = cx.intercept_keystrokes(handle_copy_keystroke);
    cx.set_global(SelectableTextClipboard {
        selection: None,
        windows: HashMap::new(),
        _subscription: subscription,
    });
}

fn handle_copy_keystroke(event: &KeystrokeEvent, window: &mut Window, cx: &mut App) {
    if event.keystroke.modifiers != gpui::Modifiers::command()
        || event.keystroke.key.as_str() != "c"
    {
        return;
    }
    let window_id = window.window_handle().window_id();
    let has_document_selection = cx
        .global::<SelectableTextClipboard>()
        .windows
        .get(&window_id)
        .and_then(|coordinator| coordinator.selection.as_ref())
        .is_some();
    if has_document_selection {
        copy_document_selection(window_id, cx);
        return;
    }
    copy_legacy_selection(window_id, cx);
}

fn copy_document_selection(window_id: WindowId, cx: &mut App) {
    if let Some(selected_text) = selectable_text_document_clipboard_text(window_id, cx) {
        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
        cx.stop_propagation();
    } else if let Some(window) = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .get_mut(&window_id)
    {
        window.selection = None;
    }
}

fn copy_legacy_selection(window_id: WindowId, cx: &mut App) {
    let Some(selection) = cx.global::<SelectableTextClipboard>().selection.clone() else {
        return;
    };
    if selection.owner.upgrade().is_none()
        || SELECTED_TEXT_ID.load(Ordering::Relaxed) != selection.selection_id
    {
        clear_clipboard_selection(selection.selection_id, cx);
        return;
    }
    if selection.window_id != window_id {
        return;
    }
    let Some(selected_text) = selection.text.get(selection.range) else {
        return;
    };
    cx.write_to_clipboard(ClipboardItem::new_string(selected_text.to_string()));
    cx.stop_propagation();
}

pub(super) fn clear_legacy_clipboard_selection(cx: &mut App) {
    cx.global_mut::<SelectableTextClipboard>().selection = None;
}

pub(super) fn set_clipboard_selection(selection: SelectableTextClipboardSelection, cx: &mut App) {
    cx.global_mut::<SelectableTextClipboard>().selection =
        (!selection.range.is_empty()).then_some(selection);
}

pub(super) fn clear_clipboard_selection(selection_id: u64, cx: &mut App) {
    let clipboard = cx.global_mut::<SelectableTextClipboard>();
    if clipboard
        .selection
        .as_ref()
        .is_some_and(|selection| selection.selection_id == selection_id)
    {
        clipboard.selection = None;
    }
}
