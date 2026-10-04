use gpui::{px, App, Bounds, HitboxId, Pixels, Point, SharedString, WindowId};
use std::ops::Range;

use super::super::{
    geometry::selection_index_for_position,
    state::{
        SelectableTextCaret, SelectableTextClipboard, SelectableTextDocumentRegistration,
        SelectableTextDocumentSelection, SelectableTextOrder, SelectableTextRegisteredFragment,
        SelectableTextWindowCoordinator,
    },
    SelectableTextPosition,
};

pub(in crate::selectable_text) fn begin_selectable_text_document_selection(
    window_id: WindowId,
    position: SelectableTextPosition,
    index: usize,
    pointer_position: Point<Pixels>,
    cx: &mut App,
) -> Option<HitboxId> {
    let window = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .get_mut(&window_id)?;
    let document = window.documents.get(&position.document_id)?;
    let fragment = document.fragments.get(&position.order())?;
    if document.owner.upgrade().is_none()
        || !fragment.position.same_fragment(&position)
        || index > fragment.text.len()
        || !fragment.text.is_char_boundary(index)
    {
        window.selection = None;
        return None;
    }
    let document_owner = document.owner.clone();
    let root_hitbox_id = document.root_hitbox_id;
    let caret = SelectableTextCaret { position, index };
    window.selection = Some(SelectableTextDocumentSelection {
        document_id: caret.position.document_id.clone(),
        owner: document_owner,
        anchor: caret.clone(),
        focus: caret,
        pointer_position,
        dragging: true,
        dragged: false,
    });
    Some(root_hitbox_id)
}

pub(super) fn update_selectable_text_document_pointer(
    window_id: WindowId,
    document_id: &SharedString,
    pointer_position: Point<Pixels>,
    finish: bool,
    cx: &mut App,
) -> Option<Pixels> {
    let window = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .get_mut(&window_id)?;
    let SelectableTextWindowCoordinator {
        documents,
        selection,
    } = window;
    let document = documents.get(document_id)?;
    let invalid_selection = selection.as_ref().is_some_and(|active| {
        active.document_id != *document_id
            || active.owner.upgrade().is_none()
            || !active.owner.ptr_eq(&document.owner)
    });
    if invalid_selection {
        *selection = None;
        return None;
    }
    let active = selection.as_mut()?;
    active.pointer_position = pointer_position;
    if let Some(focus) = nearest_selectable_text_caret(document, pointer_position) {
        if !active.anchor.position.same_fragment(&focus.position)
            || active.anchor.index != focus.index
        {
            active.dragged = true;
        }
        active.focus = focus;
    }
    if finish {
        active.dragging = false;
    }
    Some(if active.dragging {
        selectable_text_autoscroll_delta(pointer_position, document.viewport_bounds)
    } else {
        px(0.0)
    })
}

fn nearest_selectable_text_caret(
    document: &SelectableTextDocumentRegistration,
    pointer_position: Point<Pixels>,
) -> Option<SelectableTextCaret> {
    let fragment = document
        .visible_orders
        .iter()
        .filter_map(|order| document.fragments.get(order))
        .filter_map(|fragment| {
            fragment.visible.as_ref().map(|visible| {
                (
                    bounds_distance_squared(pointer_position, visible.bounds),
                    fragment,
                    visible,
                )
            })
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, fragment, visible)| (fragment, visible))?;
    Some(SelectableTextCaret {
        position: fragment.0.position.clone(),
        index: selection_index_for_position(
            fragment.0.text.as_ref(),
            &fragment.1.text_layout,
            pointer_position,
        ),
    })
}

fn bounds_distance_squared(position: Point<Pixels>, bounds: Bounds<Pixels>) -> f32 {
    let x = if position.x < bounds.left() {
        (bounds.left() - position.x).as_f32()
    } else if position.x > bounds.right() {
        (position.x - bounds.right()).as_f32()
    } else {
        0.0
    };
    let y = if position.y < bounds.top() {
        (bounds.top() - position.y).as_f32()
    } else if position.y > bounds.bottom() {
        (position.y - bounds.bottom()).as_f32()
    } else {
        0.0
    };
    x * x + y * y
}

