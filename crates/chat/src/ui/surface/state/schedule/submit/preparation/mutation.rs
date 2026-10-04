use super::super::super::super::{Context, SurfaceState};
use super::super::execution::{schedule_client_mutation_timestamp, slack_schedule_write_target};
use super::super::{SlackPreparedScheduleMutation, SlackScheduleMutationInput};
use crate::ui::surface::{
    SlackComposerDestination, SlackRemoteDraftState, SlackScheduleDraftOwner,
    SlackScheduledPendingMutation, SlackScheduledPendingOrigin,
};

impl SurfaceState {
    pub(super) fn prepare_slack_schedule_mutation(
        &mut self,
        input: SlackScheduleMutationInput<'_>,
        cx: &mut Context<Self>,
    ) -> Option<SlackPreparedScheduleMutation> {
        if self.slack_active_scheduled_edit.is_some() {
            self.prepare_slack_scheduled_edit_mutation(input, cx)
        } else {
            self.prepare_slack_composer_schedule_mutation(input, cx)
        }
    }

    fn prepare_slack_scheduled_edit_mutation(
        &mut self,
        input: SlackScheduleMutationInput<'_>,
        cx: &mut Context<Self>,
    ) -> Option<SlackPreparedScheduleMutation> {
        let active_edit = self
            .slack_active_scheduled_edit
            .as_ref()
            .expect("scheduled-edit mutation requires its active edit");
        assert!(
            matches!(input.owner, SlackScheduleDraftOwner::Main { .. }),
            "Slack scheduled-edit mutation must retain a main composer owner"
        );
        assert!(
            active_edit.edit.team_id == input.identity.team_id
                && active_edit.edit.self_user_id == input.identity.self_user_id
                && active_edit.edit.conversation_id == input.identity.conversation_id,
            "Slack scheduled-edit mutation must match its authenticated identity"
        );
        let timestamp = match schedule_client_mutation_timestamp() {
            Ok(timestamp) => timestamp,
            Err(message) => {
                self.show_slack_schedule_error(input.owner, &message, cx);
                return None;
            }
        };
        Some(SlackPreparedScheduleMutation {
            mutation: SlackScheduledPendingMutation::Update {
                edit: active_edit.edit.clone(),
                client_mutation_timestamp: timestamp,
            },
            origin: SlackScheduledPendingOrigin::ScheduledEdit {
                edit: Box::new(active_edit.edit.clone()),
                prior_draft_handle: active_edit.prior_draft_handle.clone(),
                deferred_remote_removals: active_edit.deferred_remote_removals.clone(),
            },
            client_message_id: active_edit.edit.client_message_id.clone(),
        })
    }

    fn prepare_slack_composer_schedule_mutation(
        &mut self,
        input: SlackScheduleMutationInput<'_>,
        cx: &mut Context<Self>,
    ) -> Option<SlackPreparedScheduleMutation> {
        let write_target = match slack_schedule_write_target(
            &input.draft_identity.destination,
            input.draft.broadcast,
        ) {
            Ok(write_target) => write_target,
            Err(message) => {
                self.show_slack_schedule_error(input.owner, &message, cx);
                return None;
            }
        };
        let remote = self.claim_slack_schedule_remote_state(&input, cx)?;
        self.prepare_claimed_slack_schedule_mutation(input, write_target, remote, cx)
    }

    fn claim_slack_schedule_remote_state(
        &mut self,
        input: &SlackScheduleMutationInput<'_>,
        cx: &mut Context<Self>,
    ) -> Option<SlackRemoteDraftState> {
        let Some(key) = input.draft_identity.key.as_ref() else {
            return Some(SlackRemoteDraftState::Absent);
        };
        assert!(
            key.team_id == input.identity.team_id
                && key.self_user_id == input.identity.self_user_id
                && matches!(
                    &key.destination,
                    SlackComposerDestination::Conversation { conversation_id }
                        | SlackComposerDestination::Thread {
                            conversation_id, ..
                        } if conversation_id == &input.identity.conversation_id
                ),
            "Slack scheduled-draft autosave claim must match its mutation identity"
        );
        match self.claim_slack_draft_autosave_for_schedule(
            key,
            input.identity.generation,
            input.draft.token,
        ) {
            Ok(remote) => Some(remote),
            Err(message) => {
                self.show_slack_schedule_error(input.owner, &message, cx);
                None
            }
        }
    }

    fn prepare_claimed_slack_schedule_mutation(
        &mut self,
        input: SlackScheduleMutationInput<'_>,
        write_target: crate::model::SlackDraftWriteTarget,
        remote: SlackRemoteDraftState,
        cx: &mut Context<Self>,
    ) -> Option<SlackPreparedScheduleMutation> {
        let (mutation, client_message_id) = match remote {
            SlackRemoteDraftState::Absent => {
                let client_message_id = input
                    .draft
                    .client_message_id
                    .clone()
                    .unwrap_or_else(crate::model::SlackMessageClientId::generate);
                (
                    SlackScheduledPendingMutation::Create {
                        client_message_id: client_message_id.clone(),
                        write_target,
                    },
                    client_message_id,
                )
            }
            SlackRemoteDraftState::Present(remote) => {
                let timestamp = match schedule_client_mutation_timestamp() {
                    Ok(timestamp) => timestamp,
                    Err(message) => {
                        if let Some(key) = input.draft_identity.key.as_ref() {
                            self.release_slack_draft_schedule_claim(
                                key.clone(),
                                input.identity.generation,
                                input.draft.token,
                                cx,
                            );
                        }
                        self.show_slack_schedule_error(input.owner, &message, cx);
                        return None;
                    }
                };
                (
                    SlackScheduledPendingMutation::Promote {
                        target: remote.target.clone(),
                        write_target,
                        client_mutation_timestamp: timestamp,
                    },
                    remote.client_message_id.clone(),
                )
            }
            SlackRemoteDraftState::Unknown => {
                unreachable!("schedule autosave claim rejects unknown remote state")
            }
        };
        Some(SlackPreparedScheduleMutation {
            mutation,
            origin: SlackScheduledPendingOrigin::Composer,
            client_message_id,
        })
    }
}
