use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    accesskit::ActionData, div, point, px, relative, AccessibleAction, App, Bounds, Context,
    CursorStyle, Div, Element, ElementId, ElementInputHandler, Entity, FocusHandle, Focusable,
    GlobalElementId, InteractiveElement, IntoElement, LayoutId, MouseButton, PaintQuad,
    ParentElement, Pixels, Render, Role, StatefulInteractiveElement, Style, Styled, TextAlign,
    Window,
};

use super::a11y::text_input_a11y_state;
use super::layout::{
    cursor_quad, highlight_background_quads, selection_quads, TextLayoutLine, TextViewport,
};
use super::{TextInput, TextInputMode};

impl TextInput {
    fn sync_render_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let focused = self.focus_handle.is_focused(window);
        let request_focus = std::mem::take(&mut self.request_focus);
        if request_focus && !focused && !self.disabled {
            window.focus(&self.focus_handle, cx);
        }
        let focused = self.focus_handle.is_focused(window);
        self.focused_last_render = focused;
        if focused {
            self.schedule_blink(cx);
        } else {
            self.caret_visible = true;
            self.blink_scheduled = false;
        }
        focused
    }

    fn render_height(&self) -> Pixels {
        match self.mode {
            TextInputMode::SingleLine => self.style.height.max(self.style.min_height),
            TextInputMode::Multiline { .. } => (self.style.padding_y * 2.0
                + self.style.line_height * self.measured_visible_line_count())
            .max(self.style.min_height),
        }
    }

    fn render_root(&self, focused: bool, cx: &mut Context<Self>) -> Div {
        let style = self.style.clone();
        let border = if focused {
            style.focused_border
        } else {
            style.border
        };
        let root = div()
            .h(self.render_height())
            .min_w(px(0.0))
            .rounded(style.radius)
            .border_color(border)
            .bg(style.background)
            .px(style.padding_x)
            .py(style.padding_y)
            .flex()
            .items_start()
            .overflow_hidden()
            .text_size(style.font_size)
            .line_height(style.line_height)
            .text_color(style.text)
            .track_focus(&self.focus_handle(cx))
            .cursor(if self.hovered_atom.is_some() {
                CursorStyle::PointingHand
            } else {
                CursorStyle::IBeam
            })
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up_out))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(self.bordered, |root| root.border_1());
        root.when(self.fill_width, |root| root.w_full())
            .when_some(style.font_family, |root, font_family| {
                root.font_family(font_family)
            })
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_layout_for_last_width(window, cx);
        let focused = self.sync_render_focus(window, cx);
        let accessibility = self.accessibility.clone();
        let root_id = accessibility.as_ref().map_or_else(
            || self.root_id.clone(),
            |accessibility| accessibility.id.clone(),
        );
        self.render_root(focused, cx)
            .id(root_id)
            .child(TextElement { input: cx.entity() })
            .when_some(accessibility, |root, accessibility| {
                let role = if self.obscured {
                    Role::PasswordInput
                } else {
                    match self.mode {
                        TextInputMode::SingleLine => Role::TextInput,
                        TextInputMode::Multiline { .. } => Role::MultilineTextInput,
                    }
                };
                let (a11y_value, a11y_text_runs) =
                    text_input_a11y_state(self, accessibility.id, window, cx);
                let input = cx.entity().downgrade();
                let mut root = root
                    .role(role)
                    .aria_label(accessibility.label)
                    .aria_value(a11y_value)
                    .aria_placeholder(self.placeholder.clone())
                    .a11y_synthetic_children(a11y_text_runs);
                if !self.disabled {
                    root =
                        root.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                            let Some(ActionData::Value(value)) = data else {
                                return;
                            };
                            input
                                .update(cx, |input, cx| {
                                    input.set_text_from_accessibility(
                                        value.to_string(),
                                        window,
                                        cx,
                                    );
                                })
                                .ok();
                        });
                }
                root
            })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Arc<[TextLayoutLine]>,
    highlight_backgrounds: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll_x: Pixels,
    scroll_y: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.input.update(cx, |input, cx| {
            input.last_bounds = Some(bounds);
            input.refresh_layout(bounds.size.width, window, cx);
            let lines = input
                .layout_cache
                .as_ref()
                .map_or_else(Arc::default, |cache| cache.lines.clone());
            let line_height = input.style.line_height;
            let viewport = TextViewport {
                bounds,
                line_height,
                scroll_x: input.scroll_x,
                scroll_y: input.scroll_y,
            };
            let render_placeholder = input.content.is_empty();
            let selection = if render_placeholder {
                Vec::new()
            } else {
                selection_quads(
                    &input.selected_range,
                    &lines,
                    viewport,
                    input.style.selection,
                )
            };
            let highlight_backgrounds = if render_placeholder {
                Vec::new()
            } else {
                highlight_background_quads(&input.highlights, &lines, viewport)
            };
            let cursor = if !input.selected_range.is_empty() {
                None
            } else {
                cursor_quad(input.cursor_offset(), &lines, viewport, input.style.caret)
            };
            PrepaintState {
                lines,
                highlight_backgrounds,
                cursor,
                selection,
                scroll_x: input.scroll_x,
                scroll_y: input.scroll_y,
            }
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for background in prepaint.highlight_backgrounds.drain(..) {
            window.paint_quad(background);
        }
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let line_height = self.input.read(cx).style.line_height;
        let mut line_top = bounds.top() - prepaint.scroll_y;
        for line in prepaint.lines.iter() {
            let line_bottom = line_top + line.line.size(line_height).height;
            if line_bottom > bounds.top() && line_top < bounds.bottom() {
                line.line
                    .paint(
                        point(bounds.left() - prepaint.scroll_x, line_top),
                        line_height,
                        TextAlign::Left,
                        Some(bounds),
                        window,
                        cx,
                    )
                    .ok();
            }
            line_top = line_bottom;
        }
        let should_paint_cursor =
            self.input.read(cx).caret_visible && focus_handle.is_focused(window);
        if should_paint_cursor {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }
    }
}
