use super::{
    prepare_slack_draft_desired_state, prepare_slack_draft_mutation, Context,
    SlackComposerDraftKey, SlackDraftAutosaveFailure, SlackDraftDebounce, SlackDraftDesiredState,
    SlackDraftFailureRecovery, SlackDraftLocalPresence, SlackDraftMutation,
    SlackDraftMutationRequest, SlackDraftMutationState, SlackMessageClientId,
    SlackRemoteDraftState, SurfaceState, WorkspaceApi, SLACK_DRAFT_AUTOSAVE_DEBOUNCE,
};

struct PreparedSlackDraftReconciliation {
    desired: SlackDraftDesiredState,
    mutation: Option<SlackDraftMutation>,
}

struct SlackDraftReconciliationInput {
    key: SlackComposerDraftKey,
    token: u64,
    remote: SlackRemoteDraftState,
    create_client_message_id: Option<SlackMessageClientId>,
}

struct SlackDraftMutationStart {
    workspace_api: std::sync::Arc<dyn WorkspaceApi>,
    key: SlackComposerDraftKey,
    token: u64,
    desired: SlackDraftDesiredState,
    mutation: SlackDraftMutation,
}

impl SurfaceState {
    pub(super) fn schedule_slack_draft_debounce(
        &mut self,
        key: SlackComposerDraftKey,
        token: u64,
        local_presence: SlackDraftLocalPresence,
        cx: &mut Context<Self>,
    ) {
        self.slack_draft_mutation_states.insert(
            key.clone(),
            SlackDraftMutationState::Debouncing {
                token,
                local_presence,
            },
        );
        let debounce = SlackDraftDebounce {
            identity_generation: self.slack_draft_sync_generation,
            key,
            token,
            local_presence,
        };
        self.spawn_timer_task(
            debounce,
            SLACK_DRAFT_AUTOSAVE_DEBOUNCE,
            cx,
            |this, debounce, cx| this.finish_slack_draft_debounce(debounce, cx),
        );
    }

    pub(super) fn finish_slack_draft_debounce(
        &mut self,
        debounce: SlackDraftDebounce,
        cx: &mut Context<Self>,
    ) {
        if debounce.identity_generation != self.slack_draft_sync_generation
            || !matches!(
                self.slack_draft_mutation_states.get(&debounce.key),
                Some(SlackDraftMutationState::Debouncing {
                    token,
                    local_presence,
                }) if *token == debounce.token && *local_presence == debounce.local_presence
            )
            || !matches!(
                self.slack_draft_desired_states.get(&debounce.key),
                Some(SlackDraftDesiredState::Pending {
                    token,
                    local_presence,
                    ..
                }) if *token == debounce.token && *local_presence == debounce.local_presence
            )
        {
            return;
        }
        if !self.slack_projection_matches(&debounce.key, debounce.token, debounce.local_presence) {
            self.slack_draft_desired_states.remove(&debounce.key);
            self.slack_draft_mutation_states.remove(&debounce.key);
            return;
        }
        self.reconcile_slack_draft_now(debounce.key, debounce.token, cx);
    }

    pub(super) fn reconcile_slack_draft_now(
        &mut self,
        key: SlackComposerDraftKey,
        token: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let remote = self
            .slack_remote_drafts
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let input = SlackDraftReconciliationInput {
            create_client_message_id: self.slack_draft_create_client_message_id(&key),
            key,
            token,
            remote,
        };
        if self.defer_slack_draft_reconciliation(&input, cx) {
            return;
        }
        let Some(prepared) = self.prepare_slack_draft_reconciliation_or_fail(&input, cx) else {
            return;
        };
        self.finish_prepared_slack_draft_reconciliation(workspace_api, input, prepared, cx);
    }

    fn prepare_slack_draft_reconciliation(
        &self,
        input: &SlackDraftReconciliationInput,
    ) -> Result<PreparedSlackDraftReconciliation, String> {
        let draft = self.slack_authoritative_composer_draft(&input.key);
        let desired = prepare_slack_draft_desired_state(
            &input.key,
            input.token,
            draft.as_ref(),
            &input.remote,
            input.create_client_message_id.clone(),
        )?;
        let mutation = prepare_slack_draft_mutation(&input.remote, &desired)?;
        Ok(PreparedSlackDraftReconciliation { desired, mutation })
    }

