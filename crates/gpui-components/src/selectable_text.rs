mod clipboard;
mod document;
mod element;
mod events;
mod geometry;
mod paint;
mod state;

use gpui::{
    AnyElement, App, ElementId, Hitbox, IntoElement, Pixels, SharedString, StyledText, Window,
};
use std::{
    ops::Range,
    rc::{Rc, Weak},
    sync::Arc,
};

use state::{SelectableTextAutoscrollHandler, SelectableTextLinkHandler, SelectableTextOrder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectableTextPosition {
    document_id: SharedString,
    row_index: usize,
    message_index: usize,
    message_id: SharedString,
    fragment_index: usize,
    fragment_id: SharedString,
    separator_before: SharedString,
}

/// The message a selectable fragment belongs to, placed in document reading order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectableTextMessage {
    pub row_index: usize,
    pub message_index: usize,
    pub message_id: SharedString,
}

impl SelectableTextPosition {
    pub fn new(
        document_id: impl Into<SharedString>,
        message: SelectableTextMessage,
        fragment_index: usize,
        fragment_id: impl Into<SharedString>,
        separator_before: impl Into<SharedString>,
    ) -> Self {
        let SelectableTextMessage {
            row_index,
            message_index,
            message_id,
        } = message;
        Self {
            document_id: document_id.into(),
            row_index,
            message_index,
            message_id,
            fragment_index,
            fragment_id: fragment_id.into(),
            separator_before: separator_before.into(),
        }
    }

    fn order(&self) -> SelectableTextOrder {
        SelectableTextOrder {
            row_index: self.row_index,
            message_index: self.message_index,
            fragment_index: self.fragment_index,
        }
    }

    pub(super) fn same_fragment(&self, other: &Self) -> bool {
        self.document_id == other.document_id
            && self.row_index == other.row_index
            && self.message_index == other.message_index
            && self.message_id == other.message_id
            && self.fragment_index == other.fragment_index
            && self.fragment_id == other.fragment_id
    }
}

/// Styled, read-only text with native mouse selection, clipboard copy, and optional links.
///
/// Selection state is retained by GPUI's element state, so callers can render this directly in a
/// virtualized row without allocating an entity per text fragment.
pub struct SelectableText {
    element_id: ElementId,
    text: SharedString,
    styled_text: StyledText,
    selection_color: gpui::Hsla,
    document_position: Option<SelectableTextPosition>,
    link_ranges: Arc<[Range<usize>]>,
    link_handler: Option<SelectableTextLinkHandler>,
}

impl SelectableText {
    pub fn new(
        element_id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        styled_text: StyledText,
        selection_color: gpui::Hsla,
    ) -> Self {
        Self {
            element_id: element_id.into(),
            text: text.into(),
            styled_text,
            selection_color,
            document_position: None,
            link_ranges: Arc::default(),
            link_handler: None,
        }
    }

    pub fn in_document(mut self, position: SelectableTextPosition) -> Self {
        self.document_position = Some(position);
        self
    }

    pub fn on_link_click(
        mut self,
        ranges: impl Into<Arc<[Range<usize>]>>,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.link_ranges = ranges.into();
        self.link_handler = Some(Rc::new(handler));
        self
    }
}

pub struct SelectableTextDocument {
    element_id: ElementId,
    document_id: SharedString,
    child: Option<AnyElement>,
    autoscroll_handler: Option<SelectableTextAutoscrollHandler>,
}

pub struct SelectableTextDocumentPrepaintState {
    hitbox: Hitbox,
    owner: Weak<()>,
}

impl SelectableTextDocument {
    pub fn new(
        element_id: impl Into<ElementId>,
        document_id: impl Into<SharedString>,
        child: impl IntoElement,
    ) -> Self {
        Self {
            element_id: element_id.into(),
            document_id: document_id.into(),
            child: Some(child.into_any_element()),
            autoscroll_handler: None,
        }
    }

    pub fn on_autoscroll(
        mut self,
        handler: impl Fn(Pixels, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.autoscroll_handler = Some(Rc::new(handler));
        self
    }
}
