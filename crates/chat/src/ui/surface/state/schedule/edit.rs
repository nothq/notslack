use super::{
    slack_remote_loading_file_locators, Context, SlackActiveScheduledEdit, SlackComposerDocument,
    SlackComposerDraft, SlackMainComposerDraftHandle, SlackRemoteDraftFileLocator,
    SlackScheduleDraftOwner, SlackScheduledDraftDisposition, SlackScheduledEdit, SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_scheduled_edit(
        &mut self,
        edit: SlackScheduledEdit,
        document: SlackComposerDocument,
        cx: &mut Context<Self>,
    ) {
        if self.slack_schedule_pending.is_some()
            || self.slack_composer_schedule_recovery.is_some()
            || self.slack_scheduled_delete_pending.is_some()
            || self.slack_scheduled_edit_recovery.is_some()
        {
            return;
        }
        if !self.validate_slack_scheduled_edit_owner(&edit, cx) {
            return;
        }
        self.activate_slack_scheduled_edit(edit, document, cx);
    }

    fn validate_slack_scheduled_edit_owner(
        &mut self,
        edit: &SlackScheduledEdit,
        cx: &mut Context<Self>,
    ) -> bool {
        let schedule_owner = self
            .current_slack_main_schedule_owner()
            .expect("Slack scheduled-message editing requires an active main composer");
        let identity_matches = self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == edit.team_id
                && workspace.self_user_id.as_deref() == Some(edit.self_user_id.as_str())
                && workspace.conversation_id == edit.conversation_id
        });
        if !identity_matches {
            self.show_slack_schedule_error(
                &schedule_owner,
                "Wait for the scheduled message workspace and conversation to finish loading.",
                cx,
            );
            return false;
        }
        true
    }

    fn activate_slack_scheduled_edit(
        &mut self,
        edit: SlackScheduledEdit,
        document: SlackComposerDocument,
        cx: &mut Context<Self>,
    ) {
        self.restore_slack_scheduled_edit_prior_draft(cx);
        let prior_draft_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("Slack scheduled-message editing requires an active main composer");
        let prior_draft = self.take_slack_send_draft();
        assert_eq!(
            prior_draft_handle.draft_id, prior_draft.id,
            "Slack scheduled-message editing must retain the exact prior draft identity"
        );
        self.restore_slack_send_draft(None);
        self.replace_slack_send_draft_document(document);
        let edit_draft_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("Slack scheduled-message editing requires an active main composer");
        let remote_files = edit
            .remote_files
            .iter()
            .cloned()
            .map(|reference| (self.next_slack_composer_file_id(), reference))
            .collect::<Vec<_>>();
        let remote_loads = remote_files
            .iter()
            .map(|(file_id, reference)| {
                SlackRemoteDraftFileLocator::main(&edit_draft_handle, *file_id, reference.clone())
            })
            .collect::<Vec<_>>();
        self.slack_composer_files
            .prepend_remote_loading(remote_files);
        self.advance_slack_send_draft_revision();
        self.slack_send_client_message_id = Some(edit.client_message_id.clone());
        self.slack_schedule_submission_error = None;
        self.slack_active_scheduled_edit = Some(SlackActiveScheduledEdit {
            edit,
            edit_draft_handle,
            prior_draft_handle,
            prior_draft,
            deferred_remote_removals: Vec::new(),
        });
        if let Some(workspace_api) = self.active_slack_workspace_api() {
            self.enqueue_slack_remote_draft_file_loads(remote_loads, workspace_api, cx);
        }
        self.slack_composer_focused = true;
    }

    pub(in crate::ui::surface::state) fn restore_slack_scheduled_edit_prior_draft(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(active_edit) = self.slack_active_scheduled_edit.take() else {
            return false;
        };
        let edit_draft_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("restoring a Slack scheduled edit requires its active main composer");
        assert_eq!(
            edit_draft_handle, active_edit.edit_draft_handle,
            "restoring a Slack scheduled edit must discard the exact edit draft"
        );
        let edit_draft = self.take_slack_send_draft();
        assert_eq!(
            edit_draft.id, active_edit.edit_draft_handle.draft_id,
            "Slack scheduled edit draft handle must match its draft"
        );
        let source = self
            .slack_active_main_composer_context
            .as_ref()
            .expect("restoring a Slack scheduled edit requires its active main composer")
            .source
            .clone();
        self.finish_slack_scheduled_draft(
            SlackScheduledDraftDisposition {
                owner: SlackScheduleDraftOwner::Main {
                    handle: active_edit.edit_draft_handle,
                    source,
                },
                draft: edit_draft,
                deferred_remote_removals: active_edit.deferred_remote_removals,
                cleanup_api: None,
            },
            cx,
        );
        assert_eq!(
            active_edit.prior_draft_handle.draft_id, active_edit.prior_draft.id,
            "Slack scheduled-message prior draft handle must match its draft"
        );
        self.restore_slack_send_draft(Some(active_edit.prior_draft));
        true
    }

    pub(in crate::ui::surface::state) fn restore_failed_slack_scheduled_edit(
        &mut self,
        edit: SlackScheduledEdit,
        edit_draft: SlackComposerDraft,
        deferred_remote_removals: Vec<SlackRemoteDraftFileLocator>,
        cx: &mut Context<Self>,
    ) {
        assert!(
            self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == edit.team_id
                    && workspace.self_user_id.as_deref() == Some(edit.self_user_id.as_str())
                    && workspace.conversation_id == edit.conversation_id
            }),
            "failed scheduled edit recovery requires its exact authenticated conversation"
        );
        let prior_draft_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("restoring a failed scheduled edit requires an active main composer");
        let prior_draft = self.take_slack_send_draft();
        self.restore_slack_send_draft(None);
        let restored_handle = SlackMainComposerDraftHandle {
            owner: prior_draft_handle.owner.clone(),
            draft_id: edit_draft.id,
        };
        let restored_owner = SlackScheduleDraftOwner::Main {
            handle: restored_handle,
            source: self
                .slack_active_main_composer_context
                .as_ref()
                .expect("restoring a failed scheduled edit requires its active source")
                .source
                .clone(),
        };
        let remote_loads = slack_remote_loading_file_locators(&restored_owner, &edit_draft);
        self.restore_slack_send_draft(Some(edit_draft));
        let edit_draft_handle = self
            .current_slack_main_composer_draft_handle()
            .expect("restored scheduled edit requires an active main composer");
        self.slack_active_scheduled_edit = Some(SlackActiveScheduledEdit {
            edit,
            edit_draft_handle,
            prior_draft_handle,
            prior_draft,
            deferred_remote_removals,
        });
        if let Some(workspace_api) = self.active_slack_workspace_api() {
            self.enqueue_slack_remote_draft_file_loads(remote_loads, workspace_api, cx);
        }
        self.slack_composer_focused = true;
    }
}
