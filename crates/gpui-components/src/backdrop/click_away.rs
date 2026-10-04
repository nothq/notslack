use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    canvas, App, Bounds, Context, Div, ElementId, InteractiveElement, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Pixels, Point, Stateful, Styled, Window,
};

use super::blocking_backdrop;

type ClickAwayMemberBounds = Rc<Cell<Option<Bounds<Pixels>>>>;

/// A geometric boundary shared by one dismissible root and its out-of-bounds members.
#[derive(Clone, Default)]
pub struct ClickAwayBoundary {
    members: Rc<RefCell<Vec<ClickAwayMemberBounds>>>,
}

impl ClickAwayBoundary {
    /// Creates an empty geometric click-away boundary for one rendered surface group.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a surface as part of this boundary and consumes primary gestures inside it.
    ///
    /// Register every deferred or anchored child that can paint outside the dismissible root.
    pub fn member(&self, surface: Div, cx: &mut App) -> Div {
        blocking_backdrop(self.register(surface), cx)
    }

    /// Registers the dismissible root and derives its stable identity from the caller.
    #[track_caller]
    pub fn dismissible<V: 'static>(
        &self,
        surface: Div,
        handler: impl Fn(&mut V, &MouseUpEvent, &mut Window, &mut Context<V>) + 'static,
        cx: &mut Context<V>,
    ) -> Stateful<Div> {
        let handler = cx.listener(move |this, event, window, cx| handler(this, event, window, cx));
        self.dismissible_with_id(
            surface,
            ElementId::CodeLocation(*core::panic::Location::caller()),
            handler,
            cx,
        )
    }

    /// Registers a repeated or dynamically identified dismissible root.
    pub fn dismissible_with_key<V: 'static>(
        &self,
        surface: Div,
        key: ElementId,
        handler: impl Fn(&mut V, &MouseUpEvent, &mut Window, &mut Context<V>) + 'static,
        cx: &mut Context<V>,
    ) -> Stateful<Div> {
        let handler = cx.listener(move |this, event, window, cx| handler(this, event, window, cx));
        self.dismissible_with_id(surface, key, handler, cx)
    }

    /// Registers a dismissible root using an app callback with no entity-state access.
    #[track_caller]
    pub fn dismissible_with_handler(
        &self,
        surface: Div,
        handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
        cx: &mut App,
    ) -> Stateful<Div> {
        self.dismissible_with_id(
            surface,
            ElementId::CodeLocation(*core::panic::Location::caller()),
            handler,
            cx,
        )
    }

    fn register(&self, surface: Div) -> Div {
        let member_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
        self.members.borrow_mut().push(Rc::clone(&member_bounds));
        surface.child(
            canvas(
                move |bounds, window, _| {
                    member_bounds.set(Some(bounds.intersect(&window.content_mask().bounds)));
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    }

    fn contains(&self, position: Point<Pixels>) -> bool {
        self.members.borrow().iter().any(|member_bounds| {
            member_bounds
                .get()
                .is_some_and(|bounds| bounds.contains(&position))
        })
    }

    fn dismissible_with_id(
        &self,
        surface: Div,
        id: ElementId,
        handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
        cx: &mut App,
    ) -> Stateful<Div> {
        let down_boundary = self.clone();
        let up_boundary = self.clone();
        blocking_backdrop(self.register(surface), cx)
            .id(id)
            .on_mouse_down_out(move |event: &MouseDownEvent, window, cx| {
                if event.button != MouseButton::Left || down_boundary.contains(event.position) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_mouse_up_out(
                MouseButton::Left,
                move |event: &MouseUpEvent, window, cx| {
                    if up_boundary.contains(event.position) {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    handler(event, window, cx);
                    cx.stop_propagation();
                },
            )
    }
}

/// Occludes a bounded surface and consumes primary gestures inside and outside it.
///
/// This is for anchored surfaces that cannot own a real viewport-sized backdrop.
/// An outside release is always consumed and dismissed, even if GPUI redraws
/// between press and release, so an underlying raw mouse-up handler cannot receive it.
#[track_caller]
pub fn dismissible_click_away<V: 'static>(
    surface: Div,
    handler: impl Fn(&mut V, &MouseUpEvent, &mut Window, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    ClickAwayBoundary::new().dismissible(surface, handler, cx)
}

/// Applies click-away dismissal to a repeated or dynamically identified surface.
pub fn dismissible_click_away_with_key<V: 'static>(
    surface: Div,
    key: ElementId,
    handler: impl Fn(&mut V, &MouseUpEvent, &mut Window, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    ClickAwayBoundary::new().dismissible_with_key(surface, key, handler, cx)
}
