use super::{
    slack_schedule_time_cursor_index, slack_schedule_time_options, stop_slack_schedule_key_event,
    Context, KeyDownEvent, NaiveDate, ScrollStrategy, SlackScheduleCustomState,
    SlackScheduleDatePickerState, SlackScheduleMonthDirection, SlackScheduleNestedPicker,
    SlackScheduleTimePickerState, SurfaceState, UniformListScrollHandle, Window,
};

impl SurfaceState {
    pub(crate) fn toggle_slack_custom_schedule_date_picker(&mut self, cx: &mut Context<Self>) {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return;
        };
        custom.date_input_focused = false;
        custom.time_input_focused = false;
        if matches!(
            custom.picker.as_ref(),
            Some(SlackScheduleNestedPicker::Date(_))
        ) {
            custom.picker = None;
        } else {
            custom.picker = Some(SlackScheduleNestedPicker::Date(
                SlackScheduleDatePickerState::new(
                    custom.opened_date,
                    custom.maximum_date,
                    custom.date,
                ),
            ));
        };
        custom.error = None;
        cx.notify();
    }

    pub(crate) fn toggle_slack_custom_schedule_time_picker(&mut self, cx: &mut Context<Self>) {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return;
        };
        custom.date_input_focused = false;
        custom.time_input_focused = false;
        if matches!(
            custom.picker.as_ref(),
            Some(SlackScheduleNestedPicker::Time(_))
        ) {
            custom.picker = None;
            cx.notify();
            return;
        }
        let options = slack_schedule_time_options(custom.timezone, custom.date);
        let Some(cursor_index) = slack_schedule_time_cursor_index(&options, custom.time) else {
            custom.picker = None;
            custom.error = Some("Choose a later date; no future times remain today.".to_string());
            cx.notify();
            return;
        };
        let selected_index = options.iter().position(|option| option.time == custom.time);
        let scroll_handle = UniformListScrollHandle::new();
        scroll_handle.scroll_to_item(cursor_index, ScrollStrategy::Nearest);
        custom.picker = Some(SlackScheduleNestedPicker::Time(
            SlackScheduleTimePickerState {
                options,
                selected_index,
                cursor_index,
                scroll_handle,
            },
        ));
        custom.error = None;
        cx.notify();
    }

    pub(crate) fn show_previous_slack_schedule_month(&mut self, cx: &mut Context<Self>) {
        self.move_slack_schedule_month(SlackScheduleMonthDirection::Previous, cx);
    }

    pub(crate) fn show_next_slack_schedule_month(&mut self, cx: &mut Context<Self>) {
        self.move_slack_schedule_month(SlackScheduleMonthDirection::Next, cx);
    }

    pub(crate) fn select_slack_schedule_calendar_date(
        &mut self,
        date: NaiveDate,
        cx: &mut Context<Self>,
    ) {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return;
        };
        if date < custom.opened_date || date > custom.maximum_date {
            return;
        }
        custom.set_date(date);
        let options = slack_schedule_time_options(custom.timezone, custom.date);
        if !options.iter().any(|option| option.time == custom.time) {
            let Some(first) = options.first() else {
                custom.error =
                    Some("Choose a later date; no future times remain today.".to_string());
                cx.notify();
                return;
            };
            custom.set_time(first.time);
        }
        custom.picker = None;
        cx.notify();
    }

    pub(crate) fn select_slack_schedule_time(
        &mut self,
        option_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return;
        };
        let Some(SlackScheduleNestedPicker::Time(picker)) = custom.picker.as_ref() else {
            return;
        };
        let Some(option) = picker.options.get(option_index) else {
            return;
        };
        let time = option.time;
        custom.set_time(time);
        custom.picker = None;
        cx.notify();
    }

    pub(crate) fn handle_slack_schedule_date_control_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handled = match event.keystroke.key.as_str() {
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.select_slack_schedule_calendar_cursor(cx);
                } else {
                    self.toggle_slack_custom_schedule_date_picker(cx);
                }
                true
            }
            "left" if !event.keystroke.modifiers.modified() => {
                self.move_slack_schedule_calendar_cursor(-1, cx)
            }
            "right" if !event.keystroke.modifiers.modified() => {
                self.move_slack_schedule_calendar_cursor(1, cx)
            }
            "up" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.move_slack_schedule_calendar_cursor(-7, cx)
                } else {
                    self.toggle_slack_custom_schedule_date_picker(cx);
                    true
                }
            }
            "down" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.move_slack_schedule_calendar_cursor(7, cx)
                } else {
                    self.toggle_slack_custom_schedule_date_picker(cx);
                    true
                }
            }
            "escape" => self.close_slack_schedule_nested_picker(cx),
            _ => false,
        };
        stop_slack_schedule_key_event(handled, window, cx);
    }

    pub(crate) fn handle_slack_schedule_time_control_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handled = match event.keystroke.key.as_str() {
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_time_picker_is_open() {
                    self.select_slack_schedule_time_cursor(cx);
                } else {
                    self.toggle_slack_custom_schedule_time_picker(cx);
                }
                true
            }
            "up" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_time_picker_is_open() {
                    self.move_slack_schedule_time_cursor(-1, cx)
                } else {
                    self.toggle_slack_custom_schedule_time_picker(cx);
                    true
                }
            }
            "down" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_time_picker_is_open() {
                    self.move_slack_schedule_time_cursor(1, cx)
                } else {
                    self.toggle_slack_custom_schedule_time_picker(cx);
                    true
                }
            }
            "escape" => self.close_slack_schedule_nested_picker(cx),
            _ => false,
        };
        stop_slack_schedule_key_event(handled, window, cx);
    }

    pub(crate) fn handle_slack_schedule_picker_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handled = match event.keystroke.key.as_str() {
            "escape" => self.close_slack_schedule_nested_picker(cx),
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.select_slack_schedule_calendar_cursor(cx);
                } else if self.slack_schedule_time_picker_is_open() {
                    self.select_slack_schedule_time_cursor(cx);
                }
                true
            }
            "left"
                if self.slack_schedule_date_picker_is_open()
                    && !event.keystroke.modifiers.modified() =>
            {
                self.move_slack_schedule_calendar_cursor(-1, cx)
            }
            "right"
                if self.slack_schedule_date_picker_is_open()
                    && !event.keystroke.modifiers.modified() =>
            {
                self.move_slack_schedule_calendar_cursor(1, cx)
            }
            "up" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.move_slack_schedule_calendar_cursor(-7, cx)
                } else {
                    self.move_slack_schedule_time_cursor(-1, cx)
                }
            }
            "down" if !event.keystroke.modifiers.modified() => {
                if self.slack_schedule_date_picker_is_open() {
                    self.move_slack_schedule_calendar_cursor(7, cx)
                } else {
                    self.move_slack_schedule_time_cursor(1, cx)
                }
            }
            _ => false,
        };
        stop_slack_schedule_key_event(handled, window, cx);
    }

    pub(in crate::ui::surface::state) fn close_slack_schedule_nested_picker(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .and_then(|overlay| overlay.custom_mut())
        else {
            return false;
        };
        let closed = custom.picker.take().is_some();
        if closed {
            cx.notify();
        }
        closed
    }

    pub(in crate::ui::surface::state) fn slack_schedule_date_picker_is_open(&self) -> bool {
        matches!(
            self.slack_schedule_overlay
                .as_ref()
                .and_then(|overlay| overlay.custom()),
            Some(SlackScheduleCustomState {
                picker: Some(SlackScheduleNestedPicker::Date(_)),
                ..
            })
        )
    }

    pub(in crate::ui::surface::state) fn slack_schedule_time_picker_is_open(&self) -> bool {
        matches!(
            self.slack_schedule_overlay
                .as_ref()
                .and_then(|overlay| overlay.custom()),
            Some(SlackScheduleCustomState {
                picker: Some(SlackScheduleNestedPicker::Time(_)),
                ..
            })
        )
    }
}
