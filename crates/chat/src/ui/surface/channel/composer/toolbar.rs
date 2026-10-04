use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, ParentElement, SlackComposerFormatAction, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState, SLACK_COMPOSER_TOOLBAR_HEIGHT,
};
use gpui::{KeyDownEvent, Role};

mod formatting;
mod send_controls;

impl SurfaceState {
    pub(crate) fn render_slack_composer_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-main-composer-actions")
            .role(Role::Toolbar)
            .aria_label("Composer actions")
            .h(px(SLACK_COMPOSER_TOOLBAR_HEIGHT))
            .pl(px(8.0))
            .pr(px(6.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_composer_toolbar_actions(cx))
            .child(self.render_slack_send_controls(cx))
    }

    fn render_slack_composer_toolbar_actions(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let video_clip_available = self.can_prepare_slack_video_clip_capture();
        let audio_clip_available = self.can_start_slack_audio_clip_capture();
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(self.render_slack_attachment_picker_button(cx))
            .child(self.render_slack_format_toggle(cx))
            .child(self.render_slack_emoji_button(cx))
            .child(self.render_slack_mention_button(cx))
            .when(video_clip_available || audio_clip_available, |this| {
                this.child(
                    div()
                        .mx(px(2.0))
                        .w(px(1.0))
                        .h(px(20.0))
                        .bg(rgb(palette.composer_border)),
                )
                .when(video_clip_available, |this| {
                    this.child(self.render_slack_video_clip_button(cx))
                })
                .when(audio_clip_available, |this| {
                    this.child(self.render_slack_audio_clip_button(cx))
                })
            })
            .when(self.slack_active_scheduled_edit.is_some(), |this| {
                this.child(self.render_slack_scheduled_edit_indicator(cx))
            })
    }

    fn render_slack_scheduled_edit_indicator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pending = self.slack_schedule_pending.is_some();
        div()
            .ml(px(6.0))
            .h(px(26.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .bg(alpha(0x1d9bd1, 0.14))
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(12.0))
            .text_color(rgb(0xd1d2d3))
            .child(if pending {
                "Saving scheduled message…"
            } else {
                "Editing scheduled message"
            })
            .when(!pending, |this| {
                this.child(
                    div()
                        .id("slack-composer-cancel-scheduled-edit")
                        .role(Role::Button)
                        .aria_label("Cancel editing scheduled message")
                        .focusable()
                        .tab_stop(true)
                        .cursor_pointer()
                        .text_color(rgb(0x1d9bd1))
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cancel_slack_scheduled_edit(cx);
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                                || event.keystroke.modifiers.modified()
                            {
                                return;
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                            this.cancel_slack_scheduled_edit(cx);
                        })),
                )
            })
    }

    fn render_slack_mention_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        if !self.can_mutate_current_slack_send_draft() {
            return div()
                .id("slack-composer-mention")
                .role(Role::Button)
                .aria_label("Mention someone")
                .size(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(palette.send_disabled_icon))
                .opacity(0.45)
                .child(slack_icon(
                    SlackShellIcon::Mention,
                    palette.send_disabled_icon,
                    18.0,
                    cx,
                ))
                .into_any_element();
        }
        div()
            .id("slack-composer-mention")
            .role(Role::Button)
            .aria_label("Mention someone")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_slack_mention_picker(cx);
                this.focus_slack_composer(cx);
                let focus = this.slack_composer_input.read(cx).focus_handle_clone();
                window.focus(&focus, cx);
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Mention,
                palette.composer_icon,
                18.0,
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_attachment_picker_button(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.has_current_slack_send_target()
            || !self.slack_workspace_api_capabilities.upload_files
            || self.slack_schedule_pending.is_some()
        {
            let palette = slack_palette(self.appearance_mode);
            return div()
                .id("slack-composer-attach")
                .role(Role::Button)
                .aria_label("Attach")
                .size(px(28.0))
                .rounded(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.45)
                .child(slack_icon(
                    SlackShellIcon::Plus,
                    palette.send_disabled_icon,
                    18.0,
                    cx,
                ))
                .into_any_element();
        }
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-attach")
            .role(Role::Button)
            .aria_label("Attach")
            .focusable()
            .tab_stop(true)
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(14.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .bg(alpha(palette.composer_icon_bg, 0.06))
            .on_click(cx.listener(|this, _, window, cx| {
                this.prompt_for_slack_attachment_files(window, cx);
            }))
            .child(slack_icon(
                SlackShellIcon::Plus,
                palette.composer_icon,
                18.0,
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_emoji_button(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.can_mutate_current_slack_send_draft() {
            let palette = slack_palette(self.appearance_mode);
            return div()
                .id("slack-composer-emoji")
                .role(Role::Button)
                .aria_label("Emoji")
                .size(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.45)
                .child(slack_icon(
                    SlackShellIcon::Emoji,
                    palette.send_disabled_icon,
                    18.0,
                    cx,
                ))
                .into_any_element();
        }
        self.render_slack_composer_icon_button(SlackShellIcon::Emoji, false, cx)
            .id("slack-composer-emoji")
            .role(Role::Button)
            .aria_label("Emoji")
            .focusable()
            .tab_stop(true)
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_emoji_picker(cx);
            }))
            .into_any_element()
    }

    fn render_slack_composer_icon_button(
        &self,
        icon: SlackShellIcon,
        active: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .bg(if active {
                alpha(palette.composer_focused_border, 0.35)
            } else {
                alpha(0xffffff, 0.0)
            })
            .child(slack_icon(
                icon,
                if active {
                    0xffffff
                } else {
                    palette.composer_icon
                },
                18.0,
                cx,
            ))
    }
}
