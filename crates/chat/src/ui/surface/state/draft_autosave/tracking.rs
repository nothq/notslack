use super::{
    load_all_slack_remote_drafts, Context, SlackComposerDraftKey, SlackDraftDesiredState,
    SlackDraftFailureRecovery, SlackDraftLocalPresence, SlackDraftMutationState,
    SlackDraftRehydration, SlackDraftSyncState, SlackMessageClientId, SlackRemoteDraftState,
    SurfaceState,
};

struct SlackDraftChangeReadiness {
    mutation_in_progress: bool,
    requires_authoritative_hydration: bool,
    remains_blocked_on_file_limit: bool,
    clears_error_on_semantic_change: bool,
}

struct SlackDraftChange {
    key: SlackComposerDraftKey,
    token: u64,
    local_presence: SlackDraftLocalPresence,
    create_client_message_id: Option<SlackMessageClientId>,
}

struct SlackDraftRemoteDeferral {
    change: SlackDraftChange,
    requires_authoritative_hydration: bool,
}

impl SurfaceState {
    pub(super) fn note_slack_composer_draft_change(
        &mut self,
        key: SlackComposerDraftKey,
        token: u64,
        local_presence: SlackDraftLocalPresence,
        cx: &mut Context<Self>,
    ) {
        let Some(change) = self.prepare_slack_draft_change(key, token, local_presence) else {
            return;
        };
        let readiness = self.slack_draft_change_readiness(&change.key);
        if readiness.clears_error_on_semantic_change {
            self.slack_draft_mutation_errors.remove(&change.key);
        }
        if !self.slack_remote_draft_is_known(&change.key) {
            self.defer_slack_draft_change_for_remote(
                SlackDraftRemoteDeferral {
                    change,
                    requires_authoritative_hydration: readiness.requires_authoritative_hydration,
                },
                cx,
            );
            return;
        }
        self.apply_known_slack_draft_change(change, readiness, cx);
    }

    fn prepare_slack_draft_change(
        &self,
        key: SlackComposerDraftKey,
        token: u64,
        local_presence: SlackDraftLocalPresence,
    ) -> Option<SlackDraftChange> {
        if !self.slack_workspace_api_capabilities.mutate_drafts
            || self
                .slack_draft_sync_identity
                .as_ref()
                .is_none_or(|identity| {
                    identity.team_id != key.team_id || identity.self_user_id != key.self_user_id
                })
        {
            return None;
        }
        Some(SlackDraftChange {
            create_client_message_id: self.slack_draft_create_client_message_id(&key),
            key,
            token,
            local_presence,
        })
    }

    fn apply_known_slack_draft_change(
        &mut self,
        change: SlackDraftChange,
        readiness: SlackDraftChangeReadiness,
        cx: &mut Context<Self>,
    ) {
        let SlackDraftChange {
            key,
            token,
            local_presence,
            create_client_message_id,
        } = change;
        self.slack_draft_desired_states.insert(
            key.clone(),
            SlackDraftDesiredState::Pending {
                token,
                local_presence,
                create_client_message_id,
            },
        );
        if readiness.mutation_in_progress {
            return;
        }
        if readiness.remains_blocked_on_file_limit {
            self.slack_draft_mutation_states.insert(
                key,
                SlackDraftMutationState::Failed {
                    token,
                    recovery: SlackDraftFailureRecovery::ComposerFileLimit,
                },
            );
            return;
        }
        if readiness.requires_authoritative_hydration {
            self.begin_slack_draft_rehydration(key, token, cx);
            return;
        }
        self.schedule_slack_draft_debounce(key, token, local_presence, cx);
    }

    fn defer_slack_draft_change_for_remote(
        &mut self,
        deferral: SlackDraftRemoteDeferral,
        cx: &mut Context<Self>,
    ) {
        let SlackDraftRemoteDeferral {
            change:
                SlackDraftChange {
                    key,
                    token,
                    local_presence,
                    create_client_message_id,
                },
            requires_authoritative_hydration,
        } = deferral;
        self.slack_draft_desired_states.insert(
            key.clone(),
            SlackDraftDesiredState::WaitingForRemote {
                token,
                local_presence,
                create_client_message_id,
            },
        );
        if requires_authoritative_hydration
            || matches!(self.slack_draft_sync_state, SlackDraftSyncState::Failed(_))
        {
            self.begin_slack_draft_rehydration(key, token, cx);
        }
    }

