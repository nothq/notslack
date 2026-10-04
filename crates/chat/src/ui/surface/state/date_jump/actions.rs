use super::{
    slack_date_jump_date_in_month, slack_date_jump_minimum_date, slack_date_jump_month_start,
    slack_date_jump_previous_month_date, slack_date_jump_shift_month,
    slack_date_jump_weekday_index, Context, Date, Duration, KeyDownEvent, SharedString,
    SlackDateJumpMenuAction, SlackDateJumpMenuState, SlackDateJumpOverlay,
    SlackDateJumpPickerState, SurfaceState,
};

pub(crate) struct SlackDateJumpMenuTarget {
    pub(crate) divider_id: SharedString,
    pub(crate) source_date: Date,
    pub(crate) anchor_x: f32,
    pub(crate) anchor_y: f32,
}

impl SurfaceState {
    pub(crate) fn open_slack_date_jump_menu(
        &mut self,
        target: SlackDateJumpMenuTarget,
        cx: &mut Context<Self>,
    ) {
        let SlackDateJumpMenuTarget {
            divider_id,
            source_date,
            anchor_x,
            anchor_y,
        } = target;
        if !self
            .slack_workspace_api_capabilities
            .navigate_conversation_dates
        {
            return;
        }
        if !self.slack_message_rows.iter().any(|row| {
            row.divider.as_ref().is_some_and(|divider| {
                divider.element_id == divider_id && divider.local_date == Some(source_date)
            })
        }) {
            return;
        }
        self.slack_reaction_picker = None;
        self.slack_message_menu = None;
        self.slack_message_menu_focus_pending = false;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        self.slack_schedule_overlay = None;
        self.invalidate_slack_date_jump_request();
        self.slack_date_jump_overlay = Some(SlackDateJumpOverlay::Menu(SlackDateJumpMenuState {
            divider_id,
            source_date,
            anchor_left: anchor_x - 72.0,
            anchor_top: anchor_y + 19.0,
            selected_index: None,
        }));
        self.slack_error = None;
        cx.notify();
    }

    pub(crate) fn update_slack_date_jump_anchor(
        &mut self,
        divider_id: &SharedString,
        left: f32,
        top: f32,
    ) -> bool {
        let Some(SlackDateJumpOverlay::Menu(menu)) = self.slack_date_jump_overlay.as_mut() else {
            return false;
        };
        if menu.divider_id != *divider_id
            || ((menu.anchor_left - left).abs() < 0.5 && (menu.anchor_top - top).abs() < 0.5)
        {
            return false;
        }
        menu.anchor_left = left;
        menu.anchor_top = top;
        true
    }

