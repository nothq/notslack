use std::sync::Arc;

use super::super::{
    prepare_slack_conversation_snapshot, prepare_slack_thread_snapshot, Context,
    SlackReactionRequest, SurfaceState,
};
use super::PreparedSlackReactionReceipt;
use crate::ui::surface::SlackMessageActionTarget;
use crate::ui::{
    build_slack_message_remote_images, SlackReaction, SlackReactionMutation,
    SlackReactionMutationReceipt, SlackReactionName, WorkspaceApi,
};

type SlackOptimisticReactionState = (SlackReactionMutation, Vec<SlackReaction>);

enum SlackReactionToggleStart {
    Started,
    AlreadyPending,
}

impl SurfaceState {
    pub(crate) fn toggle_slack_reaction(
        &mut self,
        message_id: &str,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.slack_message_action_target(message_id) else {
            self.fail_slack_reaction("The Slack message to react to is unavailable.", cx);
            return;
        };
        self.toggle_slack_reaction_for_target(target, reaction_name, cx);
    }

    pub(crate) fn control_slack_reaction_state(
        &self,
        message_id: &str,
    ) -> Result<crate::model::ChatMessageReactionState, String> {
        let (target, reaction_state) = self
            .slack_reaction_source(message_id)
            .ok_or_else(|| "The Slack message to react to is unavailable.".to_string())?;
        Ok(crate::model::ChatMessageReactionState {
            message_id: target.message_timestamp().as_str().to_string(),
            pending: self.slack_reaction_pending_for_target(&target),
            reactions: reaction_state
                .iter()
                .map(|reaction| crate::model::ChatReactionSummary {
                    emoji: reaction.emoji.clone(),
                    count: reaction.count,
                    current_user_active: reaction.active,
                })
                .collect(),
        })
    }

    pub(crate) fn control_toggle_slack_reaction(
        &mut self,
        message_id: &str,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) -> Result<crate::model::ChatMessageReactionState, String> {
        let (target, reactions) = self
            .slack_reaction_source(message_id)
            .ok_or_else(|| "The Slack message to react to is unavailable.".to_string())?;
        match self.begin_slack_reaction_for_target_with_state(
            target,
            reactions,
            reaction_name,
            cx,
        )? {
            SlackReactionToggleStart::Started => self.control_slack_reaction_state(message_id),
            SlackReactionToggleStart::AlreadyPending => {
                Err("A Slack reaction change is already pending for this message.".to_string())
            }
        }
    }

    pub(crate) fn toggle_slack_reaction_for_target(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) {
        if let Err(message) = self.begin_slack_reaction_for_target(target, reaction_name, cx) {
            self.fail_slack_reaction(message, cx);
        }
    }

    pub(crate) fn toggle_slack_reaction_for_target_with_state(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        reactions: Arc<[SlackReaction]>,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) {
        if let Err(message) =
            self.begin_slack_reaction_for_target_with_state(target, reactions, reaction_name, cx)
        {
            self.fail_slack_reaction(message, cx);
        }
    }

