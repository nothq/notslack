use crate::{
    model::ChatSurfaceEvent,
    ui::surface::{
        alpha, div, px, AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent,
        MouseButton, Render, StatefulInteractiveElement, Styled, SurfaceState, Window,
    },
};
use gpui::{AppContext, DragMoveEvent, Orientation, Role};

const SLACK_SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 6.0;
const SLACK_SIDEBAR_RESIZE_KEY_STEP: f32 = 10.0;
const SLACK_SIDEBAR_RESIZE_KEY_FAST_STEP: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DraggedSlackSidebarResize;

impl Render for DraggedSlackSidebarResize {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl SurfaceState {
    pub(super) fn render_slack_sidebar_resize_handle(&self, cx: &mut Context<Self>) -> AnyElement {
        let width = self.slack_sidebar_width();
        let (min_width, max_width) = self.slack_sidebar_width_bounds();
        let slider_position = ((width - min_width) / SLACK_SIDEBAR_RESIZE_KEY_STEP).floor() + 1.0;
        let focus_handle = self.slack_sidebar_resize_focus_handle.clone();
        div()
            .id("slack-sidebar-resize-handle")
            .role(Role::Splitter)
            .aria_label("Sidebar width")
            .aria_orientation(Orientation::Vertical)
            .aria_numeric_value(f64::from(width))
            .aria_value(format!("{slider_position:.0}"))
            .aria_numeric_value_step(f64::from(SLACK_SIDEBAR_RESIZE_KEY_STEP))
            .aria_min_numeric_value(f64::from(min_width))
            .aria_max_numeric_value(f64::from(max_width))
            .absolute()
            .left(px(width - SLACK_SIDEBAR_RESIZE_HANDLE_WIDTH * 0.5))
            .top(px(0.0))
            .h_full()
            .w(px(SLACK_SIDEBAR_RESIZE_HANDLE_WIDTH))
            .focusable()
            .track_focus(&self.slack_sidebar_resize_focus_handle)
            .tab_stop(true)
            .cursor_col_resize()
            .hover(|style| style.bg(alpha(0x1264a3, 0.34)))
            .focus_visible(|style| style.bg(alpha(0x1264a3, 0.56)))
            .block_mouse_except_scroll()
            .on_drag(DraggedSlackSidebarResize, |dragged, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| *dragged)
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_handle, cx);
                cx.stop_propagation();
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.handle_slack_sidebar_resize_key(event, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .occlude()
            .into_any_element()
    }

    pub(super) fn handle_slack_sidebar_resize_drag_move(
        &mut self,
        event: &DragMoveEvent<DraggedSlackSidebarResize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let width =
            (event.event.position.x - event.bounds.left()).as_f32() - self.slack_rail_width();
        self.resize_slack_sidebar_to_width(width, cx);
    }

    pub(super) fn commit_slack_sidebar_width(&self, cx: &mut Context<Self>) {
        cx.emit(ChatSurfaceEvent::SlackSidebarRatioCommitted {
            ratio_basis_points: (self.slack_sidebar_preferred_ratio * 10_000.0).round() as u16,
        });
    }

    fn handle_slack_sidebar_resize_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform || modifiers.control || modifiers.alt || modifiers.function {
            return false;
        }
        let step = if modifiers.shift {
            SLACK_SIDEBAR_RESIZE_KEY_FAST_STEP
        } else {
            SLACK_SIDEBAR_RESIZE_KEY_STEP
        };
        let (min_width, max_width) = self.slack_sidebar_width_bounds();
        let width = self.slack_sidebar_width();
        let next_width = match event.keystroke.key.as_str() {
            "left" | "arrowleft" => width - step,
            "right" | "arrowright" => width + step,
            "home" => min_width,
            "end" => max_width,
            _ => return false,
        };
        if self.resize_slack_sidebar_to_width(next_width, cx) {
            self.commit_slack_sidebar_width(cx);
        }
        true
    }
}
