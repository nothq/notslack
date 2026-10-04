use crate::ui::surface::{
    slack_icon, slack_palette, SlackComposerFormatAction, SlackPalette, SlackReplyComposerTarget,
    SlackThreadPanelState, SurfaceState,
};
use crate::ui::{
    alpha, div, px, rgb, AnyElement, Context, Div, FluentBuilder, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled, Window,
};
use gpui::{AppContext, Render, Role, Stateful, Toggled};

use super::SLACK_THREAD_COMPOSER_FORMAT_HEIGHT;
use controls::{
    slack_reply_format_bar_id, slack_reply_format_control_id, slack_thread_block_format_controls,
    slack_thread_inline_format_controls, SlackThreadFormatControl,
};

mod controls;

struct SlackReplyFormatRenderContext<'a> {
    target: &'a SlackReplyComposerTarget,
    target_enabled: bool,
    roving_target: SlackComposerFormatAction,
}

struct SlackReplyFormatState {
    active: [bool; 10],
    link_enabled: bool,
    roving_target: SlackComposerFormatAction,
}

struct SlackReplyFormatKeyboardTarget {
    composer_target: SlackReplyComposerTarget,
    action: SlackComposerFormatAction,
    enabled: bool,
}

struct SlackReplyFormatButtonBinding {
    target: SlackReplyComposerTarget,
    action: SlackComposerFormatAction,
    enabled: bool,
    palette: SlackPalette,
}

struct SlackThreadLinkTooltip;

impl Render for SlackThreadLinkTooltip {
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
    pub(super) fn render_slack_thread_format_bar(
        &self,
        panel: &SlackThreadPanelState,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let target = SlackReplyComposerTarget::ThreadPanel {
            panel_generation: panel.generation,
            parent_message_id: panel.parent_message_id.clone().into(),
            draft_key: panel.reply_draft_key.clone(),
        };
        self.render_slack_reply_format_bar(&target, enabled, cx)
    }

