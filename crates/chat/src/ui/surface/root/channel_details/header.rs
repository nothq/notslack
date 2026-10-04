use super::super::super::{
    div, px, rgb, slack_icon, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use super::slack_channel_details_action_key;
use crate::ui::{AppearanceMode, SlackWorkspace};
use gpui::{Div, Role};

const SLACK_CHANNEL_DETAILS_HEADER_HEIGHT: f32 = 134.0;

impl SurfaceState {
    pub(super) fn render_slack_channel_details_header(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_CHANNEL_DETAILS_HEADER_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .flex_col()
            .child(self.render_slack_channel_details_title_row(workspace, cx))
            .child(self.render_slack_channel_details_star_row(cx))
            .child(self.render_slack_channel_details_tabs(workspace, cx))
    }

    fn render_slack_channel_details_title_row(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(62.0))
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
            .child(self.render_slack_channel_details_close(cx))
    }

    fn render_slack_channel_details_close(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-channel-details-close")
            .role(Role::Button)
            .aria_label("Close channel details")
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(|style| {
                style.bg(rgb(if self.appearance_mode == AppearanceMode::Dark {
                    0x2a2d31
                } else {
                    0xf1f2f3
                }))
            })
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Close,
                palette.main_secondary_text,
                22.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.close_slack_channel_details(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_channel_details_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.close_slack_channel_details(cx);
                }
            }))
    }

    fn render_slack_channel_details_star_row(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(30.0))
            .flex_none()
            .px(px(28.0))
            .flex()
            .items_start()
            .when(
                self.slack_workspace_api_capabilities.mutate_stars
                    && self.slack_pending_channel_star.is_none(),
                |this| this.child(self.render_slack_channel_details_star_button(cx)),
            )
    }

    fn render_slack_channel_details_tabs(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(42.0))
            .flex_none()
            .px(px(28.0))
            .flex()
            .items_end()
            .gap(px(36.0))
            .child(
                div()
                    .h(px(36.0))
                    .border_b_2()
                    .border_color(rgb(palette.main_text))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("About"),
            )
            .when(
                self.slack_workspace_api_capabilities
                    .load_conversation_members,
                |this| this.child(self.render_slack_channel_details_members_tab(workspace, cx)),
            )
    }

    fn render_slack_channel_details_members_tab(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-channel-details-members-tab")
            .role(Role::Tab)
            .aria_label("Channel members")
            .focusable()
            .tab_stop(true)
            .h(px(36.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(5.0))
            .text_size(px(14.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(palette.main_secondary_text))
            .hover(|style| style.text_color(rgb(palette.main_text)))
            .child("Members")
            .when_some(workspace.member_count, |tab, count| {
                tab.child(count.to_string())
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_channel_details_members(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_channel_details_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_channel_details_members(cx);
                }
            }))
    }

    fn render_slack_channel_details_star_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let starred = self.slack_active_channel_is_starred();
        let action_label = if starred {
            "Remove star"
        } else {
            "Star channel"
        };
        div()
            .id("slack-channel-details-star-control")
            .role(Role::Group)
            .aria_label("Channel star controls")
            .w(px(56.0))
            .h(px(28.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .overflow_hidden()
            .flex()
            .child(self.render_slack_channel_details_star_action(action_label, starred, cx))
            .child(self.render_slack_channel_details_star_chevron(action_label, cx))
            .into_any_element()
    }

    fn render_slack_channel_details_star_action(
        &self,
        action_label: &'static str,
        starred: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-channel-details-star")
            .role(Role::Button)
            .aria_label(action_label)
            .focusable()
            .tab_stop(true)
            .w(px(35.0))
            .h_full()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| {
                style.bg(rgb(if self.appearance_mode == AppearanceMode::Dark {
                    0x2a2d31
                } else {
                    0xf1f2f3
                }))
            })
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .child(slack_icon(
                SlackShellIcon::HeaderStar,
                if starred { 0xe3a300 } else { palette.main_text },
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_active_channel_star(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_channel_details_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_active_channel_star(cx);
                }
            }))
    }

    fn render_slack_channel_details_star_chevron(
        &self,
        action_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-channel-details-star-chevron")
            .role(Role::Button)
            .aria_label(action_label)
            .focusable()
            .tab_stop(true)
            .w(px(20.0))
            .h_full()
            .border_l_1()
            .border_color(rgb(palette.main_border))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| {
                style.bg(rgb(if self.appearance_mode == AppearanceMode::Dark {
                    0x2a2d31
                } else {
                    0xf1f2f3
                }))
            })
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .child(slack_icon(
                SlackShellIcon::ChevronDown,
                palette.main_text,
                16.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_active_channel_star(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_channel_details_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_active_channel_star(cx);
                }
            }))
    }
}
