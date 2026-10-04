use gpui::{AppContext, ClickEvent, KeyDownEvent, Render, Role, Window};

use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, FluentBuilder,
    InteractiveElement, IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use crate::ui::surface::SlackScheduleButtonPresentation;

struct SlackComposerScheduleTooltip {
    label: &'static str,
}

impl Render for SlackComposerScheduleTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(6.0))
            .bg(rgb(0x1d1c1d))
            .text_size(px(13.0))
            .line_height(px(18.0))
            .text_color(rgb(0xffffff))
            .child(self.label)
    }
}

impl SurfaceState {
    pub(super) fn render_slack_send_controls(&self, cx: &mut Context<Self>) -> gpui::Div {
        if !self.slack_workspace_api_capabilities.send_message {
            return div().w(px(32.0)).h(px(28.0));
        }
        let has_draft =
            !self.slack_composer_text.trim().is_empty() || !self.slack_composer_files.is_empty();
        let files_sendable = self.slack_composer_files.slack_file_ids_ready()
            && (self.slack_composer_files.is_empty()
                || self.slack_workspace_api_capabilities.share_files);
        let enabled = has_draft
            && files_sendable
            && self.has_current_slack_send_target()
            && self.slack_schedule_pending.is_none()
            && !self.slack_composer_capture_blocks_current_draft();
        let schedule_owner = self.current_slack_main_schedule_owner();
        let schedule_visible = self.slack_schedule_controls_visible();
        let schedule_enabled = schedule_owner
            .as_ref()
            .is_some_and(|owner| self.can_schedule_slack_draft(owner));
        let group_enabled = enabled || schedule_enabled;
        let send_button =
            self.render_slack_send_button(enabled, !group_enabled && schedule_visible, cx);
        let schedule_button = schedule_visible.then(|| {
            self.render_slack_schedule_button(
                SlackScheduleButtonPresentation {
                    element_id: "slack-composer-schedule".into(),
                    owner: schedule_owner,
                    enabled: schedule_enabled,
                    group_enabled,
                    editing: self.slack_active_scheduled_edit.is_some(),
                },
                cx,
            )
        });
        self.render_slack_split_send_controls(send_button, schedule_button, group_enabled)
    }

    pub(in crate::ui::surface) fn render_slack_split_send_controls(
        &self,
        send_button: AnyElement,
        schedule_button: Option<AnyElement>,
        group_enabled: bool,
    ) -> gpui::Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(28.0))
            .rounded(px(4.0))
            .bg(alpha(0xffffff, 0.0))
            .when(group_enabled, |this| {
                this.border_1()
                    .border_color(alpha(palette.send_disabled_border, 0.18))
                    .bg(alpha(0x007a5a, 1.0))
            })
            .when(!group_enabled, |this| this.opacity(0.3))
            .flex()
            .items_center()
            .child(send_button)
            .children(schedule_button)
    }

    fn render_slack_send_button(
        &self,
        enabled: bool,
        show_disabled_separator: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = self.slack_send_button_content(enabled, show_disabled_separator, cx);
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.submit_slack_composer(cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                        || event.keystroke.modifiers.modified()
                    {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    this.submit_slack_composer(cx);
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    fn slack_send_button_content(
        &self,
        enabled: bool,
        show_disabled_separator: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = slack_palette(self.appearance_mode);
        let editing = self.slack_active_scheduled_edit.is_some();
        div()
            .id("slack-composer-send")
            .role(Role::Button)
            .aria_label(if editing {
                "Save scheduled message"
            } else {
                "Send now"
            })
            .w(px(if editing { 52.0 } else { 32.0 }))
            .h_full()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .when(editing, |this| {
                this.text_size(px(12.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(if enabled { 0xffffff } else { palette.main_text }))
                    .child("Save")
            })
            .when(!editing, |this| {
                this.child(slack_icon(
                    SlackShellIcon::Send,
                    if enabled { 0xffffff } else { palette.main_text },
                    16.0,
                    cx,
                ))
            })
            .when(show_disabled_separator, |this| {
                this.child(
                    div()
                        .absolute()
                        .right(px(0.0))
                        .top(px(4.0))
                        .w(px(1.0))
                        .h(px(20.0))
                        .bg(rgb(palette.main_text)),
                )
            })
    }

    pub(in crate::ui::surface) fn render_slack_schedule_button(
        &self,
        presentation: SlackScheduleButtonPresentation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let click_owner = presentation.owner.clone();
        let keyboard_owner = presentation.owner.clone();
        let enabled = presentation.enabled;
        let button = self.slack_schedule_button_content(&presentation, cx);
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    let owner = click_owner
                        .clone()
                        .expect("enabled Slack schedule control must retain its typed owner");
                    let position = event.position();
                    this.open_slack_send_options_for_owner(
                        owner,
                        crate::ui::surface::SlackScheduleAnchor {
                            x: position.x.as_f32(),
                            y: position.y.as_f32(),
                        },
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                        || event.keystroke.modifiers.modified()
                    {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    let owner = keyboard_owner
                        .clone()
                        .expect("enabled Slack schedule control must retain its typed owner");
                    this.open_slack_send_options_for_owner(
                        owner,
                        this.slack_schedule_keyboard_anchor(),
                        cx,
                    );
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    fn slack_schedule_button_content(
        &self,
        presentation: &SlackScheduleButtonPresentation,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = slack_palette(self.appearance_mode);
        let tooltip_label = if presentation.editing {
            "Choose a new scheduled time"
        } else {
            "Schedule for later"
        };
        div()
            .id(presentation.element_id.clone())
            .role(Role::Button)
            .aria_label(tooltip_label)
            .w(px(28.0))
            .h_full()
            .when(presentation.group_enabled, |this| {
                this.border_l_1().border_color(alpha(
                    palette.send_disabled_border,
                    if presentation.enabled { 0.18 } else { 0.12 },
                ))
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::ChevronDown,
                if presentation.enabled {
                    0xffffff
                } else if presentation.group_enabled {
                    palette.send_disabled_icon
                } else {
                    palette.main_text
                },
                14.0,
                cx,
            ))
            .tooltip(move |_, cx| {
                cx.new(|_| SlackComposerScheduleTooltip {
                    label: tooltip_label,
                })
                .into()
            })
    }
}
