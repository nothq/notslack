use super::super::all_threads_action_key;
use super::{SlackAllThreadsComposerView, SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT};
use crate::ui::surface::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, ParentElement, SlackAllThreadRow, SlackPalette,
    SlackReplyComposerTarget, SlackScheduleButtonPresentation, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState, SLACK_COMPOSER_TOOLBAR_HEIGHT,
};
use gpui::{Role, Toggled};

impl SurfaceState {
    pub(super) fn render_slack_all_threads_composer_footer(
        &self,
        row: &SlackAllThreadRow,
        state: &SlackAllThreadsComposerView,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let group_enabled = state.send_enabled || state.schedule_enabled;
        let send_button = self.render_slack_all_threads_send_button(
            &state.target,
            state.send_enabled,
            state.pending,
            cx,
        );
        let schedule_button = state.schedule_visible.then(|| {
            self.render_slack_schedule_button(
                SlackScheduleButtonPresentation {
                    element_id: format!("slack-all-threads-schedule-{}", row.key).into(),
                    owner: state.schedule_owner.clone(),
                    enabled: state.schedule_enabled,
                    group_enabled,
                    editing: false,
                },
                cx,
            )
        });
        div()
            .id(format!("slack-all-threads-actions-{}", row.key))
            .role(Role::Toolbar)
            .aria_label("Thread reply actions")
            .h(px(SLACK_COMPOSER_TOOLBAR_HEIGHT))
            .flex_none()
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_all_threads_composer_actions(row, state, palette, cx))
            .child(self.render_slack_split_send_controls(
                send_button,
                schedule_button,
                group_enabled,
            ))
    }

    fn render_slack_all_threads_composer_actions(
        &self,
        row: &SlackAllThreadRow,
        state: &SlackAllThreadsComposerView,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(self.render_slack_thread_attachment_picker_button(
                state.draft_key.clone(),
                row.key.as_ref(),
                state.enabled,
                cx,
            ))
            .child(self.render_slack_reply_toolbar_aux_actions(&state.target, state.enabled, cx))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(if state.reply_error.is_some() {
                        0xc4314b
                    } else {
                        palette.main_secondary_text
                    }))
                    .when(state.pending, |this| this.child("Sending…"))
                    .when_some(state.reply_error.clone(), |this, error| this.child(error)),
            )
    }

    pub(super) fn render_slack_all_threads_broadcast_control(
        &self,
        target: &SlackReplyComposerTarget,
        label: gpui::SharedString,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let thread_key = target
            .all_threads_key()
            .expect("All Threads broadcast target must retain its thread key");
        let click_target = target.clone();
        let keyboard_target = target.clone();
        let selected = self
            .slack_composer_drafts
            .get(target.draft_key())
            .is_some_and(|draft| draft.broadcast);
        let control = div()
            .id(format!("slack-all-threads-broadcast-{thread_key}"))
            .role(Role::CheckBox)
            .aria_label(label.clone())
            .aria_toggled(if selected {
                Toggled::True
            } else {
                Toggled::False
            })
            .h(px(SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT))
            .flex_none()
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(slack_all_threads_broadcast_checkbox(
                selected,
                palette.main_secondary_text,
                palette.composer_bg,
            ))
            .child(label);
        if enabled {
            control
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_all_threads_reply_broadcast(&click_target, cx);
                }))
                .on_key_down(cx.listener(move |this, event, window, cx| {
                    if all_threads_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_all_threads_reply_broadcast(&keyboard_target, cx);
                    }
                }))
                .into_any_element()
        } else {
            control.into_any_element()
        }
    }

    fn render_slack_all_threads_send_button(
        &self,
        target: &SlackReplyComposerTarget,
        enabled: bool,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let thread_key = target
            .all_threads_key()
            .expect("All Threads send target must retain its thread key");
        let click_target = target.clone();
        let keyboard_target = target.clone();
        let button = div()
            .id(format!("slack-all-threads-send-{thread_key}"))
            .role(Role::Button)
            .aria_label(slack_all_threads_send_label(pending))
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.send_slack_all_threads_reply(&click_target, cx);
                }))
                .on_key_down(cx.listener(move |this, event, window, cx| {
                    if all_threads_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.send_slack_all_threads_reply(&keyboard_target, cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }
}

fn slack_all_threads_send_label(pending: bool) -> &'static str {
    if pending {
        "Sending thread reply"
    } else {
        "Send thread reply"
    }
}

fn slack_all_threads_broadcast_checkbox(selected: bool, text: u32, background: u32) -> Div {
    div()
        .size(px(13.0))
        .mr(px(12.0))
        .rounded(px(3.0))
        .border_1()
        .border_color(rgb(if selected { 0x007a5a } else { text }))
        .bg(if selected {
            alpha(0x007a5a, 1.0)
        } else {
            alpha(background, 0.0)
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .when(selected, |this| this.child("✓"))
}
