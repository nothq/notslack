use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, BoxShadow, Context, Div, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled,
};

use super::buttons::SlackLaterReminderButtonSpec;
use super::{SLACK_REMINDER_DAY, SLACK_REMINDER_HOUR, SLACK_REMINDER_WEEK};
use crate::ui::alpha;
use crate::ui::surface::{
    slack_palette, SlackLaterReminderDialog, SlackLaterReminderDialogMode, SurfaceState,
};

impl SurfaceState {
    pub(super) fn render_slack_later_reminder_dialog(
        &self,
        dialog: &SlackLaterReminderDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let title = match &dialog.mode {
            SlackLaterReminderDialogMode::Create { .. } => "New reminder",
            SlackLaterReminderDialogMode::Edit { .. } => "Edit reminder",
        };
        div()
            .id("slack-later-reminder-dialog")
            .role(Role::Dialog)
            .aria_label(title)
            .relative()
            .w(px(460.0))
            .max_w(gpui::relative(0.94))
            .rounded(px(10.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .bg(rgb(palette.main_bg))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(28.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_later_reminder_dialog_header(title, dialog.saving, cx))
            .child(self.slack_later_reminder_input_entity(cx))
            .child(self.render_slack_later_reminder_due_section(dialog, cx))
            .when_some(dialog.error.as_deref(), |this, error| {
                this.child(slack_later_reminder_dialog_error(error))
            })
            .child(self.render_slack_later_reminder_dialog_footer(dialog.saving, cx))
    }

    fn render_slack_later_reminder_dialog_header(
        &self,
        title: &'static str,
        saving: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child(title),
            )
            .child(
                self.slack_later_reminder_button(SlackLaterReminderButtonSpec::close(saving), cx),
            )
    }

    fn render_slack_later_reminder_due_section(
        &self,
        dialog: &SlackLaterReminderDialog,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Remind me"),
            )
            .child(self.render_slack_later_reminder_due_choices(cx))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(slack_later_reminder_dialog_due_label(dialog.due_at)),
            )
    }

    fn render_slack_later_reminder_due_choices(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(8.0))
            .child(self.slack_later_reminder_due_button(
                "slack-later-reminder-hour",
                "In 1 hour",
                SLACK_REMINDER_HOUR,
                cx,
            ))
            .child(self.slack_later_reminder_due_button(
                "slack-later-reminder-day",
                "In 1 day",
                SLACK_REMINDER_DAY,
                cx,
            ))
            .child(self.slack_later_reminder_due_button(
                "slack-later-reminder-week",
                "In 1 week",
                SLACK_REMINDER_WEEK,
                cx,
            ))
    }

    fn render_slack_later_reminder_dialog_footer(
        &self,
        saving: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(
                self.slack_later_reminder_button(SlackLaterReminderButtonSpec::cancel(saving), cx),
            )
            .child(self.slack_later_reminder_button(SlackLaterReminderButtonSpec::save(saving), cx))
    }
}

fn slack_later_reminder_dialog_error(error: &str) -> impl IntoElement {
    div()
        .id("slack-later-reminder-error")
        .role(Role::Alert)
        .text_size(px(12.0))
        .text_color(rgb(0xe01e5a))
        .child(error.to_string())
}

fn slack_later_reminder_dialog_due_label(due_at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs();
    let hours = due_at.saturating_sub(now).div_ceil(SLACK_REMINDER_HOUR);
    match hours {
        0 => "Currently overdue".to_string(),
        1 => "Currently due in about 1 hour".to_string(),
        2..=47 => format!("Currently due in about {hours} hours"),
        _ => format!("Currently due in about {} days", hours.div_ceil(24)),
    }
}
