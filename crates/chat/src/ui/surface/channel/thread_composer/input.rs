use std::rc::Rc;

use crate::ui::surface::{slack_palette, SlackThreadPanelState, SurfaceState};
use crate::ui::{alpha, px, rgb, Context};
use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputEnterBehavior, TextInputMode,
    TextInputProps, TextInputStyle,
};

use super::{SLACK_THREAD_COMPOSER_INPUT_HEIGHT, SLACK_THREAD_COMPOSER_LINE_HEIGHT};

impl SurfaceState {
    pub(super) fn render_slack_thread_composer_input(
        &self,
        panel: &SlackThreadPanelState,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let props = self.slack_thread_composer_input_props(panel, enabled, cx);
        self.slack_thread_composer_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_thread_composer_input.clone()
    }

    fn slack_thread_composer_input_props(
        &self,
        panel: &SlackThreadPanelState,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> TextInputProps {
        let palette = slack_palette(self.appearance_mode);
        let (draft_text, highlights) = {
            let mut draft = panel.reply_draft.borrow_mut();
            let draft_text = draft.text().to_string();
            let highlights = draft.document.highlights(
                rgb(palette.main_text).into(),
                alpha(palette.send_disabled_border, 0.08),
                alpha(palette.send_disabled_border, 0.16),
                rgb(palette.link).into(),
            );
            (draft_text, highlights)
        };
        TextInputProps::multiline(draft_text)
            .placeholder("Reply…")
            .mode(TextInputMode::Multiline {
                max_visible_lines: Some(3),
            })
            .style(self.slack_thread_composer_input_style())
            .bordered(false)
            .disabled(!enabled)
            .request_focus(panel.reply_composer_focused)
            .highlights(highlights)
            .enter_behavior(TextInputEnterBehavior::SubmitOnEnter)
            .accessibility(
                self.slack_thread_composer_accessibility_id.clone(),
                "Reply to thread",
            )
            .on_change(Self::slack_thread_input_on_change(cx))
            .on_submit(Self::slack_thread_input_on_submit(cx))
            .on_escape(Self::slack_thread_input_on_escape(cx))
            .on_focus(Self::slack_thread_input_on_focus(cx))
    }

    fn slack_thread_input_on_change(cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_thread_reply_text(value, cx);
                })
                .ok();
        })
    }

    fn slack_thread_input_on_submit(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    let target = surface
                        .slack_thread_panel_reply_composer_target()
                        .map(super::super::SlackComposerTarget::Reply);
                    if target.as_ref().is_none_or(|target| {
                        !surface.activate_first_slack_composer_aux_action_if_open(target, cx)
                    }) {
                        surface.send_slack_thread_reply(cx);
                    }
                })
                .ok();
        })
    }

    fn slack_thread_input_on_escape(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.handle_slack_escape_key(cx);
                    surface
                        .slack_thread_panel
                        .as_ref()
                        .is_none_or(|panel| !panel.reply_composer_focused)
                        .then(|| surface.focus_handle.clone())
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    fn slack_thread_input_on_focus(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if !surface.slack_workspace_api_capabilities.send_thread_reply {
                        return;
                    }
                    let Some(panel) = surface.slack_thread_panel.as_mut() else {
                        return;
                    };
                    panel.reply_composer_focused = true;
                    if panel.reply_error.take().is_some() {
                        panel.list_state.remeasure();
                    }
                    cx.notify();
                })
                .ok();
        })
    }

    fn slack_thread_composer_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(SLACK_THREAD_COMPOSER_INPUT_HEIGHT),
            min_height: px(SLACK_THREAD_COMPOSER_INPUT_HEIGHT),
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
            line_height: px(SLACK_THREAD_COMPOSER_LINE_HEIGHT),
            font_family: Some("Lato".into()),
        }
    }
}
