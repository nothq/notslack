mod mutation;
mod mutation_payload;
mod reconciliation;
mod tracking;

use std::sync::Arc;
use std::time::Duration;

use crate::model::{
    SlackDraftContent, SlackDraftFileDeletion, SlackDraftUpdateTarget, SlackDraftWriteTarget,
};
use crate::model::{SlackDraftTarget, SlackFileId, SlackMessageClientId};

use super::draft_hydration::load_all_slack_remote_drafts;
use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey,
    SlackDraftDesiredContentFingerprint, SlackDraftDesiredRemote, SlackDraftDesiredState,
    SlackDraftFailureRecovery, SlackDraftLocalPresence, SlackDraftMutation,
    SlackDraftMutationRequest, SlackDraftMutationState, SlackDraftSyncState,
    SlackFileStagingDraftOwner, SlackFileStagingLocator, SlackMainComposerDraftOwner,
    SlackRemoteDraft, SlackRemoteDraftState,
};

use mutation_payload::{
    execute_slack_draft_mutation, prepare_slack_draft_desired_state, prepare_slack_draft_mutation,
    slack_staged_file_token,
};

// The authenticated Slack Desktop 4.51.180 baseline reports
// draft_syncing_reconciliation = null through its loaded KuQ1._Z selector, so module 2bGx
// resolves its draft-sync debounce to 10 seconds.
const SLACK_DRAFT_AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(10);
const SLACK_DRAFT_AUTOSAVE_RETRY_BASE: Duration = Duration::from_millis(500);
const SLACK_DRAFT_AUTOSAVE_MAX_ATTEMPTS: u8 = 3;

#[derive(Clone)]
struct SlackDraftDebounce {
    identity_generation: u64,
    key: SlackComposerDraftKey,
    token: u64,
    local_presence: SlackDraftLocalPresence,
}

enum SlackDraftMutationSuccess {
    Upserted(SlackDraftTarget),
    Deleted,
}

struct SlackDraftAutosaveFailure {
    key: SlackComposerDraftKey,
    token: u64,
    diagnostic: String,
    recovery: SlackDraftFailureRecovery,
}

#[derive(Clone)]
struct SlackDraftRehydration {
    identity_generation: u64,
    identity: crate::ui::surface::SlackDraftSyncIdentity,
    key: SlackComposerDraftKey,
    token: u64,
}

struct SlackDraftResumeState {
    token: u64,
    local_presence: SlackDraftLocalPresence,
    create_client_message_id: Option<SlackMessageClientId>,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn claim_slack_draft_autosave_for_schedule(
        &mut self,
        key: &SlackComposerDraftKey,
        generation: u64,
        token: u64,
    ) -> Result<SlackRemoteDraftState, String> {
        self.validate_slack_draft_schedule_claim(key, token)?;
        let remote = self.slack_remote_draft_for_schedule_claim(key)?;
        self.slack_draft_mutation_states.insert(
            key.clone(),
            SlackDraftMutationState::Scheduling { generation, token },
        );
        Ok(remote)
    }

    fn validate_slack_draft_schedule_claim(
        &self,
        key: &SlackComposerDraftKey,
        token: u64,
    ) -> Result<(), String> {
        if !matches!(
            self.slack_draft_sync_state,
            SlackDraftSyncState::Synchronized
        ) {
            return Err(
                "Wait for Slack draft synchronization to finish before scheduling.".to_string(),
            );
        }
        if self
            .slack_draft_sync_identity
            .as_ref()
            .is_none_or(|identity| {
                identity.team_id != key.team_id || identity.self_user_id != key.self_user_id
            })
        {
            return Err(
                "Slack draft scheduling requires the authenticated draft-sync identity."
                    .to_string(),
            );
        }
        match self.slack_draft_mutation_states.get(key) {
            None => {}
            Some(SlackDraftMutationState::Debouncing {
                token: debounce_token,
                local_presence: SlackDraftLocalPresence::Present,
            }) if *debounce_token == token => {}
            Some(SlackDraftMutationState::Debouncing { .. }) => {
                return Err(
                    "Slack draft autosave changed before scheduling could claim it.".to_string(),
                );
            }
            Some(
                SlackDraftMutationState::InFlight { .. }
                | SlackDraftMutationState::RetryScheduled { .. }
                | SlackDraftMutationState::Rehydrating { .. }
                | SlackDraftMutationState::Scheduling { .. }
                | SlackDraftMutationState::Failed { .. },
            ) => {
                return Err(
                    "Wait for Slack draft synchronization to finish before scheduling.".to_string(),
                );
            }
        }
        Ok(())
    }

