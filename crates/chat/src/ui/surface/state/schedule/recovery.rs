use super::{
    slack_remote_loading_file_locators, Context, SlackComposerDraft, SlackFileStagingLocator,
    SlackRemoteDraftFileLocator, SlackScheduleDraftOwner, SlackScheduleDraftRestore,
    SlackScheduledDraftDisposition, SlackSendDraftSource, SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn restore_matching_slack_scheduled_edit_recovery(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_schedule_pending.is_some() || self.slack_active_scheduled_edit.is_some() {
            return false;
        }
        if !self.has_matching_slack_scheduled_edit_recovery() {
            return false;
        }
        let recovery = self
            .slack_scheduled_edit_recovery
            .take()
            .expect("matching scheduled-edit recovery must remain available");
        let failure_diagnostic = recovery.failure_diagnostic;
        self.restore_failed_slack_scheduled_edit(
            recovery.edit,
            recovery.edit_draft,
            recovery.deferred_remote_removals,
            cx,
        );
        self.slack_schedule_submission_error = Some(failure_diagnostic.clone());
        self.slack_error = Some(failure_diagnostic);
        true
    }

    pub(in crate::ui::surface::state) fn has_matching_slack_scheduled_edit_recovery(&self) -> bool {
        self.slack_scheduled_edit_recovery
            .as_ref()
            .is_some_and(|recovery| {
                self.slack_active_main_composer_matches_schedule_identity(
                    &recovery.source,
                    &recovery.edit.team_id,
                    &recovery.edit.self_user_id,
                    &recovery.edit.conversation_id,
                )
            })
    }

    pub(crate) fn has_matching_slack_composer_schedule_recovery(&self) -> bool {
        self.slack_composer_schedule_recovery
            .as_ref()
            .is_some_and(|recovery| self.can_restore_slack_schedule_draft(&recovery.owner))
    }

    pub(crate) fn has_matching_slack_main_composer_schedule_recovery(&self) -> bool {
        self.slack_composer_schedule_recovery
            .as_ref()
            .is_some_and(|recovery| {
                matches!(&recovery.owner, SlackScheduleDraftOwner::Main { .. })
                    && self.can_restore_slack_schedule_draft(&recovery.owner)
            })
    }

    pub(crate) fn has_slack_composer_schedule_recovery_for_active_owner(&self) -> bool {
        self.slack_composer_schedule_recovery
            .as_ref()
            .is_some_and(|recovery| self.slack_schedule_owner_surface_is_current(&recovery.owner))
    }

    pub(super) fn can_restore_slack_schedule_draft(&self, owner: &SlackScheduleDraftOwner) -> bool {
        match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                self.slack_schedule_owner_surface_is_current(owner)
                    && self.slack_active_scheduled_edit.is_none()
                    && self.snapshot_slack_send_draft().is_empty()
            }
            SlackScheduleDraftOwner::ThreadPanel { handle, .. } => {
                if self.slack_workspace().is_none_or(|workspace| {
                    workspace.team_id != handle.key().team_id
                        || workspace.self_user_id.as_deref()
                            != Some(handle.key().self_user_id.as_str())
                }) {
                    return false;
                }
                if self.slack_schedule_owner_surface_is_current(owner) {
                    return self
                        .slack_thread_panel
                        .as_ref()
                        .is_some_and(|panel| panel.reply_draft.borrow().is_empty());
                }
                !self
                    .slack_thread_panel
                    .as_ref()
                    .is_some_and(|panel| panel.reply_draft_key == *handle.key())
                    && self
                        .slack_composer_drafts
                        .get(handle.key())
                        .is_none_or(SlackComposerDraft::is_empty)
            }
            SlackScheduleDraftOwner::AllThreads { handle, .. } => {
                if self.slack_workspace().is_none_or(|workspace| {
                    workspace.team_id != handle.key().team_id
                        || workspace.self_user_id.as_deref()
                            != Some(handle.key().self_user_id.as_str())
                }) {
                    return false;
                }
                !self
                    .slack_thread_panel
                    .as_ref()
                    .is_some_and(|panel| panel.reply_draft_key == *handle.key())
                    && self
                        .slack_composer_drafts
                        .get(handle.key())
                        .is_none_or(SlackComposerDraft::is_empty)
            }
        }
    }

    pub(in crate::ui::surface::state) fn slack_active_main_composer_matches_schedule_identity(
        &self,
        source: &SlackSendDraftSource,
        team_id: &str,
        self_user_id: &str,
        conversation_id: &str,
    ) -> bool {
        self.slack_active_main_composer_context
            .as_ref()
            .is_some_and(|context| {
                &context.source == source
                    && context.target.team_id == team_id
                    && context.target.self_user_id == self_user_id
                    && context.target.conversation_id == conversation_id
            })
    }

    pub(in crate::ui::surface::state) fn slack_composer_schedule_recovery_diagnostic(
        &self,
    ) -> Option<String> {
        self.slack_composer_schedule_recovery
            .as_ref()
            .map(|recovery| {
                format!(
                    "{} The draft remains protected; return to its composer in conversation {} in the original Slack workspace to restore it.",
                    recovery.failure_diagnostic,
                    recovery.identity.conversation_id
                )
            })
    }

    pub(in crate::ui::surface::state) fn restore_matching_slack_composer_schedule_recovery(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if self.slack_schedule_pending.is_some()
            || !self.has_matching_slack_composer_schedule_recovery()
        {
            return None;
        }
        let mut recovery = self
            .slack_composer_schedule_recovery
            .take()
            .expect("matching composer schedule recovery must remain available");
        let failure_diagnostic = recovery.failure_diagnostic.clone();
        let accepted_token = recovery.draft.token;
        let remote_loads = slack_remote_loading_file_locators(&recovery.owner, &recovery.draft);
        let restored = match self.restore_slack_schedule_draft(&recovery.owner, recovery.draft) {
            Ok(restored) => restored,
            Err(draft) => {
                recovery.draft = *draft;
                self.slack_composer_schedule_recovery = Some(recovery);
                return None;
            }
        };
        if let Some(key) = recovery.owner.draft_key().cloned() {
            self.release_slack_draft_schedule_claim(
                key,
                recovery.identity.generation,
                accepted_token,
                cx,
            );
        }
        self.enqueue_slack_remote_draft_file_loads_for_owned_schedule_draft(
            &recovery.owner,
            remote_loads,
            recovery.workspace_api,
            cx,
        );
        self.slack_schedule_submission_error = Some(failure_diagnostic.clone());
        self.set_slack_schedule_owner_error(&recovery.owner, &failure_diagnostic);
        if matches!(restored, SlackScheduleDraftRestore::Active) {
            self.focus_slack_schedule_owner(&recovery.owner);
        }
        self.sync_slack_remote_draft_hydration(cx);
        cx.notify();
        Some(failure_diagnostic)
    }

    pub(in crate::ui::surface::state) fn finish_slack_scheduled_draft(
        &mut self,
        disposition: SlackScheduledDraftDisposition,
        cx: &mut Context<Self>,
    ) {
        let SlackScheduledDraftDisposition {
            owner,
            draft,
            deferred_remote_removals,
            cleanup_api,
        } = disposition;
        let file_owner = owner.file_staging_owner();
        assert_eq!(
            draft.id,
            owner.draft_id(),
            "finishing a Slack scheduled draft requires its exact typed owner"
        );
        for file in draft.files.into_files() {
            let file_id = file.id();
            if let Some(reference) = file.state().remote_reference().cloned() {
                self.cancel_slack_remote_draft_file_load(
                    &SlackRemoteDraftFileLocator {
                        owner: file_owner.clone(),
                        draft_id: draft.id,
                        file_id,
                        reference,
                    },
                    cx,
                );
            } else if let Some(operation_id) = file.operation_id().cloned() {
                self.remove_slack_file_staging_ownership(
                    SlackFileStagingLocator {
                        owner: file_owner.clone(),
                        draft_id: draft.id,
                        file_id,
                        operation_id,
                    },
                    file,
                    cx,
                );
            }
        }
        for locator in deferred_remote_removals {
            if let Some(workspace_api) = cleanup_api.as_ref() {
                self.begin_slack_remote_draft_file_cleanup_with_api(
                    locator,
                    workspace_api.clone(),
                    cx,
                );
            } else {
                self.cancel_slack_remote_draft_file_load(&locator, cx);
            }
        }
    }
}
