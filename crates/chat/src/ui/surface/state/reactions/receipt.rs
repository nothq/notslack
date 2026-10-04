use std::{collections::HashMap, sync::Arc};

use super::super::{
    Context, PreparedSlackConversationSnapshot, PreparedSlackThreadSnapshot, SlackMessageRow,
    SlackReactionRequest, SurfaceState,
};
use super::lookup::find_slack_message_row;
use super::propagation::{mutate_slack_search_reaction_rows, slack_reaction_row_remote_image_urls};
use super::{PreparedSlackReactionReceipt, SlackAuthoritativeMessageInput};
use crate::ui::surface::{slack_reaction_rows, SlackMessageActionTarget};
use crate::ui::{
    Image, SlackMessage, SlackPinnedItem, SlackReaction, SlackReactionConversationScope,
};

impl SurfaceState {
    pub(super) fn finish_slack_reaction_request(
        &mut self,
        request: &SlackReactionRequest,
        result: Result<PreparedSlackReactionReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_reactions.get(request.target.as_ref()) != Some(request) {
            return;
        }
        self.slack_pending_reactions.remove(request.target.as_ref());
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.team_id != request.target.team_id())
        {
            self.schedule_queued_slack_conversation_reconciliation(cx);
            cx.notify();
            return;
        }
        let result = match result {
            Ok(PreparedSlackReactionReceipt::Conversation { snapshot, scope }) => {
                self.apply_slack_reaction_conversation_receipt(request, *snapshot, scope, cx)
            }
            Ok(PreparedSlackReactionReceipt::Thread {
                snapshot,
                remote_images,
            }) => self.apply_slack_reaction_thread_receipt(request, *snapshot, remote_images, cx),
            Err(message) => Err(message),
        };
        match result {
            Ok(()) => cx.notify(),
            Err(message) => {
                let rollback_result = self.apply_slack_reaction_state(
                    &request.target,
                    request.original_reactions.clone(),
                    cx,
                );
                self.queue_slack_reaction_reconciliation(&request.target, cx);
                match rollback_result {
                    Ok(()) => self.fail_slack_reaction(message, cx),
                    Err(rollback_message) => self.fail_slack_reaction(
                        format!("{message} Reaction rollback failed: {rollback_message}"),
                        cx,
                    ),
                }
            }
        }
        self.schedule_queued_slack_conversation_reconciliation(cx);
    }

    fn apply_slack_reaction_conversation_receipt(
        &mut self,
        request: &SlackReactionRequest,
        prepared: PreparedSlackConversationSnapshot,
        _scope: SlackReactionConversationScope,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if request.target.is_thread_reply()
            || prepared.snapshot.team_id != request.target.team_id()
            || prepared.snapshot.conversation_id != request.target.conversation_id()
        {
            return Err(
                "Slack reaction refresh returned a mismatched conversation state.".to_string(),
            );
        }
        let Some((message, row)) =
            authoritative_conversation_message_and_row(&prepared, &request.target)
        else {
            return Err("Slack reaction refresh omitted the reacted message.".to_string());
        };
        if !slack_reaction_message_matches_request(message, request) {
            return Err("Slack reaction refresh returned mismatched reaction state.".to_string());
        }
        let message = message.clone();
        let row = row.clone();
        let remote_images = prepared.remote_images;
        self.apply_authoritative_slack_reaction(
            SlackAuthoritativeMessageInput::new(&request.target, &message, &row, remote_images),
            cx,
        );
        Ok(())
    }

    fn apply_slack_reaction_thread_receipt(
        &mut self,
        request: &SlackReactionRequest,
        prepared: PreparedSlackThreadSnapshot,
        remote_images: HashMap<String, Arc<Image>>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(thread_timestamp) = request.target.thread_timestamp() else {
            return Err(
                "Slack reaction refresh returned a thread for a conversation message.".to_string(),
            );
        };
        if prepared.snapshot.team_id != request.target.team_id()
            || prepared.snapshot.conversation_id != request.target.conversation_id()
            || prepared.snapshot.thread_timestamp != thread_timestamp.as_str()
        {
            return Err("Slack reaction refresh returned a mismatched thread.".to_string());
        }
        let Some((message, row)) = authoritative_thread_message_and_row(&prepared, &request.target)
        else {
            return Err("Slack reaction refresh omitted the reacted reply.".to_string());
        };
        if !slack_reaction_message_matches_request(message, request) {
            return Err("Slack reaction refresh returned mismatched reaction state.".to_string());
        }
        let message = message.clone();
        let row = row.clone();
        self.apply_authoritative_slack_reaction(
            SlackAuthoritativeMessageInput::new(&request.target, &message, &row, remote_images),
            cx,
        );
        Ok(())
    }

    pub(super) fn apply_optimistic_slack_reaction(
        &mut self,
        request: &SlackReactionRequest,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.apply_slack_reaction_state(&request.target, request.optimistic_reactions.clone(), cx)
    }

    fn apply_slack_reaction_state(
        &mut self,
        target: &SlackMessageActionTarget,
        reaction_state: Arc<[SlackReaction]>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let row = self.slack_message_row_for_action_target(target).cloned();
        let search_row_exists = self
            .slack_search_rows
            .iter()
            .any(|row| row.action_target.as_ref() == target);
        if row.is_none() && !search_row_exists {
            return Err(format!(
                "Slack message {} is unavailable for reaction reconciliation",
                target.message_timestamp().as_str()
            ));
        }
        self.apply_slack_reaction_state_to_snapshots(target, &reaction_state);
        self.apply_slack_reaction_state_to_rows(target, reaction_state, row, cx);
        Ok(())
    }

    fn apply_slack_reaction_state_to_snapshots(
        &mut self,
        target: &SlackMessageActionTarget,
        reaction_state: &[SlackReaction],
    ) {
        if self
            .slack_conversation_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.team_id == target.team_id()
                    && snapshot.conversation_id == target.conversation_id()
            })
        {
            if let Some(snapshot) = self.slack_conversation_snapshot.as_mut() {
                mutate_slack_message_reaction_state(&mut snapshot.messages, target, reaction_state);
            }
        }
        if self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == target.team_id()
                && workspace.conversation_id == target.conversation_id()
        }) {
            if let Some(workspace) = self.slack_workspace_mut() {
                mutate_slack_message_reaction_state(
                    &mut workspace.messages,
                    target,
                    reaction_state,
                );
            }
        }
        if let Some(snapshot) = self.slack_all_threads_snapshot.as_mut() {
            for thread in &mut snapshot.threads {
                if thread.conversation_id != target.conversation_id()
                    || thread.thread_timestamp != target.root_timestamp().as_str()
                {
                    continue;
                }
                if thread.parent.id == target.message_timestamp().as_str() {
                    thread.parent.reactions = reaction_state.to_vec();
                }
                mutate_slack_message_reaction_state(
                    &mut thread.visible_replies,
                    target,
                    reaction_state,
                );
            }
        }
        if let Some(snapshot) = self.slack_pins_snapshot.as_mut() {
            for pin in &mut snapshot.items {
                mutate_slack_pinned_item_reaction_state(pin, target, reaction_state);
            }
        }
    }

    fn apply_slack_reaction_state_to_rows(
        &mut self,
        target: &SlackMessageActionTarget,
        reaction_state: Arc<[SlackReaction]>,
        row: Option<SlackMessageRow>,
        cx: &mut Context<Self>,
    ) {
        if let Some(mut row) = row {
            row.reaction_state = reaction_state.clone();
            row.reactions = slack_reaction_rows(&row.id, &reaction_state);
            let affected_rows = self.apply_authoritative_slack_message_row(
                target,
                &row,
                super::propagation::SlackAuthoritativeMessageMutation::Reactions,
            );
            self.finish_authoritative_slack_message_row_update(&row, affected_rows, cx);
            return;
        }
        let reaction_rows =
            slack_reaction_rows(target.message_timestamp().as_str(), &reaction_state);
        let updated = mutate_slack_search_reaction_rows(
            Arc::make_mut(&mut self.slack_search_rows),
            target,
            &reaction_state,
            &reaction_rows,
        );
        assert!(
            updated,
            "resolved Slack search reaction target must remain available"
        );
        for url in slack_reaction_row_remote_image_urls(&reaction_rows) {
            self.enqueue_slack_remote_image_url(url.to_string(), cx);
        }
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
    }

    fn queue_slack_reaction_reconciliation(
        &mut self,
        target: &SlackMessageActionTarget,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_conversation_live_target
            .as_ref()
            .is_some_and(|live_target| {
                live_target.team_id == target.team_id()
                    && live_target.conversation_id == target.conversation_id()
            })
        {
            self.queue_slack_conversation_reconciliation(cx);
        }
    }
}