    fn prepare_slack_draft_reconciliation_or_fail(
        &mut self,
        input: &SlackDraftReconciliationInput,
        cx: &mut Context<Self>,
    ) -> Option<PreparedSlackDraftReconciliation> {
        match self.prepare_slack_draft_reconciliation(input) {
            Ok(prepared) => Some(prepared),
            Err(error) => {
                self.fail_slack_draft_autosave(
                    SlackDraftAutosaveFailure {
                        key: input.key.clone(),
                        token: input.token,
                        diagnostic: format!("Slack could not save this draft: {error}"),
                        recovery: SlackDraftFailureRecovery::NextSemanticChange,
                    },
                    cx,
                );
                None
            }
        }
    }

    fn finish_prepared_slack_draft_reconciliation(
        &mut self,
        workspace_api: std::sync::Arc<dyn WorkspaceApi>,
        input: SlackDraftReconciliationInput,
        prepared: PreparedSlackDraftReconciliation,
        cx: &mut Context<Self>,
    ) {
        let Some(mutation) = prepared.mutation else {
            self.slack_draft_desired_states
                .insert(input.key.clone(), prepared.desired);
            self.slack_draft_mutation_states.remove(&input.key);
            self.slack_draft_mutation_errors.remove(&input.key);
            return;
        };
        self.start_slack_draft_mutation(
            SlackDraftMutationStart {
                workspace_api,
                key: input.key,
                token: input.token,
                desired: prepared.desired,
                mutation,
            },
            cx,
        );
    }

    fn defer_slack_draft_reconciliation(
        &mut self,
        input: &SlackDraftReconciliationInput,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(input.remote, SlackRemoteDraftState::Unknown) {
            let local_presence = self
                .slack_authoritative_composer_draft(&input.key)
                .map_or(SlackDraftLocalPresence::Absent, |_| {
                    SlackDraftLocalPresence::Present
                });
            self.slack_draft_desired_states.insert(
                input.key.clone(),
                SlackDraftDesiredState::WaitingForRemote {
                    token: input.token,
                    local_presence,
                    create_client_message_id: input.create_client_message_id.clone(),
                },
            );
            return true;
        }
        if let Some(diagnostic) =
            self.slack_authoritative_composer_file_limit_diagnostic(&input.key)
        {
            self.fail_slack_draft_autosave(
                SlackDraftAutosaveFailure {
                    key: input.key.clone(),
                    token: input.token,
                    diagnostic: diagnostic.to_string(),
                    recovery: SlackDraftFailureRecovery::ComposerFileLimit,
                },
                cx,
            );
            return true;
        }
        let has_local_pending = self
            .slack_authoritative_composer_draft(&input.key)
            .is_some_and(|draft| draft.files.has_local_pending());
        if has_local_pending {
            self.slack_draft_desired_states.insert(
                input.key.clone(),
                SlackDraftDesiredState::WaitingForLocalFiles {
                    token: input.token,
                    create_client_message_id: input.create_client_message_id.clone(),
                },
            );
            self.slack_draft_mutation_states.remove(&input.key);
            return true;
        }
        false
    }

    fn start_slack_draft_mutation(
        &mut self,
        start: SlackDraftMutationStart,
        cx: &mut Context<Self>,
    ) {
        let SlackDraftMutationStart {
            workspace_api,
            key,
            token,
            desired,
            mutation,
        } = start;
        self.slack_draft_desired_states.insert(key.clone(), desired);
        self.slack_draft_mutation_serial = self
            .slack_draft_mutation_serial
            .checked_add(1)
            .expect("Slack draft mutation serial overflowed");
        let request = SlackDraftMutationRequest {
            identity_generation: self.slack_draft_sync_generation,
            serial: self.slack_draft_mutation_serial,
            key,
            token,
            attempt: 1,
            mutation,
        };
        self.spawn_slack_draft_mutation(workspace_api, request, cx);
    }
}
