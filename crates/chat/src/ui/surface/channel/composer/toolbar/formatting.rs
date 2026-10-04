use gpui::{AppContext, KeyDownEvent, Render, Toggled, Window};

use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, ParentElement, SlackComposerFormatAction, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::Role;

mod specs;

use specs::{
    slack_composer_format_control_id, slack_composer_primary_format_controls,
    slack_composer_secondary_format_controls, SlackComposerFormatControl,
};

struct SlackComposerLinkTooltip;

impl Render for SlackComposerLinkTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(107.0))
            .h(px(66.0))
            .px(px(12.0))
            .py(px(10.0))
            .rounded(px(8.0))
            .bg(rgb(0x1d1c1d))
            .shadow_lg()
            .flex()
            .flex_col()
            .justify_between()
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(18.0))
                    .child("Link"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(rgb(0xd1d2d3))
                    .child("⌘ Shift U"),
            )
    }
}

impl SurfaceState {
    pub(super) fn render_slack_format_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let active = self.slack_formatting_enabled;
        let accessibility_label = if active {
            "Hide formatting"
        } else {
            "Show formatting"
        };
        if !self.can_mutate_current_slack_send_draft() {
            return self.render_disabled_slack_format_toggle(accessibility_label, active, cx);
        }
        div()
            .id("slack-composer-format")
            .role(Role::Button)
            .aria_label(accessibility_label)
            .aria_toggled(slack_format_toggle_state(active))
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_formatting(cx);
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::FormatToggle,
                palette.composer_icon,
                18.0,
                cx,
            ))
            .into_any_element()
    }

    fn render_disabled_slack_format_toggle(
        &self,
        accessibility_label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-format")
            .role(Role::Button)
            .aria_label(accessibility_label)
            .aria_toggled(slack_format_toggle_state(active))
            .size(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(palette.send_disabled_icon))
            .opacity(0.45)
            .child(slack_icon(
                SlackShellIcon::FormatToggle,
                palette.send_disabled_icon,
                18.0,
                cx,
            ))
            .into_any_element()
    }

    pub(crate) fn render_slack_composer_format_bar(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (active, link_enabled) = self.slack_composer_format_state(cx);
        let primary = slack_composer_primary_format_controls(active);
        let secondary = slack_composer_secondary_format_controls(active, link_enabled);
        div()
            .id("slack-main-composer-formatting")
            .role(Role::Toolbar)
            .aria_label("Formatting")
            .h(px(38.0))
            .flex_none()
            .px(px(6.0))
            .flex()
            .items_center()
            .gap(px(13.0))
            .child(self.render_slack_composer_format_group(primary, cx))
            .child(self.render_slack_composer_format_group(secondary, cx))
    }

    fn slack_composer_format_state(
        &self,
        cx: &mut Context<Self>,
    ) -> (
        [bool; SlackComposerFormatAction::TOOLBAR_CONTROLS.len()],
        bool,
    ) {
        let selection = {
            let input = self.slack_composer_input.read(cx);
            if input.text() == self.slack_composer_text {
                input.selection_range()
            } else {
                self.slack_composer_text.len()..self.slack_composer_text.len()
            }
        };
        let active = {
            let mut document = self.slack_composer_document.borrow_mut();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            SlackComposerFormatAction::TOOLBAR_CONTROLS
                .map(|action| document.format_is_active(action, &selection))
        };
        (active, !selection.is_empty())
    }

    fn render_slack_composer_format_group<const N: usize>(
        &self,
        controls: [SlackComposerFormatControl; N],
        cx: &mut Context<Self>,
    ) -> Div {
        div().flex().items_center().gap(px(4.0)).children(
            controls
                .into_iter()
                .map(|control| self.render_slack_composer_format_button(control, cx)),
        )
    }

    fn render_slack_composer_format_button(
        &self,
        control: SlackComposerFormatControl,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let action = control.action;
        if self.slack_schedule_blocks_current_composer_mutation() {
            return self.render_disabled_slack_composer_format_button(control, cx);
        }
        let enabled = control.enabled;
        let button = div()
            .id(slack_composer_format_control_id(action))
            .role(Role::Button)
            .aria_label(control.accessibility_label)
            .aria_toggled(slack_format_toggle_state(control.active))
            .track_focus(&self.slack_composer_format_focus_handles[action.toolbar_index()])
            .focusable()
            .tab_stop(self.slack_composer_format_roving_target == action)
            .size(px(28.0))
            .rounded(px(4.0))
            .bg(if control.active {
                alpha(palette.composer_focused_border, 0.35)
            } else {
                alpha(0xffffff, 0.0)
            })
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_composer_format_roving_key((action, enabled), event, window, cx);
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                control.icon,
                slack_composer_format_icon_color(control, palette),
                20.0,
                cx,
            ));
        if !enabled {
            return button.into_any_element();
        }
        button
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .when(action == SlackComposerFormatAction::Link, |this| {
                this.tooltip(|_, cx| cx.new(|_| SlackComposerLinkTooltip).into())
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.apply_slack_composer_format(action, cx);
            }))
            .into_any_element()
    }

    fn render_disabled_slack_composer_format_button(
        &self,
        control: SlackComposerFormatControl,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(slack_composer_format_control_id(control.action))
            .role(Role::Button)
            .aria_label(control.accessibility_label)
            .aria_toggled(slack_format_toggle_state(control.active))
            .size(px(28.0))
            .rounded(px(4.0))
            .opacity(0.45)
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                control.icon,
                palette.send_disabled_icon,
                20.0,
                cx,
            ))
            .into_any_element()
    }

    fn handle_slack_composer_format_roving_key(
        &mut self,
        target: (SlackComposerFormatAction, bool),
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (action, enabled) = target;
        if event.keystroke.modifiers.modified() {
            return;
        }
        let next = match event.keystroke.key.as_str() {
            "left" | "arrowleft" => {
                let current = action.toolbar_index();
                Some(
                    SlackComposerFormatAction::TOOLBAR_CONTROLS[(current
                        + SlackComposerFormatAction::TOOLBAR_CONTROLS.len()
                        - 1)
                        % SlackComposerFormatAction::TOOLBAR_CONTROLS.len()],
                )
            }
            "right" | "arrowright" => Some(
                SlackComposerFormatAction::TOOLBAR_CONTROLS[(action.toolbar_index() + 1)
                    % SlackComposerFormatAction::TOOLBAR_CONTROLS.len()],
            ),
            "enter" | "space" => {
                window.prevent_default();
                cx.stop_propagation();
                if enabled {
                    self.apply_slack_composer_format(action, cx);
                    if action != SlackComposerFormatAction::Link {
                        self.slack_composer_focused = false;
                        let focus = self.slack_composer_format_focus_handles
                            [action.toolbar_index()]
                        .clone();
                        window.focus(&focus, cx);
                    }
                }
                return;
            }
            _ => return,
        };
        let next = next.expect("Slack composer roving key must resolve a toolbar target");
        self.slack_composer_format_roving_target = next;
        let focus = self.slack_composer_format_focus_handles[next.toolbar_index()].clone();
        window.prevent_default();
        window.focus(&focus, cx);
        cx.stop_propagation();
        cx.notify();
    }
}

fn slack_format_toggle_state(active: bool) -> Toggled {
    if active {
        Toggled::True
    } else {
        Toggled::False
    }
}

fn slack_composer_format_icon_color(
    control: SlackComposerFormatControl,
    palette: super::super::super::SlackPalette,
) -> u32 {
    if control.active && control.enabled {
        0xffffff
    } else if control.enabled {
        palette.composer_icon
    } else {
        palette.send_disabled_icon
    }
}
