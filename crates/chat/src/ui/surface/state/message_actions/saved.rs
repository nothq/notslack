use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use super::super::reactions::SlackAuthoritativeMessageInput;
use super::super::WorkspaceApi;
use super::{
    prepare_slack_conversation_snapshot, Context, SlackLaterState, SlackMessageSavedRequest,
    SlackSavedMessageMutation, SurfaceState,
};
use crate::ui::surface::{SlackLaterRowContent, SlackMessageActionTarget, SlackMessageRow};
use crate::ui::{Image, SlackLaterReferenceContent, SlackMessage};

struct SlackMessageSavedWork {
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackMessageSavedRequest,
    target: Arc<SlackMessageActionTarget>,
}

struct PreparedSlackMessageSavedReceipt {
    team_id: String,
    conversation_id: String,
    message: SlackMessage,
    row: SlackMessageRow,
    remote_images: HashMap<String, Arc<Image>>,
}

impl SurfaceState {
    pub(crate) fn toggle_slack_message_saved_state_for_target(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        saved_state: Option<SlackLaterState>,
        cx: &mut Context<Self>,
    ) {
        let Some(work) = self.begin_slack_message_saved_work(target, saved_state, cx) else {
            return;
        };
        let completion_request = work.request.clone();
        let completion_target = work.target.clone();
        self.spawn_background_task(
            work,
            cx,
            execute_slack_message_saved_work,
            move |this, result, cx| {
                this.finish_slack_message_saved_request(
                    &completion_request,
                    &completion_target,
                    result,
                    cx,
                );
            },
        );
    }

    fn begin_slack_message_saved_work(
        &mut self,
        target: Arc<SlackMessageActionTarget>,
        saved_state: Option<SlackLaterState>,
        cx: &mut Context<Self>,
    ) -> Option<SlackMessageSavedWork> {
        if !self
            .slack_workspace_api_capabilities
            .mutate_message_saved_state
            || self.slack_pending_message_saved.is_some()
            || target.is_thread_reply()
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != target.team_id())
        {
            return None;
        }
        self.slack_message_row_for_action_target(&target)?;
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return None;
        };
        self.slack_message_saved_generation = self
            .slack_message_saved_generation
            .checked_add(1)
            .expect("Slack saved-message request generation overflowed");
        let request = SlackMessageSavedRequest {
            generation: self.slack_message_saved_generation,
            team_id: target.team_id().to_string(),
            conversation_id: target.conversation_id().to_string(),
            message_timestamp: target.message_timestamp().clone(),
            mutation: if saved_state.is_some() {
                SlackSavedMessageMutation::Remove
            } else {
                SlackSavedMessageMutation::Save
            },
        };
        self.slack_pending_message_saved = Some(request.clone());
        self.slack_error = None;
        cx.notify();
        Some(SlackMessageSavedWork {
            workspace_api,
            request,
            target,
        })
    }

    pub(crate) fn reset_slack_message_action_context(&mut self) {
        self.reset_slack_date_jump_context();
        self.reset_slack_message_forward_context();
        self.reset_slack_message_mutation_context();
        self.slack_message_menu = None;
        self.slack_message_menu_focus_pending = false;
        if self.slack_pending_message_saved.take().is_some() {
            self.slack_message_saved_generation = self
                .slack_message_saved_generation
                .checked_add(1)
                .expect("Slack saved-message request generation overflowed");
        }
        if self.slack_pending_message_permalink.take().is_some() {
            self.slack_message_permalink_generation = self
                .slack_message_permalink_generation
                .checked_add(1)
                .expect("Slack message permalink request generation overflowed");
        }
        if self.slack_pending_message_mark_unread.take().is_some() {
            self.slack_message_mark_unread_generation = self
                .slack_message_mark_unread_generation
                .checked_add(1)
                .expect("Slack mark-unread request generation overflowed");
        }
    }

    fn finish_slack_message_saved_request(
        &mut self,
        request: &SlackMessageSavedRequest,
        target: &SlackMessageActionTarget,
        result: Result<PreparedSlackMessageSavedReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_saved.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_message_saved = None;
        self.schedule_queued_slack_conversation_reconciliation(cx);
        if self.slack_message_saved_generation != request.generation
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            cx.notify();
            return;
        }
        let receipt = match validate_slack_message_saved_receipt(request, target, result) {
            Ok(receipt) => receipt,
            Err(message) => {
                self.slack_error = Some(message);
                cx.notify();
                return;
            }
        };
        self.apply_authoritative_slack_saved_message(
            SlackAuthoritativeMessageInput::new(
                target,
                &receipt.message,
                &receipt.row,
                receipt.remote_images,
            ),
            cx,
        );
        if request.mutation == SlackSavedMessageMutation::Remove {
            self.remove_slack_later_row_for_action_target(target);
        }
        cx.notify();
    }

    fn remove_slack_later_row_for_action_target(&mut self, target: &SlackMessageActionTarget) {
        let selected_key = self.slack_later_selected_key.clone();
        let mut removed_keys = self
            .slack_later_snapshot
            .as_ref()
            .into_iter()
            .flat_map(|snapshot| snapshot.items.iter())
            .filter_map(|item| {
                let SlackLaterReferenceContent::Message(reference) = &item.content else {
                    return None;
                };
                (reference.conversation_id == target.conversation_id()
                    && reference.timestamp == target.message_timestamp().as_str())
                .then(|| item.key.clone())
            })
            .collect::<HashSet<_>>();
        removed_keys.extend(self.slack_later_rows.iter().filter_map(|later| {
            let SlackLaterRowContent::Message { detail } = &later.content else {
                return None;
            };
            (detail.selected_message_row.action_target.as_deref() == Some(target))
                .then(|| later.key.clone())
        }));
        let removed_indices = self
            .slack_later_rows
            .iter()
            .enumerate()
            .filter_map(|(index, later)| removed_keys.contains(&later.key).then_some(index))
            .collect::<Vec<_>>();
        let removed_selected = selected_key
            .as_ref()
            .is_some_and(|key| removed_keys.contains(key));
        let previous_row_count = self.slack_later_rows.len();
        for index in removed_indices.iter().rev().copied() {
            self.slack_later_rows.remove(index);
        }
        if self.slack_later_list_state.item_count() == previous_row_count {
            for index in removed_indices.iter().rev().copied() {
                self.slack_later_list_state.splice(index..index + 1, 0);
            }
        } else {
            self.slack_later_list_state
                .reset(self.slack_later_rows.len());
        }
        if let Some(snapshot) = self.slack_later_snapshot.as_mut() {
            snapshot
                .items
                .retain(|item| !removed_keys.contains(&item.key));
        }
        if removed_selected {
            self.slack_later_selected_key = None;
            self.reset_slack_thread_context();
        }
    }
}

