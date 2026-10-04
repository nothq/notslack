use super::{
    Context, SlackComposerDocument, SlackComposerDraft, SlackMainComposerDraftOwner,
    SlackMainRoute, SlackWorkspaceUiIdentity, SurfaceState,
};
use crate::ui::surface::{SlackDraftRestore, SlackScheduledEdit, SlackScheduledEditRecovery};

struct SlackWorkspaceComposerRestore {
    failed_conversation_schedule: bool,
    draft: Option<SlackComposerDraft>,
    draft_restore: Option<SlackDraftRestore>,
    composer_draft_text: String,
}

impl SurfaceState {
    pub(super) fn reset_slack_workspace_ui_state(
        &mut self,
        identity: SlackWorkspaceUiIdentity<'_>,
        composer_draft_text: String,
        cx: &mut Context<Self>,
    ) {
        let SlackWorkspaceUiIdentity {
            team_id,
            self_user_id,
            conversation_id,
        } = identity;
        self.reset_slack_schedule_context(cx);
        let draft_restore = self
            .slack_pending_draft_restore
            .take()
            .filter(|restore| restore.conversation_id.as_ref() == conversation_id);
        let scheduled_restore = slack_scheduled_restore(draft_restore.as_ref());
        self.install_slack_routed_main_composer_after_workspace_preparation();
        let failed_scheduled_edit = self.take_matching_slack_scheduled_edit_recovery();
        let failed_conversation_schedule =
            self.has_matching_slack_main_composer_schedule_recovery();
        let failed_scheduled_edit_diagnostic = failed_scheduled_edit
            .as_ref()
            .map(|recovery| recovery.failure_diagnostic.clone());
        let new_message_draft =
            self.take_slack_workspace_new_message_draft(failed_conversation_schedule);
        let has_new_message_draft = new_message_draft.is_some();
        let conversation_draft = self.take_slack_workspace_conversation_draft(
            failed_conversation_schedule,
            team_id,
            self_user_id,
            conversation_id,
        );
        let restored_from_drafts = draft_restore.is_some();
        let conversation_recovery = self.restore_slack_workspace_composer(
            SlackWorkspaceComposerRestore {
                failed_conversation_schedule,
                draft: new_message_draft.or(conversation_draft),
                draft_restore,
                composer_draft_text,
            },
            cx,
        );
        let restored_failed_scheduled_edit = failed_scheduled_edit.is_some();
        self.restore_slack_workspace_scheduled_edit(failed_scheduled_edit, scheduled_restore, cx);
        let recovery_diagnostic = conversation_recovery
            .or(failed_scheduled_edit_diagnostic)
            .or_else(|| self.slack_schedule_blocking_owner_diagnostic());
        let focus_composer = failed_conversation_schedule
            || restored_failed_scheduled_edit
            || (!has_new_message_draft && restored_from_drafts);
        self.finish_slack_workspace_ui_reset(
            conversation_id,
            focus_composer,
            recovery_diagnostic,
            cx,
        );
    }

    fn take_matching_slack_scheduled_edit_recovery(
        &mut self,
    ) -> Option<SlackScheduledEditRecovery> {
        self.has_matching_slack_scheduled_edit_recovery().then(|| {
            self.slack_scheduled_edit_recovery
                .take()
                .expect("matching failed scheduled edit recovery must remain available")
        })
    }

    fn take_slack_workspace_new_message_draft(
        &mut self,
        failed_conversation_schedule: bool,
    ) -> Option<SlackComposerDraft> {
        let mut draft = (!failed_conversation_schedule)
            .then(|| self.active_slack_new_message_draft())
            .flatten()
            .filter(|draft| !draft.is_empty());
        if draft.is_some() || failed_conversation_schedule {
            return draft;
        }
        let parked_owner = self
            .slack_active_main_composer_context
            .as_ref()
            .and_then(|context| {
                matches!(&context.owner, SlackMainComposerDraftOwner::NewMessage(_))
                    .then(|| context.owner.clone())
            });
        draft = parked_owner
            .as_ref()
            .and_then(|owner| self.take_slack_main_composer_owner_draft(owner))
            .filter(|draft| !draft.is_empty());
        draft
    }

    fn take_slack_workspace_conversation_draft(
        &mut self,
        failed_conversation_schedule: bool,
        team_id: &str,
        self_user_id: Option<&str>,
        conversation_id: &str,
    ) -> Option<SlackComposerDraft> {
        (!failed_conversation_schedule)
            .then_some(self_user_id)
            .flatten()
            .filter(|_| self.slack_main_route == SlackMainRoute::Conversation)
            .and_then(|self_user_id| {
                self.take_slack_conversation_draft(team_id, self_user_id, conversation_id)
            })
    }

    fn restore_slack_workspace_composer(
        &mut self,
        restore: SlackWorkspaceComposerRestore,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let SlackWorkspaceComposerRestore {
            failed_conversation_schedule,
            draft,
            draft_restore,
            composer_draft_text,
        } = restore;
        if failed_conversation_schedule {
            self.restore_slack_send_draft(None);
            return Some(
                self.restore_matching_slack_composer_schedule_recovery(cx)
                    .expect("matching conversation schedule recovery must restore atomically"),
            );
        }
        if let Some(draft) = draft {
            self.restore_slack_send_draft(Some(draft));
            return None;
        }
        self.restore_slack_send_draft(None);
        let composer_document = draft_restore
            .as_ref()
            .filter(|restore| restore.scheduled_edit.is_none())
            .map_or_else(
                || SlackComposerDocument::plain_text(composer_draft_text),
                |restore| restore.document.clone(),
            );
        self.replace_slack_send_draft_document(composer_document);
        None
    }

    fn restore_slack_workspace_scheduled_edit(
        &mut self,
        failed_scheduled_edit: Option<SlackScheduledEditRecovery>,
        scheduled_restore: Option<(SlackScheduledEdit, SlackComposerDocument)>,
        cx: &mut Context<Self>,
    ) {
        if let Some(recovery) = failed_scheduled_edit {
            self.restore_failed_slack_scheduled_edit(
                recovery.edit,
                recovery.edit_draft,
                recovery.deferred_remote_removals,
                cx,
            );
        } else if let Some((edit, document)) = scheduled_restore {
            self.begin_slack_scheduled_edit(edit, document, cx);
        }
    }

    fn finish_slack_workspace_ui_reset(
        &mut self,
        conversation_id: &str,
        focus_composer: bool,
        recovery_diagnostic: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        self.slack_composer_focused = focus_composer;
        self.slack_schedule_submission_error
            .clone_from(&recovery_diagnostic);
        self.slack_error = recovery_diagnostic;
        self.slack_aux_panel = None;
        self.reset_slack_channel_menu_context();
        if self.slack_pending_conversation_id.as_deref() == Some(conversation_id) {
            self.slack_pending_conversation_id = None;
        }
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_expanded_attachment = None;
        self.slack_new_message_count = 0;
        self.sync_slack_channel_notification_preference(cx);
        self.sync_slack_preferred_skin_tone(cx);
        self.sync_slack_remote_draft_hydration(cx);
        cx.notify();
    }
}

fn slack_scheduled_restore(
    draft_restore: Option<&SlackDraftRestore>,
) -> Option<(SlackScheduledEdit, SlackComposerDocument)> {
    draft_restore.and_then(|restore| {
        restore
            .scheduled_edit
            .clone()
            .map(|edit| (edit, restore.document.clone()))
    })
}
