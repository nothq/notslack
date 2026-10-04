use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, KeyDownEvent, ParentElement, Role,
    Stateful, StatefulInteractiveElement, Styled, Window,
};

use super::reminder_action_key;
use crate::ui::surface::{SlackSurfaceAction, SurfaceState};

#[derive(Clone, Copy)]
pub(super) struct SlackLaterReminderButtonSpec {
    id: &'static str,
    label: &'static str,
    primary: bool,
    disabled: bool,
    restore_focus: bool,
    action: SlackSurfaceAction,
}

impl SlackLaterReminderButtonSpec {
    pub(super) const fn close(disabled: bool) -> Self {
        Self {
            id: "slack-later-reminder-close",
            label: "Close",
            primary: false,
            disabled,
            restore_focus: true,
            action: SurfaceState::close_slack_later_reminder_dialog,
        }
    }

    pub(super) const fn cancel(disabled: bool) -> Self {
        Self {
            id: "slack-later-reminder-cancel",
            label: "Cancel",
            primary: false,
            disabled,
            restore_focus: true,
            action: SurfaceState::close_slack_later_reminder_dialog,
        }
    }

    pub(super) const fn save(disabled: bool) -> Self {
        Self {
            id: "slack-later-reminder-save",
            label: "Save",
            primary: true,
            disabled,
            restore_focus: false,
            action: SurfaceState::submit_slack_later_reminder_dialog,
        }
    }
}

impl SurfaceState {
    pub(super) fn slack_later_reminder_due_button(
        &self,
        id: &'static str,
        label: &'static str,
        seconds: u64,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let disabled = self.slack_later_reminder_mutating;
        let accessibility_label = if disabled {
            format!("{label}, unavailable while saving")
        } else {
            label.to_string()
        };
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(accessibility_label)
            .focusable()
            .tab_stop(!disabled)
            .h(px(34.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(0x565856))
            .opacity(if disabled { 0.48 } else { 1.0 })
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(0x2b2d31)))
                    .focus_visible(|style| style.bg(rgb(0x2b2d31)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_slack_later_reminder_due_after(seconds, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if reminder_action_key(event) {
                            cx.stop_propagation();
                            this.set_slack_later_reminder_due_after(seconds, cx);
                        }
                    }))
            })
            .when(disabled, |this| this.cursor_default())
            .flex()
            .items_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xf8f8f8))
            .child(label)
    }

    pub(super) fn slack_later_reminder_button(
        &self,
        spec: SlackLaterReminderButtonSpec,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let button = self.slack_later_reminder_button_shell(spec);
        self.bind_slack_later_reminder_button(button, spec, cx)
    }

    fn slack_later_reminder_button_shell(
        &self,
        spec: SlackLaterReminderButtonSpec,
    ) -> Stateful<Div> {
        div()
            .id(spec.id)
            .when(spec.id == "slack-later-reminder-close", |this| {
                this.track_focus(&self.slack_later_reminder_close_focus_handle)
            })
            .when(spec.id == "slack-later-reminder-save", |this| {
                this.track_focus(&self.slack_later_reminder_save_focus_handle)
            })
            .role(Role::Button)
            .aria_label(reminder_button_accessibility_label(spec))
            .focusable()
            .tab_stop(!spec.disabled)
            .h(px(36.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(if spec.primary { 0x007a5a } else { 0x565856 }))
            .bg(rgb(if spec.primary { 0x007a5a } else { 0x1b1d21 }))
            .opacity(if spec.disabled { 0.48 } else { 1.0 })
            .flex()
            .items_center()
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(spec.label)
    }

    fn bind_slack_later_reminder_button(
        &self,
        button: Stateful<Div>,
        spec: SlackLaterReminderButtonSpec,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        button
            .when(!spec.disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(reminder_button_hover_background(spec)))
                    .focus_visible(move |style| style.bg(reminder_button_hover_background(spec)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.activate_slack_later_reminder_button(spec, window, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if reminder_action_key(event) {
                            cx.stop_propagation();
                            this.activate_slack_later_reminder_button(spec, window, cx);
                        }
                    }))
            })
            .when(spec.disabled, |this| this.cursor_default())
    }

    fn activate_slack_later_reminder_button(
        &mut self,
        spec: SlackLaterReminderButtonSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        (spec.action)(self, cx);
        if spec.restore_focus && self.slack_later_reminder_dialog.is_none() {
            window.focus(&self.focus_handle, cx);
        }
    }
}

fn reminder_button_accessibility_label(spec: SlackLaterReminderButtonSpec) -> String {
    if spec.disabled {
        format!("{}, unavailable while saving", spec.label)
    } else {
        spec.label.to_string()
    }
}

fn reminder_button_hover_background(spec: SlackLaterReminderButtonSpec) -> gpui::Hsla {
    rgb(if spec.primary { 0x148567 } else { 0x2b2d31 }).into()
}