    fn slack_remote_draft_for_schedule_claim(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Result<SlackRemoteDraftState, String> {
        let remote = self.slack_remote_drafts.get(key).cloned().ok_or_else(|| {
            "Wait for Slack draft synchronization to finish before scheduling.".to_string()
        })?;
        if matches!(remote, SlackRemoteDraftState::Unknown) {
            return Err(
                "Wait for Slack draft synchronization to finish before scheduling.".to_string(),
            );
        }
        Ok(remote)
    }

    pub(in crate::ui::surface::state) fn confirm_slack_draft_schedule_promotion(
        &mut self,
        key: &SlackComposerDraftKey,
        generation: u64,
        token: u64,
    ) {
        assert!(
            matches!(
                self.slack_draft_mutation_states.get(key),
                Some(SlackDraftMutationState::Scheduling {
                    generation: active_generation,
                    token: active_token,
                }) if *active_generation == generation && *active_token == token
            ),
            "confirming a Slack scheduled-draft promotion requires its exact autosave claim"
        );
        self.slack_draft_mutation_states.remove(key);
        self.slack_draft_mutation_errors.remove(key);
        self.slack_draft_desired_states.remove(key);
        self.slack_remote_drafts
            .insert(key.clone(), SlackRemoteDraftState::Absent);
    }

    pub(in crate::ui::surface::state) fn release_slack_draft_schedule_claim(
        &mut self,
        key: SlackComposerDraftKey,
        generation: u64,
        token: u64,
        cx: &mut Context<Self>,
    ) {
        assert!(
            matches!(
                self.slack_draft_mutation_states.get(&key),
                Some(SlackDraftMutationState::Scheduling {
                    generation: active_generation,
                    token: active_token,
                }) if *active_generation == generation && *active_token == token
            ),
            "releasing a Slack scheduled-draft autosave claim requires its exact owner"
        );
        self.slack_draft_mutation_states.remove(&key);
        self.note_slack_composer_draft_change(key, token, SlackDraftLocalPresence::Present, cx);
    }

    pub(in crate::ui::surface) fn slack_main_draft_autosave_error(&self) -> Option<&str> {
        if self.slack_active_scheduled_edit.is_some() {
            return None;
        }
        let key = self.current_slack_conversation_draft_key()?;
        self.slack_draft_autosave_error(&key)
    }

    pub(in crate::ui::surface) fn slack_draft_autosave_error(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<&str> {
        self.slack_draft_mutation_errors
            .get(key)
            .map(String::as_str)
    }

    pub(in crate::ui::surface) fn slack_main_composer_draft_changed(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_scheduled_edit.is_some() {
            return;
        }
        let Some(key) = self.current_slack_conversation_draft_key() else {
            return;
        };
        self.note_slack_composer_draft_change(
            key,
            self.slack_send_draft_token,
            SlackDraftLocalPresence::Present,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn slack_composer_draft_changed(
        &mut self,
        key: SlackComposerDraftKey,
        token: u64,
        cx: &mut Context<Self>,
    ) {
        let local_presence = if self.slack_authoritative_composer_draft(&key).is_some() {
            SlackDraftLocalPresence::Present
        } else {
            SlackDraftLocalPresence::Absent
        };
        self.note_slack_composer_draft_change(key, token, local_presence, cx);
    }

    pub(in crate::ui::surface::state) fn slack_staged_file_became_autosave_ready(
        &mut self,
        locator: &SlackFileStagingLocator,
        cx: &mut Context<Self>,
    ) {
        let key = match &locator.owner {
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key))
            | SlackFileStagingDraftOwner::Thread(key) => key.clone(),
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::NewMessage(_)) => {
                return;
            }
        };
        let Some(token) = self.slack_authoritative_staged_file_token(&key, locator) else {
            return;
        };
        if !matches!(
            self.slack_draft_desired_states.get(&key),
            Some(SlackDraftDesiredState::WaitingForLocalFiles {
                token: waiting_token,
                ..
            }) if *waiting_token == token
        ) {
            return;
        }
        let create_client_message_id = self.slack_draft_create_client_message_id(&key);
        self.slack_draft_desired_states.insert(
            key.clone(),
            SlackDraftDesiredState::Pending {
                token,
                local_presence: SlackDraftLocalPresence::Present,
                create_client_message_id,
            },
        );
        self.reconcile_slack_draft_now(key, token, cx);
    }

    pub(in crate::ui::surface::state) fn resume_slack_draft_autosave_after_hydration(
        &mut self,
        key: SlackComposerDraftKey,
        captured_token: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let Some(resume) = self.slack_draft_resume_state(&key, captured_token) else {
            return;
        };
        let SlackDraftResumeState {
            token,
            local_presence,
            create_client_message_id,
        } = resume;
        self.slack_draft_desired_states.insert(
            key.clone(),
            SlackDraftDesiredState::Pending {
                token,
                local_presence,
                create_client_message_id,
            },
        );
        self.slack_draft_mutation_states.insert(
            key.clone(),
            SlackDraftMutationState::Debouncing {
                token,
                local_presence,
            },
        );
        self.finish_slack_draft_debounce(
            SlackDraftDebounce {
                identity_generation: self.slack_draft_sync_generation,
                key,
                token,
                local_presence,
            },
            cx,
        );
    }

    fn slack_draft_resume_state(
        &self,
        key: &SlackComposerDraftKey,
        captured_token: Option<u64>,
    ) -> Option<SlackDraftResumeState> {
        let create_client_message_id = self.slack_draft_create_client_message_id(key);
        let pending = self
            .slack_draft_desired_states
            .get(key)
            .and_then(|desired| match desired {
                SlackDraftDesiredState::WaitingForRemote {
                    token,
                    local_presence,
                    create_client_message_id,
                }
                | SlackDraftDesiredState::Pending {
                    token,
                    local_presence,
                    create_client_message_id,
                } => Some(SlackDraftResumeState {
                    token: *token,
                    local_presence: *local_presence,
                    create_client_message_id: create_client_message_id.clone(),
                }),
                SlackDraftDesiredState::WaitingForLocalFiles { .. }
                | SlackDraftDesiredState::Absent { .. }
                | SlackDraftDesiredState::Present(_) => None,
            });
        let fallback = self
            .slack_authoritative_composer_draft(key)
            .map(|draft| SlackDraftResumeState {
                token: draft.token,
                local_presence: SlackDraftLocalPresence::Present,
                create_client_message_id: create_client_message_id.clone(),
            })
            .or_else(|| {
                captured_token.map(|token| SlackDraftResumeState {
                    token,
                    local_presence: SlackDraftLocalPresence::Absent,
                    create_client_message_id,
                })
            });
        pending.or(fallback)
    }

    pub(in crate::ui::surface::state) fn reset_slack_draft_autosave(&mut self) {
        self.slack_draft_desired_states.clear();
        self.slack_draft_mutation_states.clear();
        self.slack_draft_mutation_errors.clear();
    }
}
