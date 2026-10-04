use std::rc::Rc;

use super::SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT;
use crate::ui::surface::{
    alpha, rgb, slack_palette, Context, SlackAllThreadRow, SlackComposerTarget,
    SlackReplyComposerTarget, SurfaceState, SLACK_COMPOSER_INPUT_HEIGHT,
};
use gpui::px;
use gpui_components::text_input::{
    TextInputAction, TextInputChange, TextInputEnterBehavior, TextInputMode, TextInputProps,
    TextInputStyle,
};

impl SurfaceState {
    pub(super) fn slack_all_threads_composer_input_props(
        &self,
        row: &SlackAllThreadRow,
        target: SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) -> TextInputProps {
        let thread_key = target
            .all_threads_key()
            .expect("All Threads input target must retain its thread key")
            .clone();
        let draft_state = self.slack_composer_drafts.get(target.draft_key());
        let draft = draft_state
            .map(|draft| draft.text().to_string())
            .unwrap_or_default();
        let enabled = self.slack_workspace_api_capabilities.send_thread_reply
            && !self.slack_thread_reply_mutation_is_blocked(target.draft_key());
        let palette = slack_palette(self.appearance_mode);
        let highlights = draft_state.map(|draft| {
            let mut document = draft.document.clone();
            document.highlights(
                rgb(palette.main_text).into(),
                alpha(palette.send_disabled_border, 0.08),
                alpha(palette.send_disabled_border, 0.16),
                rgb(palette.link).into(),
            )
        });
        let input_height = SLACK_COMPOSER_INPUT_HEIGHT
            + if row.broadcast_label.is_some() {
                0.0
            } else {
                SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT
            };
        let props = TextInputProps::multiline(draft)
            .placeholder("Reply…")
            .mode(TextInputMode::Multiline {
                max_visible_lines: Some(3),
            })
            .style(self.slack_all_threads_composer_input_style(input_height))
            .bordered(false)
            .disabled(!enabled);
        let props = match highlights {
            Some(highlights) => props.highlights(highlights),
            None => props,
        };
        props
            .enter_behavior(TextInputEnterBehavior::SubmitOnEnter)
            .accessibility(
                gpui::ElementId::NamedInteger(
                    format!("slack-all-threads-reply-input-{thread_key}").into(),
                    cx.entity_id().as_u64(),
                ),
                format!("Reply in {}", row.conversation_label),
            )
            .on_change(slack_all_threads_on_change(target.clone(), cx))
            .on_submit(slack_all_threads_on_submit(target, cx))
            .on_escape(slack_all_threads_on_escape(cx))
    }

    fn slack_all_threads_composer_input_style(&self, height: f32) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(height),
            min_height: px(height),
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
            line_height: px(21.0),
            font_family: Some("Lato".into()),
        }
    }
}

fn slack_all_threads_on_change(
    target: SlackReplyComposerTarget,
    cx: &mut Context<SurfaceState>,
) -> TextInputChange {
    let surface = cx.entity().downgrade();
    Rc::new(move |value, _window, cx| {
        surface
            .update(cx, |surface, cx| {
                if surface.slack_main_route != crate::ui::surface::SlackMainRoute::AllThreads
                    || !surface.can_mutate_slack_reply_composer(&target)
                {
                    return;
                }
                surface.replace_slack_all_threads_reply_draft_text_for_target(&target, value, cx);
                cx.notify();
            })
            .ok();
    })
}

fn slack_all_threads_on_submit(
    target: SlackReplyComposerTarget,
    cx: &mut Context<SurfaceState>,
) -> TextInputAction {
    let surface = cx.entity().downgrade();
    Rc::new(move |_window, cx| {
        surface
            .update(cx, |surface, cx| {
                let composer_target = SlackComposerTarget::Reply(target.clone());
                if !surface.activate_first_slack_composer_aux_action_if_open(&composer_target, cx) {
                    surface.send_slack_all_threads_reply(&target, cx);
                }
            })
            .ok();
    })
}

fn slack_all_threads_on_escape(cx: &mut Context<SurfaceState>) -> TextInputAction {
    let surface = cx.entity().downgrade();
    Rc::new(move |_window, cx| {
        surface
            .update(cx, |surface, cx| {
                surface.handle_slack_escape_key(cx);
            })
            .ok();
    })
}
