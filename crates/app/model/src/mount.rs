use gpui::{AnyElement, App, KeyDownEvent, KeyUpEvent, WeakFocusHandle, Window};

use crate::{SurfaceTheme, Viewport};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfacePlacement {
    #[default]
    Primary,
    Sidebar,
}

#[derive(Clone, Copy)]
pub struct SurfaceFrame {
    pub theme: SurfaceTheme,
    pub viewport: Viewport,
    pub preview_width: f32,
    pub viewport_height: f32,
    pub active: bool,
    pub placement: SurfacePlacement,
    pub window_controls_visible: bool,
    pub chrome_top_inset: f32,
}

impl Default for SurfaceFrame {
    fn default() -> Self {
        let viewport = Viewport::default();
        Self {
            theme: SurfaceTheme::default(),
            viewport,
            preview_width: 360.0,
            viewport_height: viewport.app_height(),
            active: true,
            placement: SurfacePlacement::Primary,
            window_controls_visible: false,
            chrome_top_inset: viewport.chrome_top_inset(),
        }
    }
}

/// A cheaply mounted surface whose resources start on first use: activation,
/// rendering a preview, or a surface command. Preparing an inactive frame must
/// leave an unused surface dormant. App-wide services such as notifications
/// belong to the host's activation lifecycle.
pub trait SurfaceRoot: 'static {
    fn render_surface(&mut self, window: &mut Window, cx: &mut App) -> AnyElement;

    fn set_surface_focus_context(&mut self, _focus: WeakFocusHandle, _cx: &mut App) {}

    fn suppresses_window_controls(&self) -> bool {
        false
    }

    fn set_surface_frame(&mut self, _frame: SurfaceFrame) {}

    fn activate_surface(&mut self, frame: SurfaceFrame, _cx: &mut App) {
        self.set_surface_frame(frame);
    }

    fn deactivate_surface(&mut self, _cx: &mut App) {}

    fn prepare_surface_frame(&mut self, frame: SurfaceFrame, cx: &mut App) {
        self.activate_surface(frame, cx);
    }

    fn prepare_surface_window_frame(
        &mut self,
        frame: SurfaceFrame,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.prepare_surface_frame(frame, cx);
    }

    fn render_surface_preview(&mut self, frame: SurfaceFrame, cx: &mut App) -> Option<AnyElement> {
        self.activate_surface(frame, cx);
        None
    }

    fn render_surface_standalone(
        &mut self,
        frame: SurfaceFrame,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.render_surface_preview(frame, cx)
    }

    fn allows_workspace_shell_key_down(&mut self, _window: &Window, _cx: &mut App) -> bool {
        true
    }

    fn allows_workspace_shell_shortcuts(&mut self, _cx: &mut App) -> bool {
        true
    }

    fn handle_surface_key_down(&mut self, _event: &KeyDownEvent, _cx: &mut App) -> bool {
        false
    }

    fn handle_surface_key_up(&mut self, _event: &KeyUpEvent, _cx: &mut App) -> bool {
        false
    }

    fn handle_workspace_key_down(&mut self, _event: &KeyDownEvent, _cx: &mut App) -> bool {
        false
    }
}
