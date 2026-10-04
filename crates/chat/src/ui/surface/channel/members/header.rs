use super::{
    div, px, rgb, slack_icon, slack_members_action_key, slack_palette, Context, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use crate::ui::SlackWorkspace;
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_members_dialog_header(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let member_count = self
            .slack_effective_member_count(workspace)
            .unwrap_or(self.slack_members_rows.len() as u32);
        div()
            .h(px(134.0))
            .flex_none()
            .border_b_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(58.0))
                    .flex_none()
                    .px(px(28.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(22.0))
                            .line_height(px(28.0))
                            .font_weight(FontWeight::BLACK)
                            .text_color(rgb(palette.main_text))
                            .child(format!("# {}", workspace.channel_name)),
                    )
                    .child(self.render_slack_members_close_button(palette.main_secondary_text, cx)),
            )
            .child(self.render_slack_members_tab(member_count, palette.main_text))
    }

    fn render_slack_members_close_button(
        &self,
        icon_color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-channel-members-close")
            .role(Role::Button)
            .aria_label("Close channel members")
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(|style| {
                style.bg(rgb(
                    if self.appearance_mode == crate::ui::AppearanceMode::Dark {
                        0x2a2d31
                    } else {
                        0xf1f2f3
                    },
                ))
            })
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(SlackShellIcon::Close, icon_color, 22.0, cx))
            .on_click(cx.listener(|this, _, _, cx| {
                this.close_slack_members_panel(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_members_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.close_slack_members_panel(cx);
                }
            }))
    }

    fn render_slack_members_tab(&self, member_count: u32, text_color: u32) -> impl IntoElement {
        div()
            .h(px(76.0))
            .flex_none()
            .pt(px(40.0))
            .px(px(28.0))
            .flex()
            .items_end()
            .child(
                div()
                    .h(px(36.0))
                    .border_b_2()
                    .border_color(rgb(text_color))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(rgb(text_color))
                    .child(div().font_weight(FontWeight::BOLD).child("Members"))
                    .child(member_count.to_string()),
            )
    }
}
