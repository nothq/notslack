use std::{rc::Rc, time::SystemTime};

use gpui::{Entity, FocusHandle, Window};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{next_slack_later_generation, Context, SlackRailView, SurfaceState};
use crate::ui::surface::{slack_palette, SlackLaterReminderDialog, SlackLaterReminderDialogMode};
use crate::ui::{
    alpha, px, rgb, SlackLaterState, SlackReminderClientId, SlackReminderDraft,
    SlackReminderMutation,
};

const SLACK_REMINDER_DEFAULT_DUE_SECONDS: u64 = 24 * 60 * 60;

mod mutation;

impl SurfaceState {
    pub(crate) fn ensure_slack_later_reminder_focus_observers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.slack_later_reminder_focus_observers_registered {
            return;
        }
        let start = self.slack_later_reminder_focus_guard_start.clone();
        cx.on_focus(&start, window, |surface, window, cx| {
            let focus = surface.slack_later_reminder_last_focus_handle();
            window.focus(&focus, cx);
        })
        .detach();
        let end = self.slack_later_reminder_focus_guard_end.clone();
        cx.on_focus(&end, window, |surface, window, cx| {
            let focus = surface.slack_later_reminder_first_focus_handle();
            window.focus(&focus, cx);
        })
        .detach();
        self.slack_later_reminder_focus_observers_registered = true;
    }

    fn slack_later_reminder_first_focus_handle(&self) -> FocusHandle {
        if self.slack_later_reminder_mutating {
            self.slack_later_reminder_layer_focus_handle.clone()
        } else {
            self.slack_later_reminder_close_focus_handle.clone()
        }
    }

    fn slack_later_reminder_last_focus_handle(&self) -> FocusHandle {
        if self.slack_later_reminder_mutating {
            self.slack_later_reminder_layer_focus_handle.clone()
        } else {
            self.slack_later_reminder_save_focus_handle.clone()
        }
    }

    pub(crate) fn open_slack_later_reminder_create_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.mutate_later_reminders
            || self.slack_active_rail_view != SlackRailView::Later
            || self.slack_later_reminder_mutating
        {
            return;
        }
        self.slack_later_reminder_dialog = Some(SlackLaterReminderDialog {
            mode: SlackLaterReminderDialogMode::Create {
                client_id: SlackReminderClientId::new(),
            },
            description: String::new(),
            due_at: slack_reminder_due_after(SLACK_REMINDER_DEFAULT_DUE_SECONDS),
            saving: false,
            error: None,
        });
        self.slack_later_reminder_error = None;
        self.slack_later_reminder_generation =
            next_slack_later_generation(self.slack_later_reminder_generation);
        cx.notify();
    }

    pub(crate) fn open_slack_later_reminder_edit_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.mutate_later_reminders
            || self.slack_later_reminder_mutating
        {
            return;
        }
        let Some(detail) = self
            .selected_slack_later_row()
            .and_then(|row| row.reminder_detail())
            .filter(|detail| detail.state == SlackLaterState::InProgress)
            .cloned()
        else {
            return;
        };
        self.slack_later_reminder_dialog = Some(SlackLaterReminderDialog {
            mode: SlackLaterReminderDialogMode::Edit {
                reminder_id: detail.reminder_id,
            },
            description: detail.description.to_string(),
            due_at: detail.due_at,
            saving: false,
            error: None,
        });
        self.slack_later_reminder_error = None;
        self.slack_later_reminder_generation =
            next_slack_later_generation(self.slack_later_reminder_generation);
        cx.notify();
    }

    pub(crate) fn close_slack_later_reminder_dialog(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_later_reminder_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        if self.slack_later_reminder_dialog.take().is_some() {
            self.slack_later_reminder_generation =
                next_slack_later_generation(self.slack_later_reminder_generation);
            cx.notify();
        }
    }

    pub(crate) fn set_slack_later_reminder_due_after(
        &mut self,
        seconds: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self
            .slack_later_reminder_dialog
            .as_mut()
            .filter(|dialog| !dialog.saving)
        else {
            return;
        };
        dialog.due_at = slack_reminder_due_after(seconds);
        dialog.error = None;
        cx.notify();
    }

    pub(crate) fn submit_slack_later_reminder_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .slack_later_reminder_dialog
            .as_ref()
            .filter(|dialog| !dialog.saving)
        else {
            return;
        };
        let draft = match SlackReminderDraft::new(dialog.description.clone(), dialog.due_at) {
            Ok(draft) => draft,
            Err(error) => {
                self.slack_later_reminder_dialog
                    .as_mut()
                    .expect("validated Slack reminder dialog must remain open")
                    .error = Some(error);
                cx.notify();
                return;
            }
        };
        let mutation = match &dialog.mode {
            SlackLaterReminderDialogMode::Create { client_id } => SlackReminderMutation::Create {
                client_id: client_id.clone(),
                draft,
            },
            SlackLaterReminderDialogMode::Edit { reminder_id } => SlackReminderMutation::Edit {
                reminder_id: reminder_id.clone(),
                draft,
            },
        };
        self.begin_slack_later_reminder_mutation(mutation, cx);
    }

    pub(crate) fn complete_selected_slack_later_reminder(&mut self, cx: &mut Context<Self>) {
        let Some(reminder_id) = self
            .selected_slack_later_row()
            .and_then(|row| row.reminder_detail())
            .filter(|detail| detail.state == SlackLaterState::InProgress)
            .map(|detail| detail.reminder_id.clone())
        else {
            return;
        };
        self.begin_slack_later_reminder_mutation(
            SlackReminderMutation::Complete { reminder_id },
            cx,
        );
    }

    pub(crate) fn delete_selected_slack_later_reminder(&mut self, cx: &mut Context<Self>) {
        let Some(reminder_id) = self
            .selected_slack_later_row()
            .and_then(|row| row.reminder_detail())
            .map(|detail| detail.reminder_id.clone())
        else {
            return;
        };
        self.begin_slack_later_reminder_mutation(SlackReminderMutation::Delete { reminder_id }, cx);
    }

    pub(crate) fn slack_later_reminder_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_later_reminder_dialog
            .as_ref()
            .expect("Slack reminder input requires an open dialog");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if let Some(dialog) = surface.slack_later_reminder_dialog.as_mut() {
                        dialog.description = value;
                        dialog.error = None;
                        cx.notify();
                    }
                })
                .ok();
        });
        let props = TextInputProps::single_line(dialog.description.clone())
            .placeholder("What should I remind you about?")
            .style(self.slack_later_reminder_input_style())
            .disabled(dialog.saving)
            .request_focus(!dialog.saving)
            .accessibility(
                self.slack_later_reminder_accessibility_id.clone(),
                "Reminder text",
            )
            .on_change(on_change)
            .on_submit(self.slack_later_reminder_submit_action(cx))
            .on_escape(self.slack_later_reminder_close_action(cx));
        self.slack_later_reminder_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_later_reminder_input.clone()
    }

    fn slack_later_reminder_submit_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.submit_slack_later_reminder_dialog(cx)
                })
                .ok();
        })
    }

    fn slack_later_reminder_close_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.close_slack_later_reminder_dialog(cx);
                    surface
                        .slack_later_reminder_dialog
                        .is_none()
                        .then(|| surface.focus_handle.clone())
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    fn slack_later_reminder_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(42.0),
            min_height: px(42.0),
            padding_x: px(12.0),
            padding_y: px(9.0),
            radius: px(6.0),
            background: rgb(palette.composer_bg).into(),
            border: rgb(palette.composer_border).into(),
            focused_border: rgb(palette.composer_focused_border).into(),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.composer_placeholder).into(),
            selection: alpha(palette.link, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(22.0),
            font_family: Some("Lato".into()),
        }
    }
}

fn slack_reminder_due_after(seconds: u64) -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs()
        .checked_add(seconds)
        .expect("Slack reminder due time overflowed")
}
