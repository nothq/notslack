mod files;
mod loading;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::model::SlackRemoteDraftFileReference;

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    SlackComposerDocument, SlackComposerDraft, SlackComposerDraftId, SlackComposerDraftKey,
    SlackComposerFiles, SlackDraftMutationState, SlackDraftSyncIdentity, SlackDraftSyncState,
    SlackMainComposerDraftOwner, SlackRemoteDraft, SlackRemoteDraftFileLocator,
    SlackRemoteDraftState,
};
pub(in crate::ui::surface::state) use loading::load_all_slack_remote_drafts;
use loading::slack_draft_key_matches_identity;

#[derive(Clone)]
struct SlackDraftHydrationRequest {
    generation: u64,
    identity: SlackDraftSyncIdentity,
    local_draft_token_generation: u64,
    local_draft_tokens: HashMap<SlackComposerDraftKey, u64>,
}

struct SlackDraftHydrationInstallation {
    file_loads: Vec<SlackRemoteDraftFileLocator>,
    reconcile_keys: HashSet<SlackComposerDraftKey>,
}

pub(super) struct PreparedSlackRemoteDraft {
    document: SlackComposerDocument,
    broadcast: bool,
    file_references: Arc<[SlackRemoteDraftFileReference]>,
    pub(super) remote: Arc<SlackRemoteDraft>,
}

impl PreparedSlackRemoteDraft {
    fn into_composer_draft(
        self,
        id: SlackComposerDraftId,
        files: SlackComposerFiles,
    ) -> SlackComposerDraft {
        SlackComposerDraft {
            id,
            token: 0,
            client_message_id: Some(self.remote.client_message_id.clone()),
            document: self.document,
            files,
            broadcast: self.broadcast,
        }
    }
}

pub(super) struct PreparedSlackRemoteDrafts {
    pub(super) drafts: HashMap<SlackComposerDraftKey, PreparedSlackRemoteDraft>,
}

impl SurfaceState {
    pub(in crate::ui::surface) fn sync_slack_remote_draft_hydration(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_schedule_protects_draft_sync() {
            return;
        }
        let Some(identity) = self.active_slack_draft_sync_identity() else {
            self.reset_slack_remote_draft_hydration();
            return;
        };
        if self.slack_draft_sync_identity.as_ref() != Some(&identity) {
            self.reset_slack_remote_draft_hydration();
            self.slack_draft_sync_identity = Some(identity.clone());
        }
        if !self.slack_workspace_api_capabilities.load_drafts_sent
            || !matches!(self.slack_draft_sync_state, SlackDraftSyncState::Inactive)
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };

