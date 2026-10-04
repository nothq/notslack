use gpui::{
    div, px, rgb, Context, Div, FocusHandle, FontWeight, InteractiveElement, KeyDownEvent,
    ParentElement, Role, Stateful, StatefulInteractiveElement, Styled, Window,
};

use crate::ui::alpha;

use super::{super::SlackVideoClipModal, SlackVideoClipButtonAction, SlackVideoClipButtonSpec};

impl SlackVideoClipModal {
    pub(super) fn secondary_button(
        &self,
        spec: SlackVideoClipButtonSpec,
        focus_handle: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.button_shell(spec, focus_handle)
            .border_1()
            .border_color(rgb(0x8b8d8f))
            .hover(|style| style.bg(alpha(0xffffff, 0.06)))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                (spec.action)(this, window, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                activate_button_from_key(this, event, window, cx, spec.action);
            }))
    }

    pub(super) fn primary_button(
        &self,
        spec: SlackVideoClipButtonSpec,
        focus_handle: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.button_shell(spec, focus_handle)
            .bg(rgb(0x007a5a))
            .hover(|style| style.bg(rgb(0x148567)))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                (spec.action)(this, window, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                activate_button_from_key(this, event, window, cx, spec.action);
            }))
    }

    pub(super) fn danger_button(
        &self,
        spec: SlackVideoClipButtonSpec,
        focus_handle: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        self.button_shell(spec, focus_handle)
            .bg(rgb(0xe01e5a))
            .hover(|style| style.bg(rgb(0xc9184f)))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                (spec.action)(this, window, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                activate_button_from_key(this, event, window, cx, spec.action);
            }))
    }

    fn button_shell(
        &self,
        spec: SlackVideoClipButtonSpec,
        focus_handle: &FocusHandle,
    ) -> Stateful<Div> {
        div()
            .id(spec.id)
            .role(Role::Button)
            .aria_label(spec.aria_label)
            .h(px(36.0))
            .px(px(16.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .focusable()
            .track_focus(focus_handle)
            .tab_index(spec.tab_index)
            .tab_stop(true)
            .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
            .child(spec.label)
    }
}

fn activate_button_from_key(
    this: &mut SlackVideoClipModal,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<SlackVideoClipModal>,
    action: SlackVideoClipButtonAction,
) {
    if event.keystroke.modifiers.modified()
        || !matches!(event.keystroke.key.as_str(), "enter" | "space")
    {
        return;
    }
    window.prevent_default();
    cx.stop_propagation();
    action(this, window, cx);
}
