use super::slack_message_render_context_id;
use crate::ui::surface::{
    alpha, div, px, rgb, slack_icon, slack_palette, AnyElement, AppearanceMode, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    SlackMessageActionTarget, SlackMessageRenderContext, SlackMessageRow,
    SlackReactionPickerAnchor, SlackReactionPickerSource, SlackReactionRow, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{Hsla, Role, Stateful};
use std::sync::Arc;

mod variant;

#[derive(Clone, Copy)]
struct SlackReactionStyle {
    background: Hsla,
    text_color: u32,
    count_weight: FontWeight,
}

pub(crate) struct SlackReactionBarInput<'a> {
    message_id: &'a str,
    target: Option<Arc<SlackMessageActionTarget>>,
    reaction_state: Arc<[crate::ui::SlackReaction]>,
    rows: &'a [SlackReactionRow],
    render_context: SlackMessageRenderContext,
}

impl<'a> SlackReactionBarInput<'a> {
    pub(crate) fn new(
        message_id: &'a str,
        target: Option<Arc<SlackMessageActionTarget>>,
        reaction_state: Arc<[crate::ui::SlackReaction]>,
        rows: &'a [SlackReactionRow],
        render_context: SlackMessageRenderContext,
    ) -> Self {
        Self {
            message_id,
            target,
            reaction_state,
            rows,
            render_context,
        }
    }
}

struct SlackReactionInteraction {
    target: Option<Arc<SlackMessageActionTarget>>,
    reaction_state: Arc<[crate::ui::SlackReaction]>,
    render_context: SlackMessageRenderContext,
}

struct SlackReactionAddButtonContext<'a> {
    message_id: &'a str,
    target: Arc<SlackMessageActionTarget>,
    reactions: Arc<[crate::ui::SlackReaction]>,
    render_context: SlackMessageRenderContext,
    interaction_enabled: bool,
    background: Hsla,
}

