use gpui::{KeyDownEvent, Role};

use super::{
    alpha, div, px, slack_icon, slack_palette, Context, InteractiveElement, IntoElement,
    ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};

impl SurfaceState {
    pub(super) fn render_slack_video_clip_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-video-clip")
            .role(Role::Button)
            .aria_label("Record video clip")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Video,
                palette.composer_icon,
                18.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.prepare_slack_video_clip_from_control(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified()
                    || !matches!(event.keystroke.key.as_str(), "enter" | "space")
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.prepare_slack_video_clip_from_control(cx);
            }))
    }

    fn prepare_slack_video_clip_from_control(&mut self, cx: &mut Context<Self>) {
        if let Err(diagnostic) = self.prepare_slack_video_clip_capture(cx) {
            self.slack_error = Some(diagnostic);
            cx.notify();
        }
    }
}
