use super::super::super::{
    div, px, rgb, slack_palette, Context, Div, Entity, FluentBuilder, InteractiveElement,
    ParentElement, SlackAllThreadRow, SlackAuxPanelQueryBehavior, SlackComposerDraftKey,
    SlackComposerTarget, SlackPalette, SlackReplyComposerTarget, StatefulInteractiveElement,
    Styled, SurfaceState, SLACK_COMPOSER_ATTACHMENTS_HEIGHT, SLACK_COMPOSER_INPUT_HEIGHT,
    SLACK_COMPOSER_TOOLBAR_HEIGHT,
};
use gpui::Role;
use gpui_components::text_input::TextInput;

mod controls;
mod input;

const SLACK_ALL_THREADS_COMPOSER_BORDER: f32 = 2.0;
const SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT: f32 = 26.0;
const SLACK_ALL_THREADS_COMPOSER_TOP_PADDING: f32 = 10.0;
const SLACK_ALL_THREADS_COMPOSER_BOTTOM_PADDING: f32 = 23.0;
const SLACK_ALL_THREADS_COMPOSER_FORMAT_HEIGHT: f32 = 38.0;
const SLACK_ALL_THREADS_COMPOSER_HEIGHT: f32 = SLACK_ALL_THREADS_COMPOSER_BORDER
    + SLACK_COMPOSER_INPUT_HEIGHT
    + SLACK_ALL_THREADS_COMPOSER_BROADCAST_HEIGHT
    + SLACK_COMPOSER_TOOLBAR_HEIGHT;
const SLACK_ALL_THREADS_COMPOSER_CONTAINER_HEIGHT: f32 = SLACK_ALL_THREADS_COMPOSER_HEIGHT
    + SLACK_ALL_THREADS_COMPOSER_TOP_PADDING
    + SLACK_ALL_THREADS_COMPOSER_BOTTOM_PADDING;

struct SlackAllThreadsComposerView {
    input: Entity<TextInput>,
    target: SlackReplyComposerTarget,
    draft_key: SlackComposerDraftKey,
    reply_error: Option<String>,
    schedule_owner: Option<crate::ui::surface::SlackScheduleDraftOwner>,
    schedule_visible: bool,
    schedule_enabled: bool,
    pending: bool,
    enabled: bool,
    send_enabled: bool,
    has_attachments: bool,
    formatting_enabled: bool,
}

struct SlackAllThreadsComposerShellLayout {
    attachments_height: f32,
    formatting_height: f32,
    palette: SlackPalette,
}

impl SurfaceState {
    pub(super) fn render_slack_all_threads_composer(
        &self,
        row: &SlackAllThreadRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let state = self.slack_all_threads_composer_view(row, cx);
        let palette = slack_palette(self.appearance_mode);
        let (attachments_height, formatting_height) =
            slack_all_threads_composer_optional_heights(&state);
        div()
            .relative()
            .h(px(SLACK_ALL_THREADS_COMPOSER_CONTAINER_HEIGHT
                + attachments_height
                + formatting_height))
            .px(px(20.0))
            .pt(px(SLACK_ALL_THREADS_COMPOSER_TOP_PADDING))
            .pb(px(SLACK_ALL_THREADS_COMPOSER_BOTTOM_PADDING))
            .child(self.render_slack_all_threads_composer_shell(
                row,
                &state,
                SlackAllThreadsComposerShellLayout {
                    attachments_height,
                    formatting_height,
                    palette,
                },
                cx,
            ))
    }

