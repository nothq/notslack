use super::{
    alpha, div, px, rgb, slack_icon, slack_link_dialog_action_key, slack_link_dialog_text,
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_LINK_DIALOG_HORIZONTAL_PADDING,
};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_composer_link_header(
        &self,
        title: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = slack_link_dialog_text(self.appearance_mode);
        div()
            .absolute()
            .top(px(20.0))
            .left(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
            .right(px(20.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .line_height(px(28.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(text))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .id("slack-composer-link-close")
                    .role(Role::Button)
                    .aria_label("Close link dialog")
                    .focusable()
                    .tab_stop(true)
                    .size(px(36.0))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(text, 0.08)))
                    .focus_visible(|style| style.bg(alpha(text, 0.08)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_composer_link_dialog(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_link_dialog_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.close_slack_composer_link_dialog(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(SlackShellIcon::Close, text, 24.0, cx)),
            )
            .into_any_element()
    }
}
