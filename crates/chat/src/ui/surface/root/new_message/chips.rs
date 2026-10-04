use super::super::super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState, Window,
};
use super::slack_new_message_action_key;
use crate::ui::surface::SlackNewMessagePerson;
use gpui::Role;

pub(super) struct SlackNewMessageChipSpec {
    pub(super) element_id: String,
    pub(super) label: gpui::SharedString,
    pub(super) id: String,
    pub(super) is_destination: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_new_message_person_chip(
        &self,
        person: &SlackNewMessagePerson,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_new_message_chip(
            SlackNewMessageChipSpec {
                element_id: format!("slack-new-message-person-chip-{}", person.user_id),
                label: person.label.clone(),
                id: person.user_id.to_string(),
                is_destination: false,
            },
            cx,
        )
    }

    fn render_slack_new_message_chip(
        &self,
        spec: SlackNewMessageChipSpec,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let recipient_label = spec.label.clone();
        div()
            .id(spec.element_id)
            .h(px(26.0))
            .max_w(px(260.0))
            .flex_none()
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(palette.composer_chip_border))
            .bg(rgb(palette.composer_chip_bg))
            .flex()
            .items_center()
            .overflow_hidden()
            .child(slack_new_message_chip_label(
                spec.label,
                palette.composer_chip_text,
            ))
            .child(self.render_slack_new_message_chip_remove(
                spec.id,
                spec.is_destination,
                recipient_label,
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_new_message_chip_remove(
        &self,
        id: String,
        is_destination: bool,
        recipient_label: gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let keyboard_id = id.clone();
        div()
            .id(if is_destination {
                "slack-new-message-clear-destination".to_string()
            } else {
                format!("slack-new-message-remove-{id}")
            })
            .role(Role::Button)
            .aria_label(if is_destination {
                "Remove destination".to_string()
            } else {
                format!("Remove recipient {recipient_label}")
            })
            .focusable()
            .tab_stop(true)
            .w(px(26.0))
            .h_full()
            .flex_none()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(alpha(palette.main_text, 0.08)))
            .child(slack_icon(
                SlackShellIcon::Close,
                palette.main_secondary_text,
                12.0,
                cx,
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.remove_slack_new_message_chip(&id, is_destination, window, cx);
                }),
            )
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_new_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.remove_slack_new_message_chip(&keyboard_id, is_destination, window, cx);
                }
            }))
    }

    fn remove_slack_new_message_chip(
        &mut self,
        id: &str,
        is_destination: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if is_destination {
            self.clear_slack_new_message_destination(cx);
        } else {
            self.remove_slack_new_message_person(id, cx);
        }
        let focus = self
            .slack_new_message_to_input
            .read(cx)
            .focus_handle_clone();
        window.focus(&focus, cx);
    }
}

fn slack_new_message_chip_label(label: gpui::SharedString, text_color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .pl(px(8.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(14.0))
        .line_height(px(20.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(text_color))
        .child(label)
}
