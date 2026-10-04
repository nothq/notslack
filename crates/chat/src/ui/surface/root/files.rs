use super::super::{
    div, px, rgb, Context, Div, FluentBuilder, ParentElement, Styled, SurfaceState,
};

mod browser;
mod content;
mod menus;
mod sidebar;

const SLACK_FILES_HEADER_HEIGHT: f32 = 49.0;
const SLACK_FILES_ROW_HEIGHT: f32 = 65.0;

impl SurfaceState {
    pub(super) fn render_slack_files_surface(&self, cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .bg(rgb(0x1b1d21))
            .when(self.preview_width >= 1000.0, |this| {
                this.child(self.render_slack_files_sidebar(cx))
            })
            .child(self.render_slack_files_browser(cx))
    }
}

fn slack_files_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
