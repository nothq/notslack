use super::{
    parse_slack_schedule_date_input, parse_slack_schedule_time_input, slack_schedule_input_style,
    slack_schedule_time_options, Context, Entity, Rc, SlackScheduleCustomState,
    SlackSchedulePostAt, SurfaceState, TextInput, TextInputAction, TextInputChange, TextInputProps,
    Window,
};

impl SurfaceState {
    pub(crate) fn slack_schedule_date_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let custom = self
            .slack_schedule_custom_state()
            .expect("schedule date input requires the custom schedule dialog");
        let props = TextInputProps::single_line(custom.date_edit_text.clone())
            .style(slack_schedule_input_style())
            .bordered(false)
            .accessibility(
                self.slack_schedule_date_accessibility_id.clone(),
                "Scheduled message date",
            )
            .on_change(self.slack_schedule_date_input_on_change(cx))
            .on_submit(self.slack_schedule_date_input_on_submit(cx))
            .on_escape(self.slack_schedule_date_input_on_escape(cx))
            .on_focus(self.slack_schedule_date_input_on_focus(cx));
        self.slack_schedule_date_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_schedule_date_input.clone()
    }

    pub(crate) fn slack_schedule_time_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let custom = self
            .slack_schedule_custom_state()
            .expect("schedule time input requires the custom schedule dialog");
        let props = TextInputProps::single_line(custom.time_edit_text.clone())
            .style(slack_schedule_input_style())
            .bordered(false)
            .accessibility(
                self.slack_schedule_time_accessibility_id.clone(),
                "Scheduled message time",
            )
            .on_change(self.slack_schedule_time_input_on_change(cx))
            .on_submit(self.slack_schedule_time_input_on_submit(cx))
            .on_escape(self.slack_schedule_time_input_on_escape(cx))
            .on_focus(self.slack_schedule_time_input_on_focus(cx));
        self.slack_schedule_time_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_schedule_time_input.clone()
    }

    pub(in crate::ui::surface::state) fn slack_schedule_date_input_on_change(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.date_edit_text = value;
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_time_input_on_change(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.time_edit_text = value;
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_date_input_on_submit(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.commit_slack_schedule_date_input(cx);
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_time_input_on_submit(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.commit_slack_schedule_time_input(cx);
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_date_input_on_escape(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.date_edit_text.clone_from(&custom.date_label);
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_time_input_on_escape(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.time_edit_text.clone_from(&custom.time_label);
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_date_input_on_focus(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.date_input_focused = true;
                custom.time_input_focused = false;
                custom.picker = None;
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(in crate::ui::surface::state) fn slack_schedule_time_input_on_focus(
        &self,
        cx: &mut Context<Self>,
    ) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                let Some(custom) = surface.slack_schedule_custom_state_mut() else {
                    return;
                };
                custom.date_input_focused = false;
                custom.time_input_focused = true;
                custom.picker = None;
                custom.error = None;
                cx.notify();
            });
        })
    }

    pub(crate) fn ensure_slack_schedule_input_blur_observers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let date_focus = self.slack_schedule_date_input.read(cx).focus_handle_clone();
        let time_focus = self.slack_schedule_time_input.read(cx).focus_handle_clone();
        if self.slack_schedule_custom_state().is_none()
            && (date_focus.is_focused(window) || time_focus.is_focused(window))
        {
            window.focus(&self.focus_handle, cx);
        }
        if self.slack_schedule_input_blur_observers_registered {
            return;
        }
        cx.on_blur(&date_focus, window, |surface, _, cx| {
            surface.commit_slack_schedule_date_input(cx);
            if let Some(custom) = surface.slack_schedule_custom_state_mut() {
                custom.date_input_focused = false;
            }
            cx.notify();
        })
        .detach();
        cx.on_blur(&time_focus, window, |surface, _, cx| {
            surface.commit_slack_schedule_time_input(cx);
            if let Some(custom) = surface.slack_schedule_custom_state_mut() {
                custom.time_input_focused = false;
            }
            cx.notify();
        })
        .detach();
        self.slack_schedule_input_blur_observers_registered = true;
    }

    pub(in crate::ui::surface::state) fn commit_slack_schedule_date_input(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(custom) = self.slack_schedule_custom_state_mut() else {
            return;
        };
        let Some(date) = parse_slack_schedule_date_input(
            &custom.date_edit_text,
            custom.opened_date,
            custom.maximum_date,
        ) else {
            custom.date_edit_text.clone_from(&custom.date_label);
            custom.error = None;
            cx.notify();
            return;
        };
        custom.set_date(date);
        let options = slack_schedule_time_options(custom.timezone, custom.date);
        if !options.iter().any(|option| option.time == custom.time) {
            if let Some(first) = options.first() {
                custom.set_time(first.time);
            } else {
                custom.error =
                    Some("Choose a later date; no future times remain today.".to_string());
            }
        }
        custom.picker = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn commit_slack_schedule_time_input(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(custom) = self.slack_schedule_custom_state_mut() else {
            return;
        };
        let Some(time) = parse_slack_schedule_time_input(&custom.time_edit_text) else {
            custom.time_edit_text.clone_from(&custom.time_label);
            custom.error = None;
            cx.notify();
            return;
        };
        let previous_time = custom.time;
        custom.set_time(time);
        if custom
            .post_at()
            .and_then(SlackSchedulePostAt::future_unix_seconds)
            .is_err()
        {
            custom.set_time(previous_time);
        }
        custom.picker = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn slack_schedule_custom_state(
        &self,
    ) -> Option<&SlackScheduleCustomState> {
        self.slack_schedule_overlay.as_ref()?.custom()
    }

    pub(in crate::ui::surface::state) fn slack_schedule_custom_state_mut(
        &mut self,
    ) -> Option<&mut SlackScheduleCustomState> {
        self.slack_schedule_overlay.as_mut()?.custom_mut()
    }
}
