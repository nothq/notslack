use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackAuxPanelQueryBehavior,
    SlackShellIcon, SlackThreadPanelState, StatefulInteractiveElement, Styled, SurfaceState,
    Window, SLACK_COMPOSER_ATTACHMENTS_HEIGHT,
};
use crate::ui::surface::SlackScheduleButtonPresentation;
use gpui::Role;

mod actions;
mod attachments;
mod formatting;
mod input;

const SLACK_THREAD_COMPOSER_BORDER: f32 = 2.0;
const SLACK_THREAD_COMPOSER_FORMAT_HEIGHT: f32 = 38.0;
const SLACK_THREAD_COMPOSER_INPUT_HEIGHT: f32 = 38.0;
const SLACK_THREAD_COMPOSER_BROADCAST_HEIGHT: f32 = 26.0;
const SLACK_THREAD_COMPOSER_TOOLBAR_HEIGHT: f32 = 40.0;
const SLACK_THREAD_COMPOSER_LINE_HEIGHT: f32 = 21.0;
const SLACK_THREAD_COMPOSER_POPOVER_LEFT: f32 = 16.0;
const SLACK_THREAD_COMPOSER_POPOVER_BOTTOM: f32 = 168.0;

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_thread_composer(
        &self,
        panel: &SlackThreadPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        let has_attachments = self.slack_thread_reply_draft_has_files(&panel.reply_draft_key);
        let reply_target = self
            .slack_thread_panel_reply_composer_target()
            .expect("rendered Slack thread composer must retain its typed reply target");
        let composer_popover = self.slack_aux_panel.as_ref().filter(|popover| {
            self.slack_composer_aux_target.as_ref()
                == Some(&super::SlackComposerTarget::Reply(reply_target.clone()))
                && matches!(
                    popover.query_behavior,
                    Some(SlackAuxPanelQueryBehavior::Emoji | SlackAuxPanelQueryBehavior::Mention)
                )
        });
        div()
            .relative()
            .flex_none()
            .px(px(16.0))
            .pt(px(12.0))
            .when_some(panel.reply_error.as_deref(), |this, error| {
                this.child(slack_thread_reply_error(error))
            })
            .when(panel.reply_error.is_none(), |this| {
                this.when_some(
                    self.slack_draft_autosave_error(&panel.reply_draft_key),
                    |this, error| this.child(slack_thread_reply_error(error)),
                )
            })
            .child(self.render_slack_thread_composer_box(panel, cx))
            .when_some(composer_popover, |this, popover| {
                this.child(self.render_slack_composer_popover_with_offsets(
                    popover,
                    SLACK_THREAD_COMPOSER_POPOVER_LEFT,
                    SLACK_THREAD_COMPOSER_POPOVER_BOTTOM
                        + if has_attachments {
                            SLACK_COMPOSER_ATTACHMENTS_HEIGHT
                        } else {
                            0.0
                        },
                    cx,
                ))
            })
            .child(self.render_slack_composer_hint())
    }

    fn render_slack_thread_composer_box(
        &self,
        panel: &SlackThreadPanelState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_attachments = self.slack_thread_reply_draft_has_files(&panel.reply_draft_key);
        let composer_height = slack_thread_composer_height(panel, has_attachments);
        let target_enabled = self.can_send_slack_thread_reply();
        let send_enabled =
            target_enabled && self.slack_thread_reply_draft_is_sendable(&panel.reply_draft_key);
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(format!(
                "slack-thread-reply-composer-{}",
                panel.parent_message_id
            ))
            .h(px(composer_height))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(if panel.reply_composer_focused {
                palette.composer_focused_border
            } else {
                palette.composer_border
            }))
            .bg(rgb(palette.composer_bg))
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.handle_slack_composer_link_shortcut(event, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .when(panel.reply_formatting_enabled, |this| {
                this.child(self.render_slack_thread_format_bar(panel, target_enabled, cx))
            })
            .when(has_attachments, |this| {
                this.child(self.render_slack_thread_draft_attachments(
                    &panel.reply_draft_key,
                    &panel.parent_message_id,
                    cx,
                ))
            })
            .child(self.render_slack_thread_composer_input(panel, target_enabled, cx))
            .when_some(panel.broadcast_label.clone(), |this, label| {
                this.child(self.render_slack_thread_broadcast_control(
                    panel,
                    label,
                    target_enabled,
                    cx,
                ))
            })
            .child(self.render_slack_thread_composer_toolbar(
                panel,
                target_enabled,
                send_enabled,
                cx,
            ))
    }

    fn render_slack_thread_composer_toolbar(
        &self,
        panel: &SlackThreadPanelState,
        target_enabled: bool,
        send_enabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let schedule_owner = self.slack_thread_panel_schedule_owner(panel);
        let schedule_visible = self.slack_schedule_controls_visible();
        let schedule_enabled = schedule_visible && self.can_schedule_slack_draft(&schedule_owner);
        let group_enabled = send_enabled || schedule_enabled;
        let send_button = self.render_slack_thread_reply_send_button(panel, send_enabled, cx);
        let schedule_button = schedule_visible.then(|| {
            self.render_slack_schedule_button(
                SlackScheduleButtonPresentation {
                    element_id: format!("slack-thread-reply-schedule-{}", panel.parent_message_id)
                        .into(),
                    owner: Some(schedule_owner),
                    enabled: schedule_enabled,
                    group_enabled,
                    editing: false,
                },
                cx,
            )
        });
        div()
            .h(px(SLACK_THREAD_COMPOSER_TOOLBAR_HEIGHT))
            .flex_none()
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_thread_toolbar_actions(panel, target_enabled, cx))
            .child(self.render_slack_split_send_controls(
                send_button,
                schedule_button,
                group_enabled,
            ))
    }

    fn render_slack_thread_reply_send_button(
        &self,
        panel: &SlackThreadPanelState,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let pending = self.slack_thread_reply_is_pending(&panel.reply_draft_key);
        let button = div()
            .id(format!(
                "slack-thread-reply-send-{}",
                panel.parent_message_id
            ))
            .role(Role::Button)
            .aria_label(slack_thread_reply_send_label(pending))
            .w(px(32.0))
            .h_full()
            .rounded(px(4.0))
            .bg(if enabled {
                alpha(0x007a5a, 1.0)
            } else {
                alpha(palette.send_disabled_border, 0.08)
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Send,
                if enabled {
                    0xffffff
                } else {
                    palette.send_disabled_icon
                },
                16.0,
                cx,
            ));
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x148567)))
                .focus_visible(|style| style.bg(rgb(0x148567)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.send_slack_thread_reply(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        this.send_slack_thread_reply(cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    pub(crate) fn ensure_slack_thread_composer_blur_observer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self
            .slack_thread_composer_input
            .read(cx)
            .focus_handle_clone();
        if !self.slack_thread_composer_blur_observer_registered {
            cx.on_blur(&focus, window, |surface, _, cx| {
                if let Some(panel) = surface
                    .slack_thread_panel
                    .as_mut()
                    .filter(|panel| panel.reply_composer_focused)
                {
                    panel.reply_composer_focused = false;
                    cx.notify();
                }
            })
            .detach();
            self.slack_thread_composer_blur_observer_registered = true;
        }
        let focus_requested = self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.reply_composer_focused);
        if !focus_requested && focus.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
    }
}

fn slack_thread_reply_send_label(pending: bool) -> &'static str {
    if pending {
        "Sending thread reply"
    } else {
        "Send thread reply"
    }
}

fn slack_thread_composer_height(panel: &SlackThreadPanelState, has_attachments: bool) -> f32 {
    SLACK_THREAD_COMPOSER_BORDER
        + if panel.reply_formatting_enabled {
            SLACK_THREAD_COMPOSER_FORMAT_HEIGHT
        } else {
            0.0
        }
        + if has_attachments {
            SLACK_COMPOSER_ATTACHMENTS_HEIGHT
        } else {
            0.0
        }
        + SLACK_THREAD_COMPOSER_INPUT_HEIGHT
        + SLACK_THREAD_COMPOSER_TOOLBAR_HEIGHT
        + panel
            .broadcast_label
            .as_ref()
            .map_or(0.0, |_| SLACK_THREAD_COMPOSER_BROADCAST_HEIGHT)
}

fn slack_thread_reply_error(error: &str) -> impl IntoElement {
    div()
        .id("slack-thread-reply-error")
        .aria_label(format!("Thread reply failed: {error}"))
        .pb(px(8.0))
        .text_size(px(12.0))
        .line_height(px(16.0))
        .text_color(rgb(0xe89191))
        .child(error.to_string())
}
