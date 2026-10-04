use std::rc::Rc;

use gpui::SharedString;
use gpui_components::text_input::{
    TextInputAction, TextInputChange, TextInputEnterBehavior, TextInputMode, TextInputProps,
    TextInputStyle,
};

use super::{
    alpha, px, rgb, slack_palette, Context, SurfaceState, SLACK_COMPOSER_INPUT_HEIGHT,
    SLACK_COMPOSER_LINE_HEIGHT,
};

struct SlackComposerInputCallbacks {
    on_change: TextInputChange,
    on_submit: TextInputAction,
    on_escape: TextInputAction,
    on_focus: TextInputAction,
    on_layout_change: TextInputAction,
}

impl SurfaceState {
    pub(super) fn slack_composer_input_props(
        &self,
        composer_placeholder: &str,
        conversation_label: &str,
        private_channel: bool,
        cx: &mut Context<Self>,
    ) -> TextInputProps {
        let callbacks = self.slack_composer_input_callbacks(cx);
        let palette = slack_palette(self.appearance_mode);
        let highlights = {
            let mut document = self.slack_composer_document.borrow_mut();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            document.highlights(
                rgb(palette.main_text).into(),
                alpha(palette.send_disabled_border, 0.08),
                alpha(palette.send_disabled_border, 0.16),
                rgb(palette.link).into(),
            )
        };
        let accessibility_label = format!("Message to {conversation_label}");
        let placeholder = if private_channel {
            SharedString::default()
        } else {
            SharedString::from(composer_placeholder.to_string())
        };
        TextInputProps::multiline(self.slack_composer_text.clone())
            .placeholder(placeholder)
            .mode(TextInputMode::Multiline {
                max_visible_lines: Some(self.slack_composer_max_visible_lines()),
            })
            .style(self.slack_composer_input_style())
            .bordered(false)
            .disabled(!self.can_mutate_current_slack_send_draft())
            .request_focus(self.slack_composer_focused)
            .highlights(highlights)
            .enter_behavior(TextInputEnterBehavior::SubmitOnEnter)
            .accessibility(
                self.slack_composer_accessibility_id.clone(),
                accessibility_label,
            )
            .on_change(callbacks.on_change)
            .on_submit(callbacks.on_submit)
            .on_escape(callbacks.on_escape)
            .on_focus(callbacks.on_focus)
            .on_layout_change(callbacks.on_layout_change)
    }

    fn slack_composer_input_callbacks(
        &self,
        cx: &mut Context<Self>,
    ) -> SlackComposerInputCallbacks {
        SlackComposerInputCallbacks {
            on_change: Self::slack_composer_on_change(cx),
            on_submit: Self::slack_composer_on_submit(cx),
            on_escape: Self::slack_composer_on_escape(cx),
            on_focus: Self::slack_composer_on_focus(cx),
            on_layout_change: Self::slack_composer_on_layout_change(cx),
        }
    }

    fn slack_composer_on_change(cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if !surface.can_mutate_current_slack_send_draft()
                        || surface.slack_composer_text == value
                    {
                        return;
                    }
                    let private_placeholder_changed = surface
                        .slack_active_main_composer_context
                        .as_ref()
                        .is_some_and(|context| context.presentation.private_channel)
                        && surface.slack_composer_text.is_empty() != value.is_empty();
                    let send_controls_changed =
                        surface.slack_composer_text.trim().is_empty() != value.trim().is_empty();
                    let focus_changed = !surface.slack_composer_focused;
                    let error_changed = surface.slack_error.is_some();
                    let toolbar_picker_changed = surface.clear_slack_toolbar_mention_picker(
                        &super::super::SlackComposerTarget::Main,
                    );
                    let document_had_or_has_rendered_highlights =
                        surface.update_slack_composer_document(&value);
                    surface.replace_slack_send_draft_text(value);
                    surface.slack_main_composer_draft_changed(cx);
                    surface.slack_composer_focused = true;
                    surface.slack_error = None;
                    if private_placeholder_changed
                        || send_controls_changed
                        || focus_changed
                        || error_changed
                        || toolbar_picker_changed
                        || surface.slack_formatting_enabled
                        || document_had_or_has_rendered_highlights
                    {
                        cx.notify();
                    }
                })
                .ok();
        })
    }

    fn update_slack_composer_document(&self, value: &str) -> bool {
        let mut document = self.slack_composer_document.borrow_mut();
        let had_rendered_highlights = document.has_rendered_highlights();
        document.apply_text_edit(value);
        had_rendered_highlights || document.has_rendered_highlights()
    }

    fn slack_composer_on_submit(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if !surface.activate_first_slack_composer_aux_action_if_open(
                        &super::super::SlackComposerTarget::Main,
                        cx,
                    ) {
                        surface.submit_slack_composer(cx);
                    }
                })
                .ok();
        })
    }

    fn slack_composer_on_escape(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    if surface.slack_formatting_enabled {
                        surface.slack_formatting_enabled = false;
                        surface.slack_composer_focused = true;
                        let focus = surface.slack_composer_input.read(cx).focus_handle_clone();
                        cx.notify();
                        return Some(focus);
                    }
                    surface.handle_slack_escape_key(cx);
                    (!surface.slack_composer_focused).then(|| surface.focus_handle.clone())
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    fn slack_composer_on_focus(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.focus_slack_composer(cx);
                })
                .ok();
        })
    }

    fn slack_composer_on_layout_change(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |_surface, cx| {
                    cx.notify();
                })
                .ok();
        })
    }

    fn slack_composer_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(SLACK_COMPOSER_INPUT_HEIGHT),
            min_height: px(SLACK_COMPOSER_INPUT_HEIGHT),
            padding_x: px(12.0),
            padding_y: px(8.0),
            radius: px(7.0),
            background: rgb(palette.composer_bg).into(),
            border: alpha(palette.composer_border, 0.0),
            focused_border: alpha(palette.composer_focused_border, 0.0),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.composer_placeholder).into(),
            selection: alpha(0x1264a3, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(SLACK_COMPOSER_LINE_HEIGHT),
            font_family: Some("Lato".into()),
        }
    }
}
