use crate::ui::surface::{
    div, px, rgb, Context, Div, InteractiveElement, ParentElement, SlackAttachmentSelection,
    SlackMediaHostId, StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{Role, Stateful};

impl SurfaceState {
    pub(super) fn render_slack_media_retry_button(
        &self,
        selection: &SlackAttachmentSelection,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let retry_selection = selection.clone();
        let retry_keyboard_selection = selection.clone();
        let retry_host = SlackMediaHostId::lightbox(selection.attachment_id.clone());
        let retry_keyboard_host = retry_host.clone();
        div()
            .id(format!("slack-media-retry-{}", selection.attachment_id))
            .role(Role::Button)
            .aria_label(format!("Retry playback for {}", selection.attachment.title))
            .focusable()
            .tab_stop(true)
            .rounded(px(7.0))
            .bg(rgb(0x36c5f0))
            .px(px(12.0))
            .py(px(7.0))
            .cursor_pointer()
            .text_size(px(13.0))
            .text_color(rgb(0x071014))
            .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.retry_slack_media_playback(&retry_selection, retry_host.clone(), cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if slack_media_retry_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.retry_slack_media_playback(
                            &retry_keyboard_selection,
                            retry_keyboard_host.clone(),
                            cx,
                        );
                    }
                }),
            )
            .child("Retry")
    }
}

fn slack_media_retry_key(event: &gpui::KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
