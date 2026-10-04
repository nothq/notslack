use super::super::super::slack_message_render_context_id;
use crate::ui::surface::{
    div, px, rgb, slack_icon, slack_palette, Context, FluentBuilder, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, SlackMessageActionTarget, SlackMessageRenderContext,
    SlackMessageRow, SlackReactionPickerAnchor, SlackReactionPickerSource, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{Role, Stateful};
use std::sync::Arc;

struct SlackHoverReactionBinding {
    target: Arc<SlackMessageActionTarget>,
    reactions: Arc<[crate::ui::SlackReaction]>,
    render_context: SlackMessageRenderContext,
    pending: bool,
    hover_background: u32,
}

pub(super) struct SlackAddReactionActionContext<'a> {
    render_context: SlackMessageRenderContext,
    hover_group: &'a str,
    persistent: bool,
}

impl<'a> SlackAddReactionActionContext<'a> {
    pub(super) fn new(
        render_context: SlackMessageRenderContext,
        hover_group: &'a str,
        persistent: bool,
    ) -> Self {
        Self {
            render_context,
            hover_group,
            persistent,
        }
    }
}

impl SurfaceState {
    pub(super) fn render_slack_add_reaction_action(
        &self,
        row: &SlackMessageRow,
        context: SlackAddReactionActionContext<'_>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackAddReactionActionContext {
            render_context,
            hover_group,
            persistent,
        } = context;
        let palette = slack_palette(self.appearance_mode);
        let target = row
            .action_target
            .clone()
            .expect("supported Slack reaction action must have a typed target");
        let reactions = row.reaction_state.clone();
        let pending = self.slack_reaction_pending_for_target(&target);
        let action = div()
            .id(format!(
                "slack-message-add-reaction-{}-{}",
                slack_message_render_context_id(render_context),
                row.id
            ))
            .role(Role::Button)
            .aria_label(slack_add_reaction_action_label(pending))
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(32.0))
            .rounded(px(8.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0).bg(rgb(0xe8f5fa)));
        self.bind_slack_hover_reaction_action(
            action,
            SlackHoverReactionBinding {
                target,
                reactions,
                render_context,
                pending,
                hover_background: palette.composer_chip_bg,
            },
            cx,
        )
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(
            SlackShellIcon::AddReaction,
            palette.main_secondary_text,
            18.0,
            cx,
        ))
    }

    fn bind_slack_hover_reaction_action(
        &self,
        action: Stateful<gpui::Div>,
        binding: SlackHoverReactionBinding,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        let pending = binding.pending;
        let hover_background = binding.hover_background;
        let keyboard_target = binding.target.clone();
        let keyboard_reactions = binding.reactions.clone();
        let render_context = binding.render_context;
        action.when(!pending, |this| {
            this.cursor_pointer()
                .hover(move |style| style.bg(rgb(hover_background)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_reaction_picker_for_target(
                        SlackReactionPickerSource::new(
                            binding.target.clone(),
                            binding.reactions.clone(),
                            render_context,
                            SlackReactionPickerAnchor::HoverAction,
                        ),
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if slack_hover_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_reaction_picker_for_target(
                            SlackReactionPickerSource::new(
                                keyboard_target.clone(),
                                keyboard_reactions.clone(),
                                render_context,
                                SlackReactionPickerAnchor::HoverAction,
                            ),
                            cx,
                        );
                    }
                }))
        })
    }
}

fn slack_add_reaction_action_label(pending: bool) -> &'static str {
    if pending {
        "Add reaction…, reaction change pending"
    } else {
        "Add reaction…"
    }
}

fn slack_hover_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
