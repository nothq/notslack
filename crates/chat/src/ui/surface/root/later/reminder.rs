use gpui::{
    div, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::alpha;
use crate::ui::surface::SurfaceState;

mod buttons;
mod dialog;

const SLACK_REMINDER_HOUR: u64 = 60 * 60;
const SLACK_REMINDER_DAY: u64 = 24 * SLACK_REMINDER_HOUR;
const SLACK_REMINDER_WEEK: u64 = 7 * SLACK_REMINDER_DAY;

impl SurfaceState {
    pub(in crate::ui::surface::root::later) fn render_slack_later_new_reminder_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-later-new-reminder")
            .role(Role::Button)
            .aria_label("New reminder")
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(11.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(0x565856))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2b2d31)))
            .focus_visible(|style| style.bg(rgb(0x2b2d31)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_later_reminder_create_dialog(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if reminder_action_key(event) {
                    cx.stop_propagation();
                    this.open_slack_later_reminder_create_dialog(cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xf8f8f8))
            .child("New reminder")
    }

    pub(in crate::ui::surface::root::later) fn render_slack_later_reminder_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog = self
            .slack_later_reminder_dialog
            .as_ref()
            .expect("Slack reminder layer requires dialog state");
        div()
            .id("slack-later-reminder-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .focusable()
            .track_focus(&self.slack_later_reminder_layer_focus_handle)
            .tab_index(0)
            .tab_group()
            .tab_stop(false)
            .flex()
            .items_center()
            .justify_center()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_later_reminder_layer_key(event, window, cx);
            }))
            .child(self.render_slack_later_reminder_backdrop(cx))
            .child(self.render_slack_later_reminder_focus_guard(
                "slack-later-reminder-focus-start",
                &self.slack_later_reminder_focus_guard_start,
                0,
            ))
            .child(self.render_slack_later_reminder_dialog(dialog, cx))
            .child(self.render_slack_later_reminder_focus_guard(
                "slack-later-reminder-focus-end",
                &self.slack_later_reminder_focus_guard_end,
                100,
            ))
            .into_any_element()
    }

    fn render_slack_later_reminder_backdrop(&self, cx: &mut Context<Self>) -> impl IntoElement {
        dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, window, cx| {
                this.dismiss_slack_later_reminder_dialog(window, cx);
            }),
            cx,
        )
    }

    fn render_slack_later_reminder_focus_guard(
        &self,
        id: &'static str,
        focus_handle: &gpui::FocusHandle,
        tab_index: isize,
    ) -> impl IntoElement {
        div()
            .id(id)
            .absolute()
            .size(px(0.0))
            .overflow_hidden()
            .focusable()
            .track_focus(focus_handle)
            .tab_index(tab_index)
            .tab_stop(true)
    }

    fn handle_slack_later_reminder_layer_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        window.prevent_default();
        self.dismiss_slack_later_reminder_dialog(window, cx);
    }

    fn dismiss_slack_later_reminder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_slack_later_reminder_dialog(cx);
        if self.slack_later_reminder_dialog.is_none() {
            window.focus(&self.focus_handle, cx);
        }
    }
}

fn reminder_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