struct SlackReactionButtonBinding {
    target: Option<Arc<SlackMessageActionTarget>>,
    reaction_name: gpui::SharedString,
    reaction_state: Arc<[crate::ui::SlackReaction]>,
    enabled: bool,
    hover_color: u32,
}

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_message_reactions(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        self.render_slack_reactions(
            SlackReactionBarInput::new(
                &row.id,
                row.action_target.clone(),
                row.reaction_state.clone(),
                &row.reactions,
                render_context,
            ),
            cx,
        )
    }

    pub(in crate::ui::surface) fn render_slack_reactions(
        &self,
        input: SlackReactionBarInput<'_>,
        cx: &mut Context<Self>,
    ) -> Div {
        let SlackReactionBarInput {
            message_id,
            target,
            reaction_state,
            rows,
            render_context,
        } = input;
        let add_reaction_supported =
            self.slack_workspace_api_capabilities.mutate_reactions && target.is_some();
        let interaction = SlackReactionInteraction {
            target,
            reaction_state,
            render_context,
        };
        div()
            .h(px(28.0))
            .mb(px(4.0))
            .flex()
            .items_center()
            .children(
                rows.iter()
                    .map(|reaction| self.render_slack_reaction(reaction, &interaction, cx)),
            )
            .when(add_reaction_supported, |this| {
                this.child(self.render_slack_reaction_bar_add_action(message_id, &interaction, cx))
            })
    }

    fn render_slack_reaction_bar_add_action(
        &self,
        message_id: &str,
        interaction: &SlackReactionInteraction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = interaction
            .target
            .clone()
            .expect("supported Slack reaction action must have a typed target");
        let reactions = interaction.reaction_state.clone();
        let interaction_enabled = !self.slack_reaction_pending_for_target(&target);
        let render_context = interaction.render_context;
        let picker = self.slack_reaction_picker.as_ref().filter(|picker| {
            picker.identity.target() == &target
                && picker.identity.render_context() == render_context
                && picker.identity.anchor() == SlackReactionPickerAnchor::ReactionBar
        });
        let background = match self.appearance_mode {
            AppearanceMode::Dark => alpha(0xf8f8f8, 0.06),
            AppearanceMode::Light => alpha(0x1d1c1d, 0.06),
        };
        let action = self.render_slack_reaction_add_button(
            SlackReactionAddButtonContext {
                message_id,
                target,
                reactions,
                render_context,
                interaction_enabled,
                background,
            },
            cx,
        );
        div()
            .relative()
            .w(px(38.0))
            .h(px(28.0))
            .flex_none()
            .child(action)
            .when_some(picker, |this, picker| {
                this.child(self.render_slack_reaction_picker(picker, cx))
            })
            .into_any_element()
    }

    fn render_slack_reaction_add_button(
        &self,
        context: SlackReactionAddButtonContext<'_>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let action = div()
            .id(format!(
                "slack-reaction-bar-add-{}-{}",
                slack_message_render_context_id(context.render_context),
                context.message_id
            ))
            .role(Role::Button)
            .aria_label("Add reaction…")
            .focusable()
            .tab_stop(true)
            .w(px(34.0))
            .h(px(24.0))
            .px(px(8.0))
            .rounded(px(9999.0))
            .bg(context.background)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .focus_visible(|style| style.bg(rgb(0xe8f5fa)))
            .child(slack_icon(
                SlackShellIcon::ReactionBarAdd,
                0x9a9b9e,
                18.0,
                cx,
            ));
        self.bind_slack_reaction_add_button(action, context, cx)
    }

    fn bind_slack_reaction_add_button(
        &self,
        action: Stateful<Div>,
        context: SlackReactionAddButtonContext<'_>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let render_context = context.render_context;
        let target = context.target;
        let reactions = context.reactions;
        let keyboard_target = target.clone();
        let keyboard_reactions = reactions.clone();
        action.when(context.interaction_enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(alpha(0x1d1c1d, 0.10)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_reaction_picker_for_target(
                        SlackReactionPickerSource::new(
                            target.clone(),
                            reactions.clone(),
                            render_context,
                            SlackReactionPickerAnchor::ReactionBar,
                        ),
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if slack_reaction_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_reaction_picker_for_target(
                            SlackReactionPickerSource::new(
                                keyboard_target.clone(),
                                keyboard_reactions.clone(),
                                render_context,
                                SlackReactionPickerAnchor::ReactionBar,
                            ),
                            cx,
                        );
                    }
                }))
        })
    }

    fn render_slack_reaction(
        &self,
        reaction: &SlackReactionRow,
        interaction: &SlackReactionInteraction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let reaction_name = reaction.mutation_name.clone();
        let (interaction_supported, interaction_enabled) =
            self.slack_reaction_interaction_state(interaction.target.as_deref());
        let style =
            slack_reaction_style(self.appearance_mode, reaction.ownership.is_current_user());
        let reaction_button = div()
            .id(reaction.element_ids.for_context(interaction.render_context))
            .h(px(24.0))
            .mr(px(4.0))
            .mb(px(4.0))
            .rounded(px(9999.0))
            .bg(style.background)
            .text_color(rgb(style.text_color))
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .when(interaction_supported, |this| {
                this.role(Role::Button)
                    .aria_label(reaction.accessibility_label.clone())
                    .focusable()
                    .tab_stop(true)
            });
        self.bind_slack_reaction_interaction(
            reaction_button,
            SlackReactionButtonBinding {
                target: interaction.target.clone(),
                reaction_name,
                reaction_state: interaction.reaction_state.clone(),
                enabled: interaction_enabled,
                hover_color: palette.send_disabled_border,
            },
            cx,
        )
        .children(
            reaction
                .variants
                .iter()
                .map(|variant| self.render_slack_reaction_variant(variant)),
        )
        .child(slack_reaction_count(reaction, style))
        .into_any_element()
    }

    fn bind_slack_reaction_interaction(
        &self,
        button: Stateful<Div>,
        binding: SlackReactionButtonBinding,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = binding.enabled;
        let hover_color = binding.hover_color;
        let target = binding.target;
        let reaction_name = binding.reaction_name;
        let reactions = binding.reaction_state;
        let keyboard_reaction_name = reaction_name.clone();
        let keyboard_reactions = reactions.clone();
        button.when(enabled, |this| {
            let target = target.expect("enabled Slack reaction action must have a typed target");
            let keyboard_target = target.clone();
            this.cursor_pointer()
                .hover(move |style| style.bg(alpha(hover_color, 0.10)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_reaction_for_target_with_state(
                        target.clone(),
                        reactions.clone(),
                        reaction_name.as_ref(),
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if slack_reaction_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_reaction_for_target_with_state(
                            keyboard_target.clone(),
                            keyboard_reactions.clone(),
                            keyboard_reaction_name.as_ref(),
                            cx,
                        );
                    }
                }))
        })
    }

    fn slack_reaction_interaction_state(
        &self,
        target: Option<&SlackMessageActionTarget>,
    ) -> (bool, bool) {
        let supported = self.slack_workspace_api_capabilities.mutate_reactions && target.is_some();
        let pending = target.is_some_and(|target| self.slack_reaction_pending_for_target(target));
        (supported, supported && !pending)
    }
}

fn slack_reaction_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}

fn slack_reaction_style(appearance_mode: AppearanceMode, selected: bool) -> SlackReactionStyle {
    match (appearance_mode, selected) {
        (AppearanceMode::Dark, false) => SlackReactionStyle {
            background: alpha(0xf8f8f8, 0.06),
            text_color: 0xf8f8f8,
            count_weight: FontWeight::NORMAL,
        },
        (AppearanceMode::Dark, true) => SlackReactionStyle {
            background: alpha(0x004d76, 1.0),
            text_color: 0xf8f8f8,
            count_weight: FontWeight::BOLD,
        },
        (AppearanceMode::Light, false) => SlackReactionStyle {
            background: alpha(0x1d1c1d, 0.06),
            text_color: 0x1d1c1d,
            count_weight: FontWeight::NORMAL,
        },
        (AppearanceMode::Light, true) => SlackReactionStyle {
            background: alpha(0xe3f8ff, 1.0),
            text_color: 0x1264a3,
            count_weight: FontWeight::BOLD,
        },
    }
}

fn slack_reaction_count(reaction: &SlackReactionRow, style: SlackReactionStyle) -> Div {
    div()
        .text_size(px(12.0))
        .line_height(px(12.0))
        .font_weight(style.count_weight)
        .text_color(rgb(style.text_color))
        .child(reaction.count_label.clone())
}