    pub(crate) fn close_slack_date_jump_overlay(&mut self, cx: &mut Context<Self>) {
        if self.slack_date_jump_overlay.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn reset_slack_date_jump_context(&mut self) {
        self.slack_date_jump_overlay = None;
        self.invalidate_slack_date_jump_request();
    }

    pub(crate) fn move_slack_date_jump_menu_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Menu(menu)) = self.slack_date_jump_overlay.as_mut() else {
            return;
        };
        let action_count = SlackDateJumpMenuAction::ALL.len();
        menu.selected_index = Some(match menu.selected_index {
            Some(index) => index.saturating_add_signed(delta).min(action_count - 1),
            None if delta < 0 => action_count - 1,
            None => 0,
        });
        cx.notify();
    }

    pub(crate) fn select_slack_date_jump_menu_boundary(
        &mut self,
        end: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Menu(menu)) = self.slack_date_jump_overlay.as_mut() else {
            return;
        };
        menu.selected_index = Some(if end {
            SlackDateJumpMenuAction::ALL.len() - 1
        } else {
            0
        });
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_date_jump_menu(&mut self, cx: &mut Context<Self>) {
        let Some(SlackDateJumpOverlay::Menu(menu)) = self.slack_date_jump_overlay.as_ref() else {
            return;
        };
        let action = SlackDateJumpMenuAction::ALL[menu.selected_index.unwrap_or(0)];
        self.activate_slack_date_jump_menu_action(action, cx);
    }

    pub(crate) fn activate_slack_date_jump_menu_action(
        &mut self,
        action: SlackDateJumpMenuAction,
        cx: &mut Context<Self>,
    ) {
        let today = match self.current_slack_date_jump_today() {
            Ok(today) => today,
            Err(error) => {
                self.fail_slack_date_jump(error, cx);
                return;
            }
        };
        match action {
            SlackDateJumpMenuAction::Yesterday => {
                let target = today
                    .previous_day()
                    .expect("current local date must have a previous day");
                self.jump_slack_to_date(target, cx);
            }
            SlackDateJumpMenuAction::LastWeek => {
                let target = today
                    .checked_sub(Duration::days(7))
                    .expect("current local date must support a one-week subtraction");
                self.jump_slack_to_date(target, cx);
            }
            SlackDateJumpMenuAction::LastMonth => {
                let target = slack_date_jump_previous_month_date(today);
                self.jump_slack_to_date(target, cx);
            }
            SlackDateJumpMenuAction::Beginning => self.jump_slack_to_beginning(cx),
            SlackDateJumpMenuAction::SpecificDate => {
                self.slack_date_jump_overlay = Some(SlackDateJumpOverlay::Picker(
                    SlackDateJumpPickerState::new(today),
                ));
                self.slack_error = None;
                cx.notify();
            }
        }
    }

    pub(crate) fn shift_slack_date_jump_picker_month(
        &mut self,
        month_delta: i32,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Picker(picker)) = self.slack_date_jump_overlay.as_mut()
        else {
            return;
        };
        let Some(displayed_month) =
            slack_date_jump_shift_month(picker.displayed_month, month_delta)
        else {
            return;
        };
        let minimum_month = slack_date_jump_minimum_date();
        let maximum_month = slack_date_jump_month_start(picker.today);
        if displayed_month < minimum_month || displayed_month > maximum_month {
            return;
        }
        let cursor_date =
            slack_date_jump_date_in_month(displayed_month, picker.cursor_date.day(), picker.today);
        picker.rebuild(displayed_month, cursor_date);
        cx.notify();
    }

    pub(crate) fn move_slack_date_jump_picker_cursor(
        &mut self,
        day_delta: i64,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Picker(picker)) = self.slack_date_jump_overlay.as_mut()
        else {
            return;
        };
        let Some(cursor_date) = picker
            .cursor_date
            .checked_add(Duration::days(day_delta))
            .filter(|date| *date >= slack_date_jump_minimum_date() && *date <= picker.today)
        else {
            return;
        };
        let displayed_month = slack_date_jump_month_start(cursor_date);
        picker.rebuild(displayed_month, cursor_date);
        cx.notify();
    }

    pub(crate) fn move_slack_date_jump_picker_to_week_boundary(
        &mut self,
        end: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Picker(picker)) = self.slack_date_jump_overlay.as_ref()
        else {
            return;
        };
        let weekday = i64::from(slack_date_jump_weekday_index(picker.cursor_date.weekday()));
        let delta = if end { 6 - weekday } else { -weekday };
        self.move_slack_date_jump_picker_cursor(delta, cx);
    }

    pub(crate) fn activate_slack_date_jump_picker_cursor(&mut self, cx: &mut Context<Self>) {
        let Some(SlackDateJumpOverlay::Picker(picker)) = self.slack_date_jump_overlay.as_ref()
        else {
            return;
        };
        self.select_slack_date_jump_calendar_date(picker.cursor_date, cx);
    }

    pub(crate) fn select_slack_date_jump_calendar_date(
        &mut self,
        date: Date,
        cx: &mut Context<Self>,
    ) {
        let Some(SlackDateJumpOverlay::Picker(picker)) = self.slack_date_jump_overlay.as_ref()
        else {
            return;
        };
        if date < slack_date_jump_minimum_date() || date > picker.today {
            return;
        }
        self.jump_slack_to_date(date, cx);
    }

    pub(crate) fn handle_slack_date_jump_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.slack_date_jump_overlay.as_ref() {
            Some(SlackDateJumpOverlay::Menu(_)) => self.handle_slack_date_jump_menu_key(event, cx),
            Some(SlackDateJumpOverlay::Picker(_)) => {
                self.handle_slack_date_jump_picker_key(event, cx)
            }
            None => false,
        }
    }

    fn handle_slack_date_jump_menu_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match event.keystroke.key.as_str() {
            "escape" => self.close_slack_date_jump_overlay(cx),
            "up" if !event.keystroke.modifiers.modified() => {
                self.move_slack_date_jump_menu_selection(-1, cx)
            }
            "down" if !event.keystroke.modifiers.modified() => {
                self.move_slack_date_jump_menu_selection(1, cx)
            }
            "home" if !event.keystroke.modifiers.modified() => {
                self.select_slack_date_jump_menu_boundary(false, cx)
            }
            "end" if !event.keystroke.modifiers.modified() => {
                self.select_slack_date_jump_menu_boundary(true, cx)
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                self.activate_selected_slack_date_jump_menu(cx)
            }
            _ => return false,
        }
        true
    }

    fn handle_slack_date_jump_picker_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = &event.keystroke.modifiers;
        let no_modifiers = !modifiers.modified();
        let shift_only = modifiers.shift
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.function;
        match event.keystroke.key.as_str() {
            "escape" => self.close_slack_date_jump_overlay(cx),
            "left" if no_modifiers => self.move_slack_date_jump_picker_cursor(-1, cx),
            "right" if no_modifiers => self.move_slack_date_jump_picker_cursor(1, cx),
            "up" if no_modifiers => self.move_slack_date_jump_picker_cursor(-7, cx),
            "down" if no_modifiers => self.move_slack_date_jump_picker_cursor(7, cx),
            "home" if no_modifiers => self.move_slack_date_jump_picker_to_week_boundary(false, cx),
            "end" if no_modifiers => self.move_slack_date_jump_picker_to_week_boundary(true, cx),
            "pageup" if no_modifiers => self.shift_slack_date_jump_picker_month(-1, cx),
            "pagedown" if no_modifiers => self.shift_slack_date_jump_picker_month(1, cx),
            "pageup" if shift_only => self.shift_slack_date_jump_picker_month(-12, cx),
            "pagedown" if shift_only => self.shift_slack_date_jump_picker_month(12, cx),
            "enter" | "space" if no_modifiers => self.activate_slack_date_jump_picker_cursor(cx),
            _ => return false,
        }
        true
    }
}
