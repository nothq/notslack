use std::time::Instant;

use app_model::SurfaceRoot as AppSurfaceRoot;

use super::{AnyElement, App, KeyDownEvent, SurfaceFrame, SurfaceRoot, Window};

impl AppSurfaceRoot for SurfaceRoot {
    fn set_surface_frame(&mut self, frame: SurfaceFrame) {
        self.set_theme(frame.theme);
        self.set_preview_width(frame.preview_width);
        self.set_viewport_height(frame.viewport_height);
    }

    fn activate_surface(&mut self, frame: SurfaceFrame, cx: &mut App) {
        self.activation_started_at = frame.active.then(Instant::now);
        self.set_surface_frame(frame);
        self.prepare_frame(frame.active, cx);
    }

    fn render_surface_preview(&mut self, frame: SurfaceFrame, cx: &mut App) -> Option<AnyElement> {
        self.set_surface_frame(frame);
        frame.active.then(|| self.render_preview(cx))
    }

    fn render_surface_standalone(
        &mut self,
        frame: SurfaceFrame,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.set_surface_frame(frame);
        Some(self.render_standalone(cx))
    }

    fn allows_workspace_shell_key_down(&mut self, window: &Window, cx: &mut App) -> bool {
        let Some(surface) = self.current_surface() else {
            return true;
        };
        !surface.read(cx).slack_text_entry_focused(window, cx)
    }

    fn handle_workspace_key_down(&mut self, event: &KeyDownEvent, cx: &mut App) -> bool {
        self.handle_key_down(event, cx)
    }

    fn render_surface(&mut self, _window: &mut Window, cx: &mut gpui::App) -> AnyElement {
        self.render_standalone(cx)
    }
}