        self.slack_draft_sync_generation = self
            .slack_draft_sync_generation
            .checked_add(1)
            .expect("Slack draft hydration generation overflowed");
        let local_draft_tokens = self.capture_slack_local_draft_tokens(&identity);
        for key in local_draft_tokens.keys() {
            self.slack_remote_drafts
                .insert(key.clone(), SlackRemoteDraftState::Unknown);
        }
        let request = SlackDraftHydrationRequest {
            generation: self.slack_draft_sync_generation,
            identity,
            local_draft_token_generation: self.slack_send_draft_token_generation,
            local_draft_tokens,
        };
        self.slack_draft_sync_state = SlackDraftSyncState::Loading;
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = load_all_slack_remote_drafts(&workspace_api, &request.identity);
                (workspace_api, request, result)
            },
            |this, (workspace_api, request, result), cx| {
                this.finish_slack_remote_draft_hydration(workspace_api, request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface) fn reset_slack_remote_draft_hydration(&mut self) {
        if self.slack_schedule_protects_draft_sync() {
            return;
        }
        if self.slack_draft_sync_identity.is_none()
            && matches!(self.slack_draft_sync_state, SlackDraftSyncState::Inactive)
            && self.slack_remote_drafts.is_empty()
            && self.slack_draft_desired_states.is_empty()
            && self.slack_draft_mutation_states.is_empty()
            && self.slack_draft_mutation_errors.is_empty()
        {
            return;
        }
        self.slack_draft_sync_generation = self
            .slack_draft_sync_generation
            .checked_add(1)
            .expect("Slack draft hydration generation overflowed");
        self.slack_draft_sync_identity = None;
        self.slack_draft_sync_state = SlackDraftSyncState::Inactive;
        self.slack_remote_drafts.clear();
        self.reset_slack_remote_draft_file_loads();
        self.reset_slack_draft_autosave();
    }

    fn slack_schedule_protects_draft_sync(&self) -> bool {
        self.slack_schedule_pending.is_some()
            || self.slack_composer_schedule_recovery.is_some()
            || self
                .slack_draft_mutation_states
                .values()
                .any(|state| matches!(state, SlackDraftMutationState::Scheduling { .. }))
    }

    fn finish_slack_remote_draft_hydration(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackDraftHydrationRequest,
        result: Result<PreparedSlackRemoteDrafts, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_draft_hydration_request_is_current(&request) {
            return;
        }
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_draft_sync_state = SlackDraftSyncState::Failed(format!(
                    "Slack draft synchronization failed: {error}"
                ));
                cx.notify();
                return;
            }
        };
        self.mark_known_slack_remote_drafts_absent(&request);
        let installation = self.install_prepared_slack_remote_drafts(&request, prepared);
        self.complete_slack_remote_draft_hydration(workspace_api, request, installation, cx);
    }

    fn slack_draft_hydration_request_is_current(
        &self,
        request: &SlackDraftHydrationRequest,
    ) -> bool {
        self.slack_draft_sync_generation == request.generation
            && self.slack_draft_sync_identity.as_ref() == Some(&request.identity)
            && self.active_slack_draft_sync_identity().as_ref() == Some(&request.identity)
            && matches!(self.slack_draft_sync_state, SlackDraftSyncState::Loading)
    }

    fn mark_known_slack_remote_drafts_absent(&mut self, request: &SlackDraftHydrationRequest) {
        let mut known_keys = self.slack_known_local_draft_keys(&request.identity);
        known_keys.extend(request.local_draft_tokens.keys().cloned());
        known_keys.extend(
            self.slack_remote_drafts
                .keys()
                .filter(|key| slack_draft_key_matches_identity(key, &request.identity))
                .cloned(),
        );
        for key in &known_keys {
            self.slack_remote_drafts
                .insert(key.clone(), SlackRemoteDraftState::Absent);
        }
    }

    fn install_prepared_slack_remote_drafts(
        &mut self,
        request: &SlackDraftHydrationRequest,
        prepared: PreparedSlackRemoteDrafts,
    ) -> SlackDraftHydrationInstallation {
        let absent_local_keys_are_unchanged =
            self.slack_send_draft_token_generation == request.local_draft_token_generation;
        let mut file_loads = Vec::new();
        let mut reconcile_keys = request
            .local_draft_tokens
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        for (key, prepared_draft) in prepared.drafts {
            let captured_token = request.local_draft_tokens.get(&key).copied();
            let (locators, requires_reconciliation) = self.install_prepared_slack_remote_draft(
                &key,
                prepared_draft,
                captured_token,
                absent_local_keys_are_unchanged,
            );
            file_loads.extend(locators);
            if requires_reconciliation {
                reconcile_keys.insert(key);
            }
        }
        SlackDraftHydrationInstallation {
            file_loads,
            reconcile_keys,
        }
    }

    fn install_prepared_slack_remote_draft(
        &mut self,
        key: &SlackComposerDraftKey,
        prepared: PreparedSlackRemoteDraft,
        captured_token: Option<u64>,
        absent_local_keys_are_unchanged: bool,
    ) -> (Vec<SlackRemoteDraftFileLocator>, bool) {
        let current_token = self.slack_local_draft_token(key);
        self.slack_remote_drafts.insert(
            key.clone(),
            SlackRemoteDraftState::Present(prepared.remote.clone()),
        );
        let local_draft_is_unchanged = match captured_token {
            Some(captured_token) => current_token == Some(captured_token),
            None => current_token.is_none() && absent_local_keys_are_unchanged,
        };
        let preserves_existing_local_files = local_draft_is_unchanged
            && self
                .slack_draft_file_hydration_context(key)
                .is_some_and(|context| context.has_local_files);
        if local_draft_is_unchanged && !preserves_existing_local_files {
            let draft_id = self.next_slack_composer_draft_id();
            let (files, locators) =
                self.prepare_slack_remote_draft_files(key, draft_id, &prepared.file_references);
            self.install_hydrated_slack_composer_draft(
                key,
                prepared.into_composer_draft(draft_id, files),
                current_token.unwrap_or_default(),
            );
            return (locators, false);
        }
        (
            self.merge_hydrated_slack_remote_draft_files(key, &prepared.file_references),
            true,
        )
    }

    fn complete_slack_remote_draft_hydration(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackDraftHydrationRequest,
        mut installation: SlackDraftHydrationInstallation,
        cx: &mut Context<Self>,
    ) {
        self.slack_draft_sync_state = SlackDraftSyncState::Synchronized;
        installation.reconcile_keys.extend(
            self.slack_draft_desired_states
                .keys()
                .filter(|key| slack_draft_key_matches_identity(key, &request.identity))
                .cloned(),
        );
        for key in installation.reconcile_keys {
            self.resume_slack_draft_autosave_after_hydration(
                key.clone(),
                request.local_draft_tokens.get(&key).copied(),
                cx,
            );
        }
        self.enqueue_slack_remote_draft_file_loads(installation.file_loads, workspace_api, cx);
        if let Some(panel) = self.slack_thread_panel.as_ref() {
            panel.list_state.remeasure();
        }
        self.slack_all_threads_list_state.remeasure();
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn active_slack_draft_sync_identity(
        &self,
    ) -> Option<SlackDraftSyncIdentity> {
        let workspace = self.slack_workspace()?;
        let self_user_id = workspace.self_user_id.as_ref()?;
        (!workspace.team_id.is_empty() && !self_user_id.is_empty()).then(|| {
            SlackDraftSyncIdentity {
                team_id: workspace.team_id.clone(),
                self_user_id: self_user_id.clone(),
            }
        })
    }

    fn capture_slack_local_draft_tokens(
        &self,
        identity: &SlackDraftSyncIdentity,
    ) -> HashMap<SlackComposerDraftKey, u64> {
        let mut tokens = self
            .slack_composer_drafts
            .iter()
            .filter(|(key, _)| slack_draft_key_matches_identity(key, identity))
            .map(|(key, draft)| (key.clone(), draft.token))
            .collect::<HashMap<_, _>>();
        if let Some(key) = self.current_slack_conversation_draft_key() {
            tokens.insert(key, self.slack_send_draft_token);
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| slack_draft_key_matches_identity(&panel.reply_draft_key, identity))
        {
            tokens.insert(
                panel.reply_draft_key.clone(),
                panel.reply_draft.borrow().token,
            );
        }
        tokens
    }

    fn slack_known_local_draft_keys(
        &self,
        identity: &SlackDraftSyncIdentity,
    ) -> HashSet<SlackComposerDraftKey> {
        let mut keys = self
            .slack_composer_drafts
            .keys()
            .filter(|key| slack_draft_key_matches_identity(key, identity))
            .cloned()
            .collect::<HashSet<_>>();
        if let Some(key) = self.current_slack_conversation_draft_key() {
            keys.insert(key);
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| slack_draft_key_matches_identity(&panel.reply_draft_key, identity))
        {
            keys.insert(panel.reply_draft_key.clone());
        }
        keys
    }

    pub(in crate::ui::surface::state) fn current_slack_conversation_draft_key(
        &self,
    ) -> Option<SlackComposerDraftKey> {
        let SlackMainComposerDraftOwner::Conversation(key) =
            &self.slack_active_main_composer_context.as_ref()?.owner
        else {
            return None;
        };
        Some(key.clone())
    }

    fn slack_local_draft_token(&self, key: &SlackComposerDraftKey) -> Option<u64> {
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            return Some(self.slack_send_draft_token);
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            return Some(panel.reply_draft.borrow().token);
        }
        self.slack_composer_drafts.get(key).map(|draft| draft.token)
    }
}
