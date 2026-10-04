use super::super::{Context, SurfaceState};
use super::SlackAuthoritativeMessageInput;
use crate::ui::surface::SlackMessageActionTarget;
use crate::ui::{SlackMessage, SlackPinnedItem};

mod rows;

pub(super) use rows::{mutate_slack_search_reaction_rows, slack_reaction_row_remote_image_urls};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SlackAuthoritativeMessageMutation {
    Reactions,
    SavedState,
}

impl SurfaceState {
    pub(super) fn apply_authoritative_slack_reaction(
        &mut self,
        input: SlackAuthoritativeMessageInput<'_>,
        cx: &mut Context<Self>,
    ) {
        self.apply_authoritative_slack_message(
            input,
            SlackAuthoritativeMessageMutation::Reactions,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn apply_authoritative_slack_saved_message(
        &mut self,
        input: SlackAuthoritativeMessageInput<'_>,
        cx: &mut Context<Self>,
    ) {
        self.apply_authoritative_slack_message(
            input,
            SlackAuthoritativeMessageMutation::SavedState,
            cx,
        );
    }

    fn apply_authoritative_slack_message(
        &mut self,
        input: SlackAuthoritativeMessageInput<'_>,
        mutation: SlackAuthoritativeMessageMutation,
        cx: &mut Context<Self>,
    ) {
        let SlackAuthoritativeMessageInput {
            target,
            message,
            row,
            remote_images,
        } = input;
        self.apply_authoritative_slack_message_to_active_snapshots(target, message, mutation);
        let affected_rows = self.apply_authoritative_slack_message_row(target, row, mutation);
        self.apply_authoritative_slack_message_to_secondary_snapshots(target, message, mutation);
        self.slack_remote_images.extend(remote_images);
        self.finish_authoritative_slack_message_row_update(row, affected_rows, cx);
    }

    fn apply_authoritative_slack_message_to_active_snapshots(
        &mut self,
        target: &SlackMessageActionTarget,
        message: &SlackMessage,
        mutation: SlackAuthoritativeMessageMutation,
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
                mutate_slack_messages(&mut snapshot.messages, target, message, mutation);
            }
        }
        if self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == target.team_id()
                && workspace.conversation_id == target.conversation_id()
        }) {
            if let Some(workspace) = self.slack_workspace_mut() {
                mutate_slack_messages(&mut workspace.messages, target, message, mutation);
            }
        }
    }

    fn apply_authoritative_slack_message_to_secondary_snapshots(
        &mut self,
        target: &SlackMessageActionTarget,
        message: &SlackMessage,
        mutation: SlackAuthoritativeMessageMutation,
    ) {
        if let Some(snapshot) = self.slack_all_threads_snapshot.as_mut() {
            for thread in &mut snapshot.threads {
                if thread.conversation_id != target.conversation_id()
                    || thread.thread_timestamp != target.root_timestamp().as_str()
                {
                    continue;
                }
                if thread.parent.id == target.message_timestamp().as_str() {
                    mutate_slack_message(&mut thread.parent, message, mutation);
                }
                mutate_slack_messages(&mut thread.visible_replies, target, message, mutation);
            }
        }
        if let Some(snapshot) = self.slack_pins_snapshot.as_mut() {
            for pin in &mut snapshot.items {
                apply_authoritative_slack_pinned_item(pin, target, message, mutation);
            }
        }
    }
}

fn apply_authoritative_slack_pinned_item(
    pin: &mut SlackPinnedItem,
    target: &SlackMessageActionTarget,
    message: &SlackMessage,
    mutation: SlackAuthoritativeMessageMutation,
) {
    match mutation {
        SlackAuthoritativeMessageMutation::Reactions => {
            mutate_slack_pinned_item_reaction_state(pin, target, &message.reactions);
        }
        SlackAuthoritativeMessageMutation::SavedState => {
            let SlackPinnedItem::Message(pin) = pin else {
                return;
            };
            if pin.conversation_id == target.conversation_id()
                && pin.message.id == target.message_timestamp().as_str()
            {
                mutate_slack_message(&mut pin.message, message, mutation);
            }
        }
    }
}

fn mutate_slack_messages(
    messages: &mut [SlackMessage],
    target: &SlackMessageActionTarget,
    authoritative: &SlackMessage,
    mutation: SlackAuthoritativeMessageMutation,
) -> bool {
    for message in messages {
        if message.id == target.message_timestamp().as_str() {
            mutate_slack_message(message, authoritative, mutation);
            return true;
        }
        if mutate_slack_messages(&mut message.replies, target, authoritative, mutation) {
            return true;
        }
    }
    false
}

fn mutate_slack_message(
    message: &mut SlackMessage,
    authoritative: &SlackMessage,
    mutation: SlackAuthoritativeMessageMutation,
) {
    match mutation {
        SlackAuthoritativeMessageMutation::Reactions => {
            message.reactions.clone_from(&authoritative.reactions);
        }
        SlackAuthoritativeMessageMutation::SavedState => {
            message.saved_state = authoritative.saved_state;
        }
    }
}

fn mutate_slack_pinned_item_reaction_state(
    item: &mut SlackPinnedItem,
    target: &SlackMessageActionTarget,
    reaction_state: &[crate::ui::SlackReaction],
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
