use super::{
    Context, SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey,
    SlackMainComposerDraftOwner, SlackWorkspace, SurfaceState,
};
use crate::ui::surface::SlackFileStagingDraftOwner;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn store_current_slack_conversation_draft(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.restore_slack_scheduled_edit_prior_draft(cx);
        let Some(context) = self.slack_active_main_composer_context.as_ref() else {
            return;
        };
        let SlackMainComposerDraftOwner::Conversation(key) = &context.owner else {
            return;
        };
        let key = key.clone();
        let owner = context.owner.clone();
        let draft = self.take_slack_send_draft();
        self.clear_slack_main_composer_after_draft_taken(&owner);
        self.store_slack_composer_draft(key, draft);
    }

    pub(in crate::ui::surface::state) fn prepare_slack_drafts_for_workspace(
        &mut self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) {
        let same_identity = self.slack_workspace().is_some_and(|current| {
            current.team_id == workspace.team_id
                && current.self_user_id == workspace.self_user_id
                && workspace.self_user_id.is_some()
        });
        if same_identity {
            self.store_current_slack_conversation_draft(cx);
        } else {
            self.discard_slack_drafts_for_workspace_identity_change(cx);
        }
    }

    pub(in crate::ui::surface) fn discard_slack_drafts_for_workspace_identity_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.reset_slack_schedule_context(cx);
        self.reset_slack_remote_draft_hydration();
        self.discard_active_slack_workspace_drafts(cx);
        self.discard_stored_slack_workspace_drafts(cx);
    }

    fn discard_active_slack_workspace_drafts(&mut self, cx: &mut Context<Self>) {
        if let Some(context) = self.slack_active_main_composer_context.clone() {
            let handle = self
                .current_slack_main_composer_draft_handle()
                .expect("active Slack main composer context must own its draft");
            let draft = self.take_slack_send_draft();
            assert_eq!(
                handle.draft_id, draft.id,
                "discarding a Slack workspace must retain the exact active draft identity"
            );
            self.clear_slack_main_composer_after_draft_taken(&context.owner);
            self.discard_slack_composer_draft(
                SlackFileStagingDraftOwner::Main(context.owner),
                draft,
                cx,
            );
            self.restore_slack_send_draft(None);
        } else {
            assert!(
                self.slack_composer_files.is_empty(),
                "an ownerless Slack workspace cannot retain active composer files"
            );
            self.restore_slack_send_draft(None);
        }
        let thread_panel = self.slack_thread_panel.take();
        self.discard_slack_thread_context();
        if let Some(panel) = thread_panel {
            self.discard_slack_composer_draft(
                SlackFileStagingDraftOwner::Thread(panel.reply_draft_key),
                panel.reply_draft.into_inner(),
                cx,
            );
        }
    }

    fn discard_stored_slack_workspace_drafts(&mut self, cx: &mut Context<Self>) {
        let stored_drafts = self.slack_composer_drafts.drain().collect::<Vec<_>>();
        for (key, draft) in stored_drafts {
            self.discard_slack_stored_composer_draft(key, draft, cx);
        }
        let new_message_drafts = self.slack_new_message_drafts.drain().collect::<Vec<_>>();
        for (key, draft) in new_message_drafts {
            self.discard_slack_composer_draft(
                SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::NewMessage(key)),
                draft,
                cx,
            );
        }
        if let Some((key, draft)) = self.slack_new_message_return_draft.take() {
            self.discard_slack_composer_draft(
                SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key)),
                draft,
                cx,
            );
        }
    }

    pub(super) fn take_slack_conversation_draft(
        &mut self,
        team_id: &str,
        self_user_id: &str,
        conversation_id: &str,
    ) -> Option<SlackComposerDraft> {
        self.slack_composer_drafts.remove(&SlackComposerDraftKey {
            team_id: team_id.to_string(),
            self_user_id: self_user_id.to_string(),
            destination: SlackComposerDestination::Conversation {
                conversation_id: conversation_id.to_string(),
            },
        })
    }

    pub(crate) fn slack_composer_draft_key(
        &self,
        destination: SlackComposerDestination,
    ) -> Option<SlackComposerDraftKey> {
        let workspace = self.slack_workspace()?;
        let self_user_id = workspace.self_user_id.clone()?;
        (!workspace.team_id.is_empty() && !self_user_id.is_empty()).then(|| SlackComposerDraftKey {
            team_id: workspace.team_id.clone(),
            self_user_id,
            destination,
        })
    }

    pub(crate) fn store_slack_composer_draft(
        &mut self,
        key: SlackComposerDraftKey,
        draft: SlackComposerDraft,
    ) {
        if let Some(existing) = self.slack_composer_drafts.get(&key) {
            assert!(
                existing.files.is_empty()
                    || (existing.id == draft.id
                        && existing.files.same_identity_and_order(&draft.files)),
                "storing a Slack composer draft cannot replace a different file-owning draft"
            );
        }
        if draft.is_empty() {
            self.slack_composer_drafts.remove(&key);
        } else {
            self.slack_composer_drafts.insert(key, draft);
        }
    }
}
