use gpui::{App, Hitbox, SharedString, WindowId};
use std::{collections::BTreeMap, rc::Weak};

use super::super::{
    state::{
        SelectableTextClipboard, SelectableTextDocumentRegistration,
        SelectableTextRegisteredFragment, SelectableTextVisibleFragment,
        SelectableTextWindowCoordinator,
    },
    SelectableTextPosition,
};

pub(in crate::selectable_text) fn invalidate_selectable_text_document(
    window_id: WindowId,
    document_id: &SharedString,
    owner: &Weak<()>,
    cx: &mut App,
) {
    let Some(window) = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .get_mut(&window_id)
    else {
        return;
    };
    let owns_document = window
        .documents
        .get(document_id)
        .is_some_and(|document| document.owner.ptr_eq(owner));
    if !owns_document {
        return;
    }
    window.documents.remove(document_id);
    if window
        .selection
        .as_ref()
        .is_some_and(|selection| selection.owner.ptr_eq(owner))
    {
        window.selection = None;
    }
}

pub(in crate::selectable_text) fn begin_selectable_text_document_frame(
    window_id: WindowId,
    document_id: SharedString,
    owner: Weak<()>,
    root_hitbox: &Hitbox,
    cx: &mut App,
) {
    let root_hitbox_id = root_hitbox.id;
    let viewport_bounds = root_hitbox.bounds;
    let window = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .entry(window_id)
        .or_default();
    replace_document_owner(window, &document_id, &owner);
    let document =
        window
            .documents
            .entry(document_id)
            .or_insert_with(|| SelectableTextDocumentRegistration {
                owner: owner.clone(),
                root_hitbox_id,
                viewport_bounds,
                fragments: BTreeMap::new(),
                visible_orders: Vec::new(),
            });
    for order in std::mem::take(&mut document.visible_orders) {
        if let Some(fragment) = document.fragments.get_mut(&order) {
            fragment.visible = None;
        }
    }
    document.owner = owner;
    document.root_hitbox_id = root_hitbox_id;
    document.viewport_bounds = viewport_bounds;
}

fn replace_document_owner(
    window: &mut SelectableTextWindowCoordinator,
    document_id: &SharedString,
    owner: &Weak<()>,
) {
    let replaces_document = window
        .documents
        .get(document_id)
        .is_some_and(|document| !document.owner.ptr_eq(owner));
    if !replaces_document {
        return;
    }
    window.documents.remove(document_id);
    if window
        .selection
        .as_ref()
        .is_some_and(|selection| selection.document_id == *document_id)
    {
        window.selection = None;
    }
}

pub(in crate::selectable_text) fn register_selectable_text_document_fragment(
    window_id: WindowId,
    position: SelectableTextPosition,
    text: SharedString,
    visible: SelectableTextVisibleFragment,
    cx: &mut App,
) {
    let window = cx
        .global_mut::<SelectableTextClipboard>()
        .windows
        .entry(window_id)
        .or_default();
    let SelectableTextWindowCoordinator {
        documents,
        selection,
    } = window;
    let Some(document) = documents.get_mut(&position.document_id) else {
        return;
    };
    if document.owner.upgrade().is_none() {
        return;
    }
    let order = position.order();
    let invalidates_selection = document.fragments.get(&order).is_some_and(|fragment| {
        (!fragment.position.same_fragment(&position) || fragment.text != text)
            && selection.as_ref().is_some_and(|active| {
                active.anchor.position.order() == order || active.focus.position.order() == order
            })
    });
    if invalidates_selection {
        *selection = None;
    }
    let fragment =
        document
            .fragments
            .entry(order)
            .or_insert_with(|| SelectableTextRegisteredFragment {
                position: position.clone(),
                text: text.clone(),
                visible: None,
            });
    let newly_visible = fragment.visible.is_none();
    fragment.position = position;
    fragment.text = text;
    fragment.visible = Some(visible);
    if newly_visible {
        document.visible_orders.push(order);
    }
}

pub(super) fn prune_unselected_selectable_text_document_fragments(
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
        return;
    }
    if let Some(document) = window.documents.get_mut(document_id) {
        document
            .fragments
            .retain(|_, fragment| fragment.visible.is_some());
    }
}