    fn render_slack_all_threads_composer_shell(
        &self,
        row: &SlackAllThreadRow,
        state: &SlackAllThreadsComposerView,
        layout: SlackAllThreadsComposerShellLayout,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let SlackAllThreadsComposerShellLayout {
            attachments_height,
            formatting_height,
            palette,
        } = layout;
        div()
            .id(format!("slack-all-threads-reply-composer-{}", row.key))
            .role(Role::Group)
            .aria_label(format!("Reply in {}", row.conversation_label))
            .h(px(SLACK_ALL_THREADS_COMPOSER_HEIGHT
                + attachments_height
                + formatting_height))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.composer_border))
            .bg(rgb(palette.composer_bg))
            .flex()
            .flex_col()
            .when(state.formatting_enabled, |this| {
                this.child(self.render_slack_reply_format_bar(&state.target, state.enabled, cx))
            })
            .when(state.has_attachments, |this| {
                this.child(self.render_slack_thread_draft_attachments(
                    &state.draft_key,
                    row.key.as_ref(),
                    cx,
                ))
            })
            .child(state.input.clone())
            .when_some(row.broadcast_label.clone(), |this, label| {
                this.child(self.render_slack_all_threads_broadcast_control(
                    &state.target,
                    label,
                    state.enabled,
                    cx,
                ))
            })
            .child(self.render_slack_all_threads_composer_footer(row, state, cx))
    }

    pub(super) fn render_slack_all_threads_composer_popover(
        &self,
        row: &SlackAllThreadRow,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let composer = self.slack_all_threads_composers.get(&row.key)?;
        let target = SlackComposerTarget::Reply(composer.target.clone());
        let popover = self.slack_aux_panel.as_ref().filter(|popover| {
            self.slack_composer_aux_target.as_ref() == Some(&target)
                && matches!(
                    popover.query_behavior,
                    Some(SlackAuxPanelQueryBehavior::Emoji | SlackAuxPanelQueryBehavior::Mention)
                )
        })?;
        let has_attachments = self.slack_thread_reply_draft_has_files(composer.target.draft_key());
        Some(self.render_slack_composer_popover_with_offsets(
            popover,
            28.0,
            SLACK_ALL_THREADS_COMPOSER_BOTTOM_PADDING
                + SLACK_COMPOSER_TOOLBAR_HEIGHT
                + if has_attachments {
                    SLACK_COMPOSER_ATTACHMENTS_HEIGHT
                } else {
                    0.0
                },
            cx,
        ))
    }

    fn slack_all_threads_composer_view(
        &self,
        row: &SlackAllThreadRow,
        cx: &mut Context<Self>,
    ) -> SlackAllThreadsComposerView {
        let thread_key = row.key.to_string();
        let composer = self
            .slack_all_threads_composers
            .get(&row.key)
            .expect("Slack All Threads composer should be installed");
        let target = composer.target.clone();
        let draft_key = target.draft_key().clone();
        let pending = self.slack_thread_reply_is_pending(&draft_key);
        let mutation_blocked = self.slack_thread_reply_mutation_is_blocked(&draft_key);
        let enabled = self.slack_workspace_api_capabilities.send_thread_reply && !mutation_blocked;
        let input = composer.input.clone();
        let formatting_enabled = composer.formatting_enabled;
        let input_props = self.slack_all_threads_composer_input_props(row, target.clone(), cx);
        input.update(cx, |input, cx| input.apply_props(input_props, cx));
        let reply_error = self
            .slack_all_threads_reply_errors
            .get(&thread_key)
            .cloned()
            .or_else(|| {
                self.slack_draft_autosave_error(&draft_key)
                    .map(str::to_string)
            });
        let schedule_owner = self.slack_all_threads_schedule_owner(&draft_key, row.key.clone());
        let schedule_visible = self.slack_schedule_controls_visible();
        let schedule_enabled = schedule_owner
            .as_ref()
            .is_some_and(|owner| self.can_schedule_slack_draft(owner));
        SlackAllThreadsComposerView {
            input,
            target,
            has_attachments: self.slack_thread_reply_draft_has_files(&draft_key),
            send_enabled: enabled && self.slack_thread_reply_draft_is_sendable(&draft_key),
            draft_key,
            reply_error,
            schedule_owner,
            schedule_visible,
            schedule_enabled,
            pending,
            enabled,
            formatting_enabled,
        }
    }
}

fn slack_all_threads_composer_optional_heights(state: &SlackAllThreadsComposerView) -> (f32, f32) {
    (
        if state.has_attachments {
            SLACK_COMPOSER_ATTACHMENTS_HEIGHT
        } else {
            0.0
        },
        if state.formatting_enabled {
            SLACK_ALL_THREADS_COMPOSER_FORMAT_HEIGHT
        } else {
            0.0
        },
    )
}