    pub(in crate::ui::surface) fn render_slack_reply_format_bar(
        &self,
        target: &SlackReplyComposerTarget,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.slack_reply_format_state(target, cx);
        let render_context = SlackReplyFormatRenderContext {
            target,
            target_enabled: enabled,
            roving_target: state.roving_target,
        };
        div()
            .id(slack_reply_format_bar_id(target))
            .role(Role::Toolbar)
            .aria_label("Formatting")
            .h(px(SLACK_THREAD_COMPOSER_FORMAT_HEIGHT))
            .flex_none()
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(13.0))
            .child(self.render_slack_reply_format_group(
                slack_thread_inline_format_controls(&state.active),
                &render_context,
                cx,
            ))
            .child(self.render_slack_reply_format_group(
                slack_thread_block_format_controls(&state.active, state.link_enabled),
                &render_context,
                cx,
            ))
    }

    fn slack_reply_format_state(
        &self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) -> SlackReplyFormatState {
        let draft_text = match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .expect("rendered Slack thread format bar lost its panel")
                .reply_draft
                .borrow()
                .text()
                .to_string(),
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => self
                .slack_composer_drafts
                .get(draft_key)
                .map(|draft| draft.text().to_string())
                .unwrap_or_default(),
        };
        let selection = {
            let input = self
                .slack_reply_composer_input(target)
                .expect("rendered Slack reply format bar must retain its input");
            let input = input.read(cx);
            if input.text() == draft_text {
                input.selection_range()
            } else {
                draft_text.len()..draft_text.len()
            }
        };
        let active = match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                let draft = self
                    .slack_thread_panel
                    .as_ref()
                    .expect("rendered Slack thread format bar lost its panel")
                    .reply_draft
                    .borrow();
                SlackComposerFormatAction::TOOLBAR_CONTROLS
                    .map(|action| draft.document.format_is_active(action, &selection))
            }
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => self
                .slack_composer_drafts
                .get(draft_key)
                .map_or([false; 10], |draft| {
                    SlackComposerFormatAction::TOOLBAR_CONTROLS
                        .map(|action| draft.document.format_is_active(action, &selection))
                }),
        };
        let roving_target = self
            .slack_reply_format_roving_target(target)
            .expect("rendered Slack reply format bar must retain its roving target");
        SlackReplyFormatState {
            active,
            link_enabled: !selection.is_empty(),
            roving_target,
        }
    }

    fn render_slack_reply_format_group<const N: usize>(
        &self,
        controls: [SlackThreadFormatControl; N],
        render_context: &SlackReplyFormatRenderContext<'_>,
        cx: &mut Context<Self>,
    ) -> Div {
        div().flex().items_center().gap(px(4.0)).children(
            controls
                .into_iter()
                .map(|control| self.render_slack_reply_format_button(control, render_context, cx)),
        )
    }

    fn render_slack_reply_format_button(
        &self,
        control: SlackThreadFormatControl,
        render_context: &SlackReplyFormatRenderContext<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackThreadFormatControl {
            action,
            icon,
            accessibility_label,
            active,
            enabled: control_enabled,
        } = control;
        let enabled = render_context.target_enabled && control_enabled;
        let palette = slack_palette(self.appearance_mode);
        let icon_fill = slack_thread_format_icon_fill(active, enabled, palette);
        let focus = self
            .slack_reply_format_focus_handle(render_context.target, action)
            .expect("rendered Slack reply format control must retain its focus handle");
        let keyboard_target = SlackReplyFormatKeyboardTarget {
            composer_target: render_context.target.clone(),
            action,
            enabled,
        };
        let button = div()
            .id(slack_reply_format_control_id(render_context.target, action))
            .role(Role::Button)
            .aria_label(accessibility_label)
            .aria_toggled(slack_reply_format_toggled(active))
            .track_focus(&focus)
            .focusable()
            .tab_stop(render_context.roving_target == action)
            .size(px(28.0))
            .rounded(px(4.0))
            .bg(if active {
                alpha(palette.composer_focused_border, 0.35)
            } else {
                alpha(0xffffff, 0.0)
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, icon_fill, 20.0, cx))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_reply_format_roving_key(&keyboard_target, event, window, cx);
            }));
        bind_slack_reply_format_button(
            button,
            SlackReplyFormatButtonBinding {
                target: render_context.target.clone(),
                action,
                enabled,
                palette,
            },
            cx,
        )
    }

    fn handle_slack_reply_format_roving_key(
        &mut self,
        target: &SlackReplyFormatKeyboardTarget,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SlackReplyFormatKeyboardTarget {
            composer_target,
            action,
            enabled,
        } = target;
        if event.keystroke.modifiers.modified() {
            return;
        }
        let next = match event.keystroke.key.as_str() {
            "left" | "arrowleft" => Some(slack_previous_format_action(*action)),
            "right" | "arrowright" => Some(slack_next_format_action(*action)),
            "enter" | "space" => {
                window.prevent_default();
                cx.stop_propagation();
                if *enabled {
                    self.apply_slack_reply_format(composer_target, *action, cx);
                    if *action != SlackComposerFormatAction::Link {
                        let focus = self
                            .slack_reply_format_focus_handle(composer_target, *action)
                            .expect("formatted Slack reply lost its focus handle");
                        window.focus(&focus, cx);
                    }
                }
                return;
            }
            _ => return,
        };
        let next = next.expect("Slack thread composer roving key must resolve a toolbar target");
        self.set_slack_reply_format_roving_target(composer_target, next);
        let focus = self
            .slack_reply_format_focus_handle(composer_target, next)
            .expect("Slack reply lost its roving focus handle");
        window.prevent_default();
        window.focus(&focus, cx);
        cx.stop_propagation();
        cx.notify();
    }
}

fn slack_reply_format_toggled(active: bool) -> Toggled {
    if active {
        Toggled::True
    } else {
        Toggled::False
    }
}

fn bind_slack_reply_format_button(
    button: Stateful<Div>,
    binding: SlackReplyFormatButtonBinding,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let SlackReplyFormatButtonBinding {
        target,
        action,
        enabled,
        palette,
    } = binding;
    if !enabled {
        return button.into_any_element();
    }
    button
        .cursor_pointer()
        .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
        .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
        .when(action == SlackComposerFormatAction::Link, |this| {
            this.tooltip(|_, cx| cx.new(|_| SlackThreadLinkTooltip).into())
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.apply_slack_reply_format(&target, action, cx);
        }))
        .into_any_element()
}

fn slack_thread_format_icon_fill(active: bool, enabled: bool, palette: SlackPalette) -> u32 {
    if active && enabled {
        0xffffff
    } else if enabled {
        palette.composer_icon
    } else {
        palette.send_disabled_icon
    }
}

fn slack_previous_format_action(action: SlackComposerFormatAction) -> SlackComposerFormatAction {
    let controls = SlackComposerFormatAction::TOOLBAR_CONTROLS;
    controls[(action.toolbar_index() + controls.len() - 1) % controls.len()]
}

fn slack_next_format_action(action: SlackComposerFormatAction) -> SlackComposerFormatAction {
    let controls = SlackComposerFormatAction::TOOLBAR_CONTROLS;
    controls[(action.toolbar_index() + 1) % controls.len()]
}