    fn begin_slack_reaction_for_target(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) -> Result<SlackReactionToggleStart, String> {
        let reactions = self
            .slack_reaction_state_for_action_target(&target)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Slack message {} is unavailable for reaction mutation",
                    target.message_timestamp().as_str()
                )
            })?;
        self.begin_slack_reaction_for_target_with_state(target, reactions, reaction_name, cx)
    }

    fn begin_slack_reaction_for_target_with_state(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        reactions: Arc<[SlackReaction]>,
        reaction_name: &str,
        cx: &mut Context<Self>,
    ) -> Result<SlackReactionToggleStart, String> {
        if !self.slack_workspace_api_capabilities.mutate_reactions {
            return Err("Slack reaction changes are unavailable for this workspace.".to_string());
        }
        if self.slack_reaction_pending_for_target(&target) {
            return Ok(SlackReactionToggleStart::AlreadyPending);
        }
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        let request = self.prepare_slack_reaction_request(target, reactions, reaction_name)?;
        let workspace_api = self
            .active_slack_workspace_api()
            .ok_or_else(|| "missing Slack workspace api".to_string())?;
        self.start_slack_reaction_request(request, workspace_api, cx)?;
        Ok(SlackReactionToggleStart::Started)
    }

    fn start_slack_reaction_request(
        &mut self,
        request: SlackReactionRequest,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.slack_reaction_generation = self
            .slack_reaction_generation
            .checked_add(1)
            .expect("Slack reaction request generation overflowed");
        let request = SlackReactionRequest {
            generation: self.slack_reaction_generation,
            ..request
        };
        self.apply_optimistic_slack_reaction(&request, cx)?;
        let previous = self
            .slack_pending_reactions
            .insert(request.target.clone(), request.clone());
        assert!(
            previous.is_none(),
            "Slack reaction target must have at most one in-flight mutation"
        );
        self.slack_error = None;
        cx.notify();
        self.spawn_slack_reaction_request(request, workspace_api, cx);
        Ok(())
    }

    fn spawn_slack_reaction_request(
        &mut self,
        request: SlackReactionRequest,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        let completion_request = request.clone();
        self.spawn_background_task(
            request,
            cx,
            move |request| {
                workspace_api
                    .mutate_slack_reaction(
                        request.target.reaction(),
                        &request.reaction_name,
                        request.mutation,
                    )
                    .map(|receipt| match receipt {
                        SlackReactionMutationReceipt::Conversation { snapshot, scope } => {
                            PreparedSlackReactionReceipt::Conversation {
                                snapshot: Box::new(prepare_slack_conversation_snapshot(snapshot)),
                                scope,
                            }
                        }
                        SlackReactionMutationReceipt::Thread(snapshot) => {
                            let remote_images =
                                build_slack_message_remote_images(&snapshot.replies);
                            PreparedSlackReactionReceipt::Thread {
                                snapshot: Box::new(prepare_slack_thread_snapshot(snapshot)),
                                remote_images,
                            }
                        }
                    })
            },
            move |this, result, cx| {
                this.finish_slack_reaction_request(&completion_request, result, cx);
            },
        );
    }

    fn prepare_slack_reaction_request(
        &self,
        target: Arc<SlackMessageActionTarget>,
        original_reactions: Arc<[SlackReaction]>,
        reaction_name: &str,
    ) -> Result<SlackReactionRequest, String> {
        let workspace = self
            .slack_workspace()
            .ok_or_else(|| "Slack workspace is unavailable".to_string())?;
        if workspace.team_id != target.team_id() {
            return Err("Slack reaction target belongs to another workspace".to_string());
        }
        let reaction_name = SlackReactionName::parse(reaction_name)
            .map_err(|error| format!("Cannot change this Slack reaction: {error}"))?;
        let (mutation, optimistic_reactions) =
            optimistic_slack_reaction_state(&original_reactions, &reaction_name)?;
        Ok(SlackReactionRequest {
            generation: 0,
            target,
            reaction_name,
            mutation,
            original_reactions,
            optimistic_reactions: optimistic_reactions.into(),
        })
    }

    pub(super) fn fail_slack_reaction(
        &mut self,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.slack_error = Some(message.into());
        cx.notify();
    }
}

fn optimistic_slack_reaction_state(
    current: &[SlackReaction],
    reaction_name: &SlackReactionName,
) -> Result<SlackOptimisticReactionState, String> {
    let mut optimistic = current.to_vec();
    let Some(index) = optimistic
        .iter()
        .position(|reaction| reaction.emoji == reaction_name.as_str())
    else {
        optimistic.push(SlackReaction {
            emoji: reaction_name.as_str().to_string(),
            count: 1,
            active: true,
        });
        return Ok((SlackReactionMutation::Add, optimistic));
    };
    if optimistic[index].active {
        let count = optimistic[index].count.checked_sub(1).ok_or_else(|| {
            format!(
                "Cannot remove Slack reaction {} with a zero count",
                reaction_name.as_str()
            )
        })?;
        if count == 0 {
            optimistic.remove(index);
        } else {
            optimistic[index].count = count;
            optimistic[index].active = false;
        }
        Ok((SlackReactionMutation::Remove, optimistic))
    } else {
        optimistic[index].count = optimistic[index]
            .count
            .checked_add(1)
            .ok_or_else(|| format!("Slack reaction {} count overflowed", reaction_name.as_str()))?;
        optimistic[index].active = true;
        Ok((SlackReactionMutation::Add, optimistic))
    }
}
