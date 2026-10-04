use gpui::{
    App, Bounds, Global, HitboxId, Pixels, Point, SharedString, Subscription, TextLayout, Window,
    WindowId,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    ops::Range,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

use super::SelectableTextPosition;

static NEXT_SELECTABLE_TEXT_ID: AtomicU64 = AtomicU64::new(1);
pub(super) static SELECTED_TEXT_ID: AtomicU64 = AtomicU64::new(0);

pub(super) type SelectedTextRange = Rc<RefCell<Option<Range<usize>>>>;
pub(super) type SelectableTextLinkHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
pub(super) type SelectableTextAutoscrollHandler = Rc<dyn Fn(Pixels, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SelectableTextOrder {
    pub(super) row_index: usize,
    pub(super) message_index: usize,
    pub(super) fragment_index: usize,
}

#[derive(Clone)]
pub(super) struct SelectableTextCaret {
    pub(super) position: SelectableTextPosition,
    pub(super) index: usize,
}

pub(super) struct SelectableTextDocumentSelection {
    pub(super) document_id: SharedString,
    pub(super) owner: Weak<()>,
    pub(super) anchor: SelectableTextCaret,
    pub(super) focus: SelectableTextCaret,
    pub(super) pointer_position: Point<Pixels>,
    pub(super) dragging: bool,
    pub(super) dragged: bool,
}

pub(super) struct SelectableTextVisibleFragment {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) text_layout: TextLayout,
}

pub(super) struct SelectableTextRegisteredFragment {
    pub(super) position: SelectableTextPosition,
    pub(super) text: SharedString,
    pub(super) visible: Option<SelectableTextVisibleFragment>,
}

pub(super) struct SelectableTextDocumentRegistration {
    pub(super) owner: Weak<()>,
    pub(super) root_hitbox_id: HitboxId,
    pub(super) viewport_bounds: Bounds<Pixels>,
    pub(super) fragments: BTreeMap<SelectableTextOrder, SelectableTextRegisteredFragment>,
    pub(super) visible_orders: Vec<SelectableTextOrder>,
}

#[derive(Default)]
pub(super) struct SelectableTextWindowCoordinator {
    pub(super) documents: HashMap<SharedString, SelectableTextDocumentRegistration>,
    pub(super) selection: Option<SelectableTextDocumentSelection>,
}

#[derive(Clone)]
pub(super) struct SelectableTextClipboardSelection {
    pub(super) selection_id: u64,
    pub(super) window_id: WindowId,
    pub(super) owner: Weak<()>,
    pub(super) text: SharedString,
    pub(super) range: Range<usize>,
}

pub(super) struct SelectableTextClipboard {
    pub(super) selection: Option<SelectableTextClipboardSelection>,
    pub(super) windows: HashMap<WindowId, SelectableTextWindowCoordinator>,
    pub(super) _subscription: Subscription,
}

impl Global for SelectableTextClipboard {}

pub(super) struct SelectableTextDocumentState {
    pub(super) document_id: SharedString,
    pub(super) owner: Rc<()>,
}

pub(super) struct SelectableTextState {
    pub(super) selection_id: u64,
    pub(super) owner: Rc<()>,
    pub(super) rendered_text: Option<SharedString>,
    pub(super) mouse_down_index: Rc<Cell<Option<usize>>>,
    pub(super) mouse_down_link_index: Rc<Cell<Option<usize>>>,
    pub(super) mouse_dragged: Rc<Cell<bool>>,
    pub(super) selected_range: SelectedTextRange,
}

impl Default for SelectableTextState {
    fn default() -> Self {
        Self {
            selection_id: NEXT_SELECTABLE_TEXT_ID.fetch_add(1, Ordering::Relaxed),
            owner: Rc::new(()),
            rendered_text: None,
            mouse_down_index: Rc::default(),
            mouse_down_link_index: Rc::default(),
            mouse_dragged: Rc::default(),
            selected_range: Rc::default(),
        }
    }
}

impl SelectableTextState {
    pub(super) fn prepare_for_text(&mut self, text: &SharedString) {
        if self.rendered_text.as_ref() == Some(text) {
            return;
        }
        if SELECTED_TEXT_ID.load(Ordering::Relaxed) == self.selection_id {
            SELECTED_TEXT_ID.store(0, Ordering::Relaxed);
        }
        self.rendered_text = Some(text.clone());
        self.mouse_down_index.set(None);
        self.mouse_down_link_index.set(None);
        self.mouse_dragged.set(false);
        *self.selected_range.borrow_mut() = None;
    }
}
