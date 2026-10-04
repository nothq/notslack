use crate::ui::surface::{
    div, px, rgb, AnyElement, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    IntoElement, ParentElement, SlackLaterReminderDetail, SlackSurfaceActionButton,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::SlackLaterState;
use gpui::Role;

use super::super::helpers::later_action_key;

impl SurfaceState {
    pub(super) fn render_slack_later_reminder_detail(
        &self,
        detail: &SlackLaterReminderDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("Reminder", None, cx))
            .child(self.render_slack_later_reminder_detail_content(detail, cx))
            .into_any_element()
    }

    fn render_slack_later_reminder_detail_content(
        &self,
        detail: &SlackLaterReminderDetail,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-later-reminder-detail-scroll")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_scroll()
            .p(px(28.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(detail.description.clone()),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0xb9babd))
                    .child(slack_later_reminder_due_label(detail)),
            )
            .when_some(self.slack_later_reminder_error.as_deref(), |this, error| {
                this.child(slack_later_reminder_mutation_error(error))
            })
            .child(self.render_slack_later_reminder_actions(detail, cx))
    }

    fn render_slack_later_reminder_actions(
        &self,
        detail: &SlackLaterReminderDetail,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .pt(px(8.0))
            .flex()
            .gap(px(8.0))
            .when(detail.state == SlackLaterState::InProgress, |this| {
                this.child(self.render_slack_later_reminder_action_button(
                    SlackSurfaceActionButton {
                        id: "slack-later-reminder-edit",
                        label: "Edit",
                        action: SurfaceState::open_slack_later_reminder_edit_dialog,
                    },
                    false,
                    cx,
                ))
                .child(self.render_slack_later_reminder_action_button(
                    SlackSurfaceActionButton {
                        id: "slack-later-reminder-complete",
                        label: "Mark complete",
                        action: SurfaceState::complete_selected_slack_later_reminder,
                    },
                    true,
                    cx,
                ))
            })
            .child(self.render_slack_later_reminder_action_button(
                SlackSurfaceActionButton {
                    id: "slack-later-reminder-delete",
                    label: "Delete",
                    action: SurfaceState::delete_selected_slack_later_reminder,
                },
                false,
                cx,
            ))
    }

    fn render_slack_later_reminder_action_button(
        &self,
        button: SlackSurfaceActionButton,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSurfaceActionButton { id, label, action } = button;
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .h(px(36.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(if primary { 0x007a5a } else { 0x565856 }))
            .bg(rgb(if primary { 0x007a5a } else { 0x1b1d21 }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(if primary { 0x148567 } else { 0x2b2d31 })))
            .focus_visible(|style| style.bg(rgb(if primary { 0x148567 } else { 0x2b2d31 })))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .on_key_down(cx.listener(move |this, event, _, cx| {
                if later_action_key(event) {
                    cx.stop_propagation();
                    action(this, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(label)
    }
}

fn slack_later_reminder_mutation_error(error: &str) -> impl IntoElement {
    div()
        .id("slack-later-reminder-mutation-error")
        .role(Role::Alert)
        .text_size(px(13.0))
        .text_color(rgb(0xe01e5a))
        .child(error.to_string())
}

fn slack_later_reminder_due_label(detail: &SlackLaterReminderDetail) -> String {
    match detail.state {
        SlackLaterState::Completed => "Completed".to_string(),
        SlackLaterState::Archived => "Archived".to_string(),
        SlackLaterState::InProgress if detail.due_at == 0 => "No due time".to_string(),
        SlackLaterState::InProgress => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after the Unix epoch")
                .as_secs();
            if detail.due_at <= now {
                "Overdue".to_string()
            } else {
                let minutes = detail.due_at.saturating_sub(now).div_ceil(60);
                if minutes < 60 {
                    format!("Due in {minutes} minutes")
                } else {
                    let hours = minutes.div_ceil(60);
                    if hours < 24 {
                        format!("Due in {hours} hours")
                    } else {
                        let days = hours.div_ceil(24);
                        format!("Due in {days} days")
                    }
                }
            }
        }
    }
}
