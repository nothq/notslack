use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};
use std::sync::atomic::Ordering;

use super::{
    clipboard::ensure_selectable_text_clipboard,
    document::{
        begin_selectable_text_document_frame, bind_selectable_text_document_events,
        finish_selectable_text_document_frame, invalidate_selectable_text_document,
        register_selectable_text_document_fragment,
    },
    state::{
        SelectableTextDocumentState, SelectableTextState, SelectableTextVisibleFragment,
        SELECTED_TEXT_ID,
    },
    SelectableText, SelectableTextDocument, SelectableTextDocumentPrepaintState,
};

impl IntoElement for SelectableTextDocument {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableTextDocument {
    type RequestLayoutState = ();
    type PrepaintState = SelectableTextDocumentPrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self
            .child
            .as_mut()
            .expect("selectable text document child must exist")
            .request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        ensure_selectable_text_clipboard(cx);
        let global_id = global_id.expect("selectable text document must have an element id");
        let window_id = window.window_handle().window_id();
        let document_id = self.document_id.clone();
        let owner = window.with_element_state::<SelectableTextDocumentState, _>(
            global_id,
            |state, _window| {
                let mut state = state.unwrap_or_else(|| SelectableTextDocumentState {
                    document_id: document_id.clone(),
                    owner: std::rc::Rc::new(()),
                });
                if state.document_id != document_id {
                    invalidate_selectable_text_document(
                        window_id,
                        &state.document_id,
                        &std::rc::Rc::downgrade(&state.owner),
                        cx,
                    );
                    state.document_id = document_id;
                    state.owner = std::rc::Rc::new(());
                }
                (std::rc::Rc::downgrade(&state.owner), state)
            },
        );
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        self.child
            .as_mut()
            .expect("selectable text document child must exist")
            .prepaint(window, cx);
        SelectableTextDocumentPrepaintState { hitbox, owner }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let window_id = window.window_handle().window_id();
        begin_selectable_text_document_frame(
            window_id,
            self.document_id.clone(),
            prepaint.owner.clone(),
            &prepaint.hitbox,
            cx,
        );
        bind_selectable_text_document_events(
            self.document_id.clone(),
            prepaint.hitbox.clone(),
            self.autoscroll_handler.clone(),
            window,
        );
        self.child
            .as_mut()
            .expect("selectable text document child must exist")
            .paint(window, cx);
        finish_selectable_text_document_frame(
            window_id,
            &self.document_id,
            self.autoscroll_handler.as_ref(),
            window,
            cx,
        );
    }
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
        Some(gpui::accesskit::Role::Label)
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        node.set_value(self.text.to_string());
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.styled_text
            .request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text
            .prepaint(None, inspector_id, bounds, state, window, cx);
        window.with_optional_element_state::<SelectableTextState, _>(
            global_id,
            |selectable_state, window| {
                let selectable_state = selectable_state.map(|state| {
                    let mut state = state.unwrap_or_default();
                    state.prepare_for_text(&self.text);
                    state
                });
                (
                    window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    selectable_state,
                )
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let global_id = global_id.expect("selectable text must have an element id");
        let text_layout = self.styled_text.layout().clone();
        ensure_selectable_text_clipboard(cx);
        window.with_element_state::<SelectableTextState, _>(global_id, |state, window| {
            let state = state.unwrap_or_default();
            if self.document_position.is_none()
                && state.mouse_down_index.get().is_some()
                && SELECTED_TEXT_ID.load(Ordering::Relaxed) == state.selection_id
            {
                window.capture_pointer(hitbox.id);
            }
            if let Some(position) = self.document_position.as_ref() {
                register_selectable_text_document_fragment(
                    window.window_handle().window_id(),
                    position.clone(),
                    self.text.clone(),
                    SelectableTextVisibleFragment {
                        bounds,
                        text_layout: text_layout.clone(),
                    },
                    cx,
                );
            }
            self.bind_hover_cursor(hitbox, &text_layout, window);
            self.bind_mouse_down(hitbox, &text_layout, &state, window);
            self.bind_mouse_move(&text_layout, &state, window);
            self.bind_mouse_up(&text_layout, &state, window);
            self.paint_selection(&text_layout, &state, window, cx);
            ((), state)
        });
        self.styled_text
            .paint(None, inspector_id, bounds, state, &mut (), window, cx);
    }
}
