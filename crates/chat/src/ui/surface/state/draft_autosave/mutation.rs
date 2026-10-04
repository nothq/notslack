use super::{
    execute_slack_draft_mutation, slack_staged_file_token, Arc, Context, SlackComposerDraft,
    SlackComposerDraftKey, SlackDraftAutosaveFailure, SlackDraftDebounce, SlackDraftDesiredState,
    SlackDraftFailureRecovery, SlackDraftLocalPresence, SlackDraftMutation,
    SlackDraftMutationRequest, SlackDraftMutationState, SlackDraftMutationSuccess,
    SlackFileStagingLocator, SlackMessageClientId, SlackRemoteDraft, SlackRemoteDraftState,
    SurfaceState, WorkspaceApi, SLACK_DRAFT_AUTOSAVE_MAX_ATTEMPTS, SLACK_DRAFT_AUTOSAVE_RETRY_BASE,
};

impl SurfaceState {
    pub(super) fn spawn_slack_draft_mutation(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackDraftMutationRequest,
        cx: &mut Context<Self>,
    ) {
        self.slack_draft_mutation_states.insert(
            request.key.clone(),
            SlackDraftMutationState::InFlight {
                serial: request.serial,
                token: request.token,
            },
        );
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = execute_slack_draft_mutation(&workspace_api, &request.mutation);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_draft_mutation(request, result, cx);
            },
        );
    }

    fn finish_slack_draft_mutation(
        &mut self,
        request: SlackDraftMutationRequest,
        result: Result<SlackDraftMutationSuccess, String>,
        cx: &mut Context<Self>,
    ) {
        if request.identity_generation != self.slack_draft_sync_generation
            || !matches!(
                self.slack_draft_mutation_states.get(&request.key),
                Some(SlackDraftMutationState::InFlight { serial, token })
                    if *serial == request.serial && *token == request.token
            )
        {
            return;
        }
        match result {
            Ok(success) => {
                self.apply_slack_draft_mutation_success(&request, success);
                self.slack_draft_mutation_states.remove(&request.key);
                self.slack_draft_mutation_errors.remove(&request.key);
                if self.slack_draft_has_newer_pending_state(&request) {
                    self.reconcile_pending_slack_draft_now(request.key, cx);
                }
            }
            Err(_) if request.attempt < SLACK_DRAFT_AUTOSAVE_MAX_ATTEMPTS => {
                self.schedule_slack_draft_retry(request, cx);
            }
            Err(error) => {
                self.fail_slack_draft_autosave(
                    SlackDraftAutosaveFailure {
                        key: request.key,
                        token: request.token,
                        diagnostic: format!("Slack could not save this draft: {error}"),
                        recovery: SlackDraftFailureRecovery::AuthoritativeHydration,
                    },
                    cx,
                );
            }
        }
    }

    fn schedule_slack_draft_retry(
        &mut self,
        mut request: SlackDraftMutationRequest,
        cx: &mut Context<Self>,
    ) {
        self.slack_draft_mutation_states.insert(
            request.key.clone(),
            SlackDraftMutationState::RetryScheduled {
                serial: request.serial,
                token: request.token,
            },
        );
        let delay = SLACK_DRAFT_AUTOSAVE_RETRY_BASE
            .checked_mul(1_u32 << (request.attempt - 1))
            .expect("Slack draft retry duration overflowed");
        request.attempt += 1;
        self.spawn_timer_task(request, delay, cx, |this, request, cx| {
            if request.identity_generation != this.slack_draft_sync_generation
                || !matches!(
                    this.slack_draft_mutation_states.get(&request.key),
                    Some(SlackDraftMutationState::RetryScheduled { serial, token })
                        if *serial == request.serial && *token == request.token
                )
            {
                return;
            }
            let Some(workspace_api) = this.active_slack_workspace_api() else {
                return;
            };
            this.spawn_slack_draft_mutation(workspace_api, request, cx);
        });
    }

    fn apply_slack_draft_mutation_success(
        &mut self,
        request: &SlackDraftMutationRequest,
        success: SlackDraftMutationSuccess,
    ) {
        match (&request.mutation, success) {
            (
                SlackDraftMutation::Create {
                    client_message_id,
                    write_target,
                    content,
                }
                | SlackDraftMutation::Update {
                    client_message_id,
                    write_target,
                    content,
                    ..
                },
                SlackDraftMutationSuccess::Upserted(target),
            ) => {
                self.slack_remote_drafts.insert(
                    request.key.clone(),
                    SlackRemoteDraftState::Present(Arc::new(SlackRemoteDraft {
                        target,
                        write_target: write_target.clone(),
                        client_message_id: client_message_id.clone(),
                        desired_content_fingerprint: content.clone(),
                    })),
                );
            }
            (SlackDraftMutation::Delete { .. }, SlackDraftMutationSuccess::Deleted) => {
                self.slack_remote_drafts
                    .insert(request.key.clone(), SlackRemoteDraftState::Absent);
                self.clear_slack_draft_create_client_message_id(&request.key);
            }
            _ => unreachable!("Slack draft mutation returned a mismatched receipt"),
        }
    }

    fn slack_draft_has_newer_pending_state(&self, request: &SlackDraftMutationRequest) -> bool {
        matches!(
            self.slack_draft_desired_states.get(&request.key),
            Some(SlackDraftDesiredState::Pending { token, .. }) if *token != request.token
        )
    }

    pub(super) fn slack_draft_create_client_message_id(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<SlackMessageClientId> {
        match self.slack_draft_desired_states.get(key)? {
            SlackDraftDesiredState::WaitingForRemote {
                create_client_message_id,
                ..
            }
            | SlackDraftDesiredState::Pending {
                create_client_message_id,
                ..
            }
            | SlackDraftDesiredState::WaitingForLocalFiles {
                create_client_message_id,
                ..
            } => create_client_message_id.clone(),
            SlackDraftDesiredState::Absent { .. } => None,
            SlackDraftDesiredState::Present(desired) => Some(desired.client_message_id.clone()),
        }
    }

    fn clear_slack_draft_create_client_message_id(&mut self, key: &SlackComposerDraftKey) {
        match self.slack_draft_desired_states.get_mut(key) {
            Some(
                SlackDraftDesiredState::WaitingForRemote {
                    create_client_message_id,
                    ..
                }
                | SlackDraftDesiredState::Pending {
                    create_client_message_id,
                    ..
                }
                | SlackDraftDesiredState::WaitingForLocalFiles {
                    create_client_message_id,
                    ..
                },
            ) => *create_client_message_id = None,
            Some(SlackDraftDesiredState::Absent { .. } | SlackDraftDesiredState::Present(_))
            | None => {}
        }
    }

    fn reconcile_pending_slack_draft_now(
        &mut self,
        key: SlackComposerDraftKey,
        cx: &mut Context<Self>,
    ) {
        let Some((token, local_presence)) =
            self.slack_draft_desired_states
                .get(&key)
                .and_then(|desired| match desired {
                    SlackDraftDesiredState::Pending {
                        token,
                        local_presence,
                        ..
                    } => Some((*token, *local_presence)),
                    _ => None,
                })
        else {
            return;
        };
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

    pub(super) fn fail_slack_draft_autosave(
        &mut self,
        failure: SlackDraftAutosaveFailure,
        cx: &mut Context<Self>,
    ) {
        let token = self
            .slack_draft_desired_token(&failure.key)
            .unwrap_or(failure.token);
        self.slack_draft_mutation_errors
            .insert(failure.key.clone(), failure.diagnostic);
        self.slack_draft_mutation_states.insert(
            failure.key,
            SlackDraftMutationState::Failed {
                token,
                recovery: failure.recovery,
            },
        );
        cx.notify();
    }

    pub(super) fn slack_draft_desired_token(&self, key: &SlackComposerDraftKey) -> Option<u64> {
        match self.slack_draft_desired_states.get(key)? {
            SlackDraftDesiredState::WaitingForRemote { token, .. }
            | SlackDraftDesiredState::Pending { token, .. }
            | SlackDraftDesiredState::WaitingForLocalFiles { token, .. }
            | SlackDraftDesiredState::Absent { token, .. } => Some(*token),
            SlackDraftDesiredState::Present(desired) => Some(desired.token),
        }
    }

    pub(super) fn slack_projection_matches(
        &self,
        key: &SlackComposerDraftKey,
        token: u64,
        local_presence: SlackDraftLocalPresence,
    ) -> bool {
        match (local_presence, self.slack_authoritative_composer_draft(key)) {
            (SlackDraftLocalPresence::Present, Some(draft)) => draft.token == token,
            (SlackDraftLocalPresence::Absent, None) => true,
            _ => false,
        }
    }

    pub(in crate::ui::surface::state) fn slack_authoritative_composer_draft(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<SlackComposerDraft> {
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            return Some(self.slack_active_scheduled_edit.as_ref().map_or_else(
                || self.snapshot_slack_send_draft(),
                |edit| edit.prior_draft.clone(),
            ));
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            return Some(panel.reply_draft.borrow().clone());
        }
        self.slack_composer_drafts.get(key).cloned()
    }

    pub(super) fn slack_authoritative_composer_file_limit_diagnostic(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<&'static str> {
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            return self.slack_active_scheduled_edit.as_ref().map_or_else(
                || self.slack_composer_files.validate_slack_file_limit().err(),
                |edit| edit.prior_draft.files.validate_slack_file_limit().err(),
            );
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            return panel
                .reply_draft
                .borrow()
                .files
                .validate_slack_file_limit()
                .err();
        }
        self.slack_composer_drafts
            .get(key)?
            .files
            .validate_slack_file_limit()
            .err()
    }

    pub(super) fn slack_authoritative_staged_file_token(
        &self,
        key: &SlackComposerDraftKey,
        locator: &SlackFileStagingLocator,
    ) -> Option<u64> {
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            if let Some(edit) = self.slack_active_scheduled_edit.as_ref() {
                return slack_staged_file_token(
                    edit.prior_draft_handle.draft_id,
                    edit.prior_draft.token,
                    &edit.prior_draft.files,
                    locator,
                );
            }
            let handle = self.current_slack_main_composer_draft_handle()?;
            return slack_staged_file_token(
                handle.draft_id,
                self.slack_send_draft_token,
                &self.slack_composer_files,
                locator,
            );
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            let draft = panel.reply_draft.borrow();
            return slack_staged_file_token(draft.id, draft.token, &draft.files, locator);
        }
        let draft = self.slack_composer_drafts.get(key)?;
        slack_staged_file_token(draft.id, draft.token, &draft.files, locator)
    }
}