    fn slack_draft_change_readiness(
        &self,
        key: &SlackComposerDraftKey,
    ) -> SlackDraftChangeReadiness {
        let mutation_state = self.slack_draft_mutation_states.get(key);
        let mutation_in_progress = matches!(
            mutation_state,
            Some(
                SlackDraftMutationState::InFlight { .. }
                    | SlackDraftMutationState::RetryScheduled { .. }
                    | SlackDraftMutationState::Rehydrating { .. }
                    | SlackDraftMutationState::Scheduling { .. }
            )
        );
        let requires_authoritative_hydration = matches!(
            mutation_state,
            Some(SlackDraftMutationState::Failed {
                recovery: SlackDraftFailureRecovery::AuthoritativeHydration,
                ..
            })
        );
        let failed_on_file_limit = matches!(
            mutation_state,
            Some(SlackDraftMutationState::Failed {
                recovery: SlackDraftFailureRecovery::ComposerFileLimit,
                ..
            })
        );
        let remains_blocked_on_file_limit = failed_on_file_limit
            && self
                .slack_authoritative_composer_file_limit_diagnostic(key)
                .is_some();
        let clears_error_on_semantic_change = matches!(
            mutation_state,
            Some(SlackDraftMutationState::Failed {
                recovery: SlackDraftFailureRecovery::NextSemanticChange,
                ..
            })
        ) || failed_on_file_limit
            && !remains_blocked_on_file_limit;
        SlackDraftChangeReadiness {
            mutation_in_progress,
            requires_authoritative_hydration,
            remains_blocked_on_file_limit,
            clears_error_on_semantic_change,
        }
    }

    fn slack_remote_draft_is_known(&mut self, key: &SlackComposerDraftKey) -> bool {
        match self.slack_remote_drafts.get(key) {
            Some(SlackRemoteDraftState::Absent | SlackRemoteDraftState::Present(_)) => true,
            Some(SlackRemoteDraftState::Unknown) => false,
            None if matches!(
                self.slack_draft_sync_state,
                SlackDraftSyncState::Synchronized
            ) =>
            {
                self.slack_remote_drafts
                    .insert(key.clone(), SlackRemoteDraftState::Absent);
                true
            }
            None => false,
        }
    }

    fn begin_slack_draft_rehydration(
        &mut self,
        key: SlackComposerDraftKey,
        token: u64,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_drafts_sent {
            return;
        }
        let Some(identity) = self.active_slack_draft_sync_identity() else {
            return;
        };
        if identity.team_id != key.team_id || identity.self_user_id != key.self_user_id {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let request = SlackDraftRehydration {
            identity_generation: self.slack_draft_sync_generation,
            identity,
            key: key.clone(),
            token,
        };
        self.slack_remote_drafts
            .insert(key.clone(), SlackRemoteDraftState::Unknown);
        self.slack_draft_mutation_states
            .insert(key, SlackDraftMutationState::Rehydrating { token });
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = load_all_slack_remote_drafts(&workspace_api, &request.identity);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_draft_rehydration(request, result, cx);
            },
        );
    }

    fn finish_slack_draft_rehydration(
        &mut self,
        request: SlackDraftRehydration,
        result: Result<super::super::draft_hydration::PreparedSlackRemoteDrafts, String>,
        cx: &mut Context<Self>,
    ) {
        if request.identity_generation != self.slack_draft_sync_generation
            || self.active_slack_draft_sync_identity().as_ref() != Some(&request.identity)
            || !matches!(
                self.slack_draft_mutation_states.get(&request.key),
                Some(SlackDraftMutationState::Rehydrating { token })
                    if *token == request.token
            )
        {
            return;
        }
        let mut prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                let token = self
                    .slack_draft_desired_token(&request.key)
                    .unwrap_or(request.token);
                self.slack_draft_mutation_errors
                    .entry(request.key.clone())
                    .or_insert_with(|| format!("Slack could not refresh this draft: {error}"));
                self.slack_draft_mutation_states.insert(
                    request.key,
                    SlackDraftMutationState::Failed {
                        token,
                        recovery: SlackDraftFailureRecovery::AuthoritativeHydration,
                    },
                );
                cx.notify();
                return;
            }
        };
        let remote = prepared
            .drafts
            .remove(&request.key)
            .map_or(SlackRemoteDraftState::Absent, |draft| {
                SlackRemoteDraftState::Present(draft.remote)
            });
        self.slack_remote_drafts.insert(request.key.clone(), remote);
        self.resume_slack_draft_autosave_after_hydration(request.key, None, cx);
    }
}
