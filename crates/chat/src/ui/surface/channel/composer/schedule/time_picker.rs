use super::{
    alpha, div, point, px, rgb, AnyElement, BoxShadow, Context, FluentBuilder, InteractiveElement,
    IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement,
    SlackScheduleTimeOptionContext, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_SCHEDULE_TIME_OPTION_HEIGHT, SLACK_SCHEDULE_TIME_PICKER_HEIGHT,
    SLACK_SCHEDULE_TIME_PICKER_LEFT, SLACK_SCHEDULE_TIME_PICKER_TOP,
    SLACK_SCHEDULE_TIME_PICKER_WIDTH,
};
use crate::ui::surface::{
    SlackScheduleCustomState, SlackScheduleTimeOption, SlackScheduleTimePickerState,
};
use gpui::{uniform_list, Role};

impl SurfaceState {
    pub(super) fn render_slack_schedule_time_picker(
        &self,
        _custom: &SlackScheduleCustomState,
        picker: &SlackScheduleTimePickerState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("slack-schedule-time-picker")
            .role(Role::ListBox)
            .aria_label("Choose a scheduled time")
            .focusable()
            .tab_stop(false)
            .absolute()
            .left(px(SLACK_SCHEDULE_TIME_PICKER_LEFT))
            .top(px(SLACK_SCHEDULE_TIME_PICKER_TOP))
            .w(px(SLACK_SCHEDULE_TIME_PICKER_WIDTH))
            .h(px(SLACK_SCHEDULE_TIME_PICKER_HEIGHT))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(0x34373b))
            .bg(rgb(0x1a1d21))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(12.0)),
                blur_radius: px(32.0),
                spread_radius: px(-8.0),
                inset: false,
            }])
            .occlude()
            .overflow_hidden()
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(Self::handle_slack_schedule_picker_key_down))
            .child(self.render_slack_schedule_time_options(picker, cx))
            .into_any_element()
    }

    fn render_slack_schedule_time_options(
        &self,
        picker: &SlackScheduleTimePickerState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity();
        let options = picker.options.clone();
        let option_count = options.len();
        let selected_index = picker.selected_index;
        let cursor_index = picker.cursor_index;
        uniform_list(
            "slack-schedule-time-options",
            option_count,
            move |range, _window, cx| {
                let options = options.clone();
                view.update(cx, |this, cx| {
                    range
                        .map(|option_index| {
                            let option = options
                                .get(option_index)
                                .expect("Slack schedule time option index must exist");
                            this.render_slack_schedule_time_option(
                                option,
                                SlackScheduleTimeOptionContext {
                                    index: option_index,
                                    selected_index,
                                    cursor_index,
                                    option_count,
                                },
                                cx,
                            )
                        })
                        .collect::<Vec<_>>()
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .track_scroll(&picker.scroll_handle)
        .size_full()
    }

    fn render_slack_schedule_time_option(
        &self,
        option: &SlackScheduleTimeOption,
        context: SlackScheduleTimeOptionContext,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = context.selected_index == Some(context.index);
        let cursor = context.cursor_index == context.index;
        div()
            .id(("slack-schedule-time-option", context.index))
            .role(Role::ListBoxOption)
            .aria_label(option.label.clone())
            .aria_selected(selected)
            .aria_position_in_set(context.index + 1)
            .aria_size_of_set(context.option_count)
            .focusable()
            .tab_stop(cursor)
            .h(px(SLACK_SCHEDULE_TIME_OPTION_HEIGHT))
            .w_full()
            .flex_none()
            .bg(rgb(if selected {
                0x1264a3
            } else if cursor {
                0x34373b
            } else {
                0x1a1d21
            }))
            .cursor_pointer()
            .when(!selected, |this| {
                this.hover(|style| style.bg(rgb(0x34373b)))
                    .focus_visible(|style| style.bg(rgb(0x34373b)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_schedule_time(context.index, cx);
            }))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(0xf8f8f8))
            .child(
                div()
                    .w(px(28.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if selected { "✓" } else { "" }),
            )
            .child(option.label.clone())
    }
}
