use crate::ui::surface::{
    slack_icon, slack_palette, SlackComposerDraftKey, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    alpha, div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui::Role;

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_thread_attachment_picker_button(
        &self,
        draft_key: SlackComposerDraftKey,
        context_id: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let enabled = enabled && self.slack_workspace_api_capabilities.upload_files;
        let button = div()
            .id(format!("slack-thread-attach-{context_id}"))
            .role(Role::Button)
            .aria_label("Attach files to thread reply")
            .size(px(28.0))
            .rounded_full()
            .bg(if enabled {
                alpha(palette.composer_icon_bg, 0.06)
            } else {
                alpha(palette.composer_icon_bg, 0.0)
            })
            .flex()
            .items_center()
            .justify_center()
            .opacity(if enabled { 1.0 } else { 0.45 })
            .child(slack_icon(
                SlackShellIcon::Plus,
                if enabled {
                    palette.composer_icon
                } else {
                    palette.send_disabled_icon
                },
                18.0,
                cx,
            ));
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(alpha(palette.composer_icon_bg, 0.08)))
                .focus_visible(|style| style.bg(alpha(palette.composer_icon_bg, 0.08)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.prompt_for_slack_thread_attachment_files(draft_key.clone(), window, cx);
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }
}