fn execute_slack_message_saved_work(
    work: SlackMessageSavedWork,
) -> Result<PreparedSlackMessageSavedReceipt, String> {
    let snapshot = work.workspace_api.mutate_slack_message_saved_state(
        &work.request.conversation_id,
        &work.request.message_timestamp,
        work.request.mutation,
    )?;
    let prepared = prepare_slack_conversation_snapshot(snapshot);
    let expected_saved_state = work
        .request
        .mutation
        .active_after()
        .then_some(SlackLaterState::InProgress);
    let message = prepared
        .snapshot
        .messages
        .iter()
        .find(|message| message.id == work.request.message_timestamp.as_str())
        .ok_or_else(|| "Slack saved-message refresh omitted the target message.".to_string())?;
    let row = prepared
        .message_rows
        .iter()
        .find(|row| row.action_target.as_deref() == Some(work.target.as_ref()))
        .ok_or_else(|| "Slack saved-message refresh omitted the target row.".to_string())?;
    if message.saved_state != expected_saved_state || row.saved_state != expected_saved_state {
        return Err("Slack saved-message refresh returned mismatched saved state.".to_string());
    }
    Ok(PreparedSlackMessageSavedReceipt {
        team_id: prepared.snapshot.team_id,
        conversation_id: prepared.snapshot.conversation_id,
        message: message.clone(),
        row: row.clone(),
        remote_images: prepared.remote_images,
    })
}

fn validate_slack_message_saved_receipt(
    request: &SlackMessageSavedRequest,
    target: &SlackMessageActionTarget,
    result: Result<PreparedSlackMessageSavedReceipt, String>,
) -> Result<PreparedSlackMessageSavedReceipt, String> {
    let receipt = result?;
    let expected_saved_state = request
        .mutation
        .active_after()
        .then_some(SlackLaterState::InProgress);
    if receipt.team_id != request.team_id
        || receipt.conversation_id != request.conversation_id
        || receipt.message.id != request.message_timestamp.as_str()
        || receipt.row.action_target.as_deref() != Some(target)
        || receipt.message.saved_state != expected_saved_state
        || receipt.row.saved_state != receipt.message.saved_state
    {
        return Err("Slack returned mismatched saved-message state.".to_string());
    }
    Ok(receipt)
}
