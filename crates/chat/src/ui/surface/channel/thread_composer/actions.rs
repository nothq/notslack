use crate::ui::surface::{
    slack_icon, slack_palette, SlackPalette, SlackReplyComposerTarget, SlackShellIcon,
    SlackThreadPanelState, SurfaceState,
};
use crate::ui::{
    alpha, div, px, rgb, AnyElement, Context, Div, FluentBuilder, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{Role, SharedString, Toggled};

use super::SLACK_THREAD_COMPOSER_BROADCAST_HEIGHT;

impl SurfaceState {
    pub(super) fn render_slack_thread_toolbar_actions(
        &self,
        panel: &SlackThreadPanelState,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let target = SlackReplyComposerTarget::ThreadPanel {
            panel_generation: panel.generation,
            parent_message_id: panel.parent_message_id.clone().into(),
            draft_key: panel.reply_draft_key.clone(),
        };
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(self.render_slack_thread_attachment_picker_button(
                panel.reply_draft_key.clone(),
                &panel.parent_message_id,
                enabled,
                cx,
            ))
            .child(self.render_slack_reply_toolbar_aux_actions(&target, enabled, cx))
            .when(
                self.slack_thread_reply_is_pending(&panel.reply_draft_key),
                |this| {
                    this.child(
                        div()
                            .ml(px(4.0))
                            .text_size(px(12.0))
                            .text_color(
                                rgb(slack_palette(self.appearance_mode).main_secondary_text),
                            )
                            .child("Sending…"),
                    )
                },
            )
    }

    pub(in crate::ui::surface) fn render_slack_reply_toolbar_aux_actions(
        &self,
        target: &SlackReplyComposerTarget,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(self.render_slack_thread_format_toggle(
                target,
                self.slack_reply_formatting_enabled(target),
                enabled,
                cx,
            ))
            .child(self.render_slack_thread_emoji_button(target, enabled, cx))
            .child(self.render_slack_thread_mention_button(target, enabled, cx))
    }

    fn render_slack_thread_format_toggle(
        &self,
        target: &SlackReplyComposerTarget,
        active: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let click_target = target.clone();
        let keyboard_target = target.clone();
        let button = div()
            .id(slack_reply_action_id(target, "format-toggle"))
            .role(Role::Button)
            .aria_label("Show formatting")
            .aria_toggled(if active {
                Toggled::True
            } else {
                Toggled::False
            })
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::FormatToggle,
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
                .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_reply_formatting(&click_target, cx);
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        cx.stop_propagation();
                        this.toggle_slack_reply_formatting(&keyboard_target, cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    fn render_slack_thread_emoji_button(
        &self,
        target: &SlackReplyComposerTarget,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let click_target = target.clone();
        let keyboard_target = target.clone();
        let button = div()
            .id(slack_reply_action_id(target, "emoji"))
            .role(Role::Button)
            .aria_label("Choose emoji")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Emoji,
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
                .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_slack_reply_emoji_picker(&click_target, cx);
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        cx.stop_propagation();
                        this.open_slack_reply_emoji_picker(&keyboard_target, cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    fn render_slack_thread_mention_button(
        &self,
        target: &SlackReplyComposerTarget,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let click_target = target.clone();
        let keyboard_target = target.clone();
        let button = div()
            .id(slack_reply_action_id(target, "mention"))
            .role(Role::Button)
            .aria_label("Mention someone")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Mention,
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
                .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_slack_reply_mention_picker(&click_target, cx);
                    if let Some(input) = this.slack_reply_composer_input(&click_target) {
                        window.focus(&input.read(cx).focus_handle_clone(), cx);
                    }
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.open_slack_reply_mention_picker(&keyboard_target, cx);
                        if let Some(input) = this.slack_reply_composer_input(&keyboard_target) {
                            window.focus(&input.read(cx).focus_handle_clone(), cx);
                        }
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    pub(super) fn render_slack_thread_broadcast_control(
        &self,
        panel: &SlackThreadPanelState,
        label: gpui::SharedString,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let control = div()
            .id(format!(
                "slack-thread-reply-broadcast-{}",
                panel.parent_message_id
            ))
            .role(Role::CheckBox)
            .aria_label(label.clone())
            .aria_toggled(if panel.reply_draft.borrow().broadcast {
                Toggled::True
            } else {
                Toggled::False
            })
            .h(px(SLACK_THREAD_COMPOSER_BROADCAST_HEIGHT))
            .flex_none()
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(slack_thread_broadcast_checkbox(
                panel.reply_draft.borrow().broadcast,
                palette,
            ))
            .child(label);
        if enabled {
            control
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_slack_thread_reply_broadcast(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        cx.stop_propagation();
                        this.toggle_slack_thread_reply_broadcast(cx);
                    }
                }))
                .into_any_element()
        } else {
            control.into_any_element()
        }
    }
}

fn slack_reply_action_id(target: &SlackReplyComposerTarget, action: &str) -> SharedString {
    match target {
        SlackReplyComposerTarget::ThreadPanel { .. } => format!("slack-thread-{action}").into(),
        SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
            format!("slack-all-threads-{action}-{thread_key}").into()
        }
    }
}

fn slack_thread_broadcast_checkbox(checked: bool, palette: SlackPalette) -> Div {
    div()
        .size(px(13.0))
        .mr(px(12.0))
        .rounded(px(3.0))
        .border_1()
        .border_color(rgb(if checked {
            0x007a5a
        } else {
            palette.main_secondary_text
        }))
        .bg(if checked {
            alpha(0x007a5a, 1.0)
        } else {
            alpha(palette.composer_bg, 0.0)
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .when(checked, |this| this.child("✓"))
}