fn authoritative_conversation_message_and_row<'a>(
    prepared: &'a PreparedSlackConversationSnapshot,
    target: &SlackMessageActionTarget,
) -> Option<(&'a SlackMessage, &'a SlackMessageRow)> {
    let message = prepared
        .snapshot
        .messages
        .iter()
        .find(|message| message.id == target.message_timestamp().as_str())?;
    let row = find_slack_message_row(&prepared.message_rows, target)?;
    Some((message, row))
}

fn authoritative_thread_message_and_row<'a>(
    prepared: &'a PreparedSlackThreadSnapshot,
    target: &SlackMessageActionTarget,
) -> Option<(&'a SlackMessage, &'a SlackMessageRow)> {
    let message = prepared
        .snapshot
        .replies
        .iter()
        .find(|message| message.id == target.message_timestamp().as_str())?;
    let row = find_slack_message_row(&prepared.reply_rows, target)?;
    Some((message, row))
}

fn slack_reaction_message_matches_request(
    message: &SlackMessage,
    request: &SlackReactionRequest,
) -> bool {
    let active = message
        .reactions
        .iter()
        .find(|reaction| reaction.emoji == request.reaction_name.as_str())
        .is_some_and(|reaction| reaction.active);
    active == request.mutation.active_after()
}

fn mutate_slack_message_reaction_state(
    messages: &mut [SlackMessage],
    target: &SlackMessageActionTarget,
    reaction_state: &[SlackReaction],
) -> bool {
    for message in messages {
        if message.id == target.message_timestamp().as_str() {
            message.reactions = reaction_state.to_vec();
            return true;
        }
        if mutate_slack_message_reaction_state(&mut message.replies, target, reaction_state) {
            return true;
        }
    }
    false
}

fn mutate_slack_pinned_item_reaction_state(
    item: &mut SlackPinnedItem,
    target: &SlackMessageActionTarget,
    reaction_state: &[SlackReaction],
) {
    let (conversation_id, message) = match item {
        SlackPinnedItem::Message(item) => (&item.conversation_id, &mut item.message),
        SlackPinnedItem::File(item) => (&item.conversation_id, &mut item.file_message),
        SlackPinnedItem::FileComment(item) => (&item.conversation_id, &mut item.comment_message),
    };
    if conversation_id == target.conversation_id()
        && message.id == target.message_timestamp().as_str()
    {
        message.reactions = reaction_state.to_vec();
    }
}