fn selectable_text_autoscroll_delta(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Pixels {
    const EDGE_MARGIN: f32 = 28.0;
    const MIN_SPEED: f32 = 4.0;
    const MAX_SPEED: f32 = 32.0;
    let top_overflow = (bounds.top() + px(EDGE_MARGIN) - position.y)
        .as_f32()
        .max(0.0);
    if top_overflow > 0.0 {
        return px(-(MIN_SPEED + top_overflow * 0.35).min(MAX_SPEED));
    }
    let bottom_overflow = (position.y - (bounds.bottom() - px(EDGE_MARGIN)))
        .as_f32()
        .max(0.0);
    if bottom_overflow > 0.0 {
        return px((MIN_SPEED + bottom_overflow * 0.35).min(MAX_SPEED));
    }
    px(0.0)
}

pub(in crate::selectable_text) fn selectable_text_document_selection_status(
    window_id: WindowId,
    position: &SelectableTextPosition,
    cx: &App,
) -> Option<(bool, bool)> {
    let selection = cx
        .global::<SelectableTextClipboard>()
        .windows
        .get(&window_id)?
        .selection
        .as_ref()?;
    if selection.document_id != position.document_id
        || !selection.anchor.position.same_fragment(position)
    {
        return None;
    }
    Some((
        selection.dragged,
        selection
            .anchor
            .position
            .same_fragment(&selection.focus.position)
            && selection.anchor.index == selection.focus.index,
    ))
}

pub(in crate::selectable_text) fn clear_selectable_text_document_selection(
    window_id: WindowId,
    document_id: &SharedString,
    cx: &mut App,
) {
    let Some(window) = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .get_mut(&window_id)
    else {
        return;
    };
    if window
        .selection
        .as_ref()
        .is_some_and(|selection| selection.document_id == *document_id)
    {
        window.selection = None;
    }
}

pub(in crate::selectable_text) fn selectable_text_document_fragment_range(
    window_id: WindowId,
    position: &SelectableTextPosition,
    text_len: usize,
    cx: &App,
) -> Option<Range<usize>> {
    let window = cx
        .global::<SelectableTextClipboard>()
        .windows
        .get(&window_id)?;
    let selection = window.selection.as_ref()?;
    let document = window.documents.get(&selection.document_id)?;
    let (start, end) = normalized_selectable_text_document_selection(document, selection)?;
    let order = position.order();
    if order < start.position.order() || order > end.position.order() {
        return None;
    }
    let fragment = document.fragments.get(&order)?;
    if !fragment.position.same_fragment(position) || fragment.text.len() != text_len {
        return None;
    }
    document_fragment_range(order, text_len, start, end)
}

fn document_fragment_range(
    order: SelectableTextOrder,
    text_len: usize,
    start: &SelectableTextCaret,
    end: &SelectableTextCaret,
) -> Option<Range<usize>> {
    if start.position.order() == end.position.order() {
        return (start.index < end.index).then_some(start.index..end.index);
    }
    if order == start.position.order() {
        return (start.index < text_len).then_some(start.index..text_len);
    }
    if order == end.position.order() {
        return (end.index > 0).then_some(0..end.index);
    }
    (text_len > 0).then_some(0..text_len)
}

fn normalized_selectable_text_document_selection<'a>(
    document: &'a SelectableTextDocumentRegistration,
    selection: &'a SelectableTextDocumentSelection,
) -> Option<(&'a SelectableTextCaret, &'a SelectableTextCaret)> {
    if selection.owner.upgrade().is_none()
        || !selection.owner.ptr_eq(&document.owner)
        || !selectable_text_caret_is_valid(document, &selection.anchor)
        || !selectable_text_caret_is_valid(document, &selection.focus)
    {
        return None;
    }
    let anchor_key = (selection.anchor.position.order(), selection.anchor.index);
    let focus_key = (selection.focus.position.order(), selection.focus.index);
    if anchor_key <= focus_key {
        Some((&selection.anchor, &selection.focus))
    } else {
        Some((&selection.focus, &selection.anchor))
    }
}

fn selectable_text_caret_is_valid(
    document: &SelectableTextDocumentRegistration,
    caret: &SelectableTextCaret,
) -> bool {
    document
        .fragments
        .get(&caret.position.order())
        .is_some_and(|fragment| {
            fragment.position.same_fragment(&caret.position)
                && caret.index <= fragment.text.len()
                && fragment.text.is_char_boundary(caret.index)
        })
}

pub(in crate::selectable_text) fn selectable_text_document_clipboard_text(
    window_id: WindowId,
    cx: &mut App,
) -> Option<String> {
    let result = {
        let window = cx
            .global::<SelectableTextClipboard>()
            .windows
            .get(&window_id)?;
        let selection = window.selection.as_ref()?;
        let document = window.documents.get(&selection.document_id)?;
        serialize_document_selection(document, selection)
    };
    if result.is_none() {
        if let Some(window) = cx
            .global_mut::<SelectableTextClipboard>()
            .windows
            .get_mut(&window_id)
        {
            window.selection = None;
        }
    }
    result
}

fn serialize_document_selection(
    document: &SelectableTextDocumentRegistration,
    selection: &SelectableTextDocumentSelection,
) -> Option<String> {
    let (start, end) = normalized_selectable_text_document_selection(document, selection)?;
    if start.position.same_fragment(&end.position) && start.index == end.index {
        return None;
    }
    let mut selected_text = String::new();
    let mut first_fragment = true;
    for (order, fragment) in document
        .fragments
        .range(start.position.order()..=end.position.order())
    {
        let range = serialized_fragment_range(*order, fragment, start, end);
        if !first_fragment {
            selected_text.push_str(fragment.position.separator_before.as_ref());
        }
        selected_text.push_str(fragment.text.get(range)?);
        first_fragment = false;
    }
    (!first_fragment).then_some(selected_text)
}

fn serialized_fragment_range(
    order: SelectableTextOrder,
    fragment: &SelectableTextRegisteredFragment,
    start: &SelectableTextCaret,
    end: &SelectableTextCaret,
) -> Range<usize> {
    if start.position.order() == end.position.order() {
        start.index..end.index
    } else if order == start.position.order() {
        start.index..fragment.text.len()
    } else if order == end.position.order() {
        0..end.index
    } else {
        0..fragment.text.len()
    }
}
