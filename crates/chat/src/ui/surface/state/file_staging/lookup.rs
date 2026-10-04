use super::SurfaceState;
use crate::ui::surface::{
    SlackComposerDraftId, SlackComposerFile, SlackComposerFiles, SlackFileStagingDraftOwner,
    SlackFileStagingLocator, SlackMainComposerDraftOwner,
};

impl SurfaceState {
    pub(super) fn slack_file_for_staging_mut(
        &mut self,
        locator: &SlackFileStagingLocator,
    ) -> Option<&mut SlackComposerFile> {
        let files = self.slack_files_for_staging_mut(locator)?;
        let file = files.file_mut(locator.file_id)?;
        (file.operation_id() == Some(&locator.operation_id)).then_some(file)
    }

    pub(crate) fn slack_file_staging_owner_is_current(
        &self,
        owner: &SlackFileStagingDraftOwner,
    ) -> bool {
        let Some(workspace) = self.slack_workspace() else {
            return false;
        };
        let Some(self_user_id) = workspace.self_user_id.as_deref() else {
            return false;
        };
        match owner {
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key))
            | SlackFileStagingDraftOwner::Thread(key) => {
                key.team_id == workspace.team_id && key.self_user_id == self_user_id
            }
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::NewMessage(key)) => {
                key.team_id == workspace.team_id && key.self_user_id == self_user_id
            }
        }
    }

    fn slack_files_for_staging_mut(
        &mut self,
        locator: &SlackFileStagingLocator,
    ) -> Option<&mut SlackComposerFiles> {
        self.slack_files_for_owned_composer_draft_mut(&locator.owner, locator.draft_id)
    }

    pub(crate) fn slack_files_for_owned_composer_draft_mut(
        &mut self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> Option<&mut SlackComposerFiles> {
        if self.active_slack_owned_composer_files_matches(owner, draft_id) {
            return Some(&mut self.slack_composer_files);
        }
        if self.slack_scheduled_owned_composer_files_matches(owner, draft_id) {
            return self.slack_scheduled_owned_composer_files_mut(owner, draft_id);
        }
        if let Some(index) = self.slack_outbound_owned_composer_index(owner, draft_id) {
            return self
                .slack_outbound_delivery_ledger
                .get_mut(index)
                .map(|delivery| &mut delivery.accepted_draft.files);
        }
        self.slack_stored_owned_composer_files_mut(owner, draft_id)
    }

    fn active_slack_owned_composer_files_matches(
        &self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> bool {
        match owner {
            SlackFileStagingDraftOwner::Main(owner) => self
                .current_slack_main_composer_draft_handle()
                .is_some_and(|handle| handle.owner == *owner && handle.draft_id == draft_id),
            SlackFileStagingDraftOwner::Thread(_) => false,
        }
    }

    fn slack_scheduled_owned_composer_files_matches(
        &self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> bool {
        let prior_matches = match owner {
            SlackFileStagingDraftOwner::Main(owner) => self
                .slack_active_scheduled_edit
                .as_ref()
                .is_some_and(|active| {
                    active.prior_draft_handle.owner == *owner
                        && active.prior_draft_handle.draft_id == draft_id
                }),
            SlackFileStagingDraftOwner::Thread(_) => false,
        };
        prior_matches
            || self
                .slack_schedule_pending
                .as_ref()
                .is_some_and(|pending| pending.owner.matches_owned_draft(owner, draft_id))
            || self
                .slack_composer_schedule_recovery
                .as_ref()
                .is_some_and(|recovery| recovery.owner.matches_owned_draft(owner, draft_id))
    }

    fn slack_scheduled_owned_composer_files_mut(
        &mut self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> Option<&mut SlackComposerFiles> {
        let prior_matches = match owner {
            SlackFileStagingDraftOwner::Main(owner) => self
                .slack_active_scheduled_edit
                .as_ref()
                .is_some_and(|active| {
                    active.prior_draft_handle.owner == *owner
                        && active.prior_draft_handle.draft_id == draft_id
                }),
            SlackFileStagingDraftOwner::Thread(_) => false,
        };
        if prior_matches {
            return self
                .slack_active_scheduled_edit
                .as_mut()
                .map(|active| &mut active.prior_draft.files);
        }
        if self
            .slack_schedule_pending
            .as_ref()
            .is_some_and(|pending| pending.owner.matches_owned_draft(owner, draft_id))
        {
            return self
                .slack_schedule_pending
                .as_mut()
                .map(|pending| &mut pending.accepted_draft.files);
        }
        self.slack_composer_schedule_recovery
            .as_mut()
            .filter(|recovery| recovery.owner.matches_owned_draft(owner, draft_id))
            .map(|recovery| &mut recovery.draft.files)
    }

    fn slack_outbound_owned_composer_index(
        &self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> Option<usize> {
        match owner {
            SlackFileStagingDraftOwner::Main(owner) => self
                .slack_outbound_delivery_ledger
                .iter()
                .position(|delivery| {
                    delivery.accepted_draft_handle.owner == *owner
                        && delivery.accepted_draft_handle.draft_id == draft_id
                }),
            SlackFileStagingDraftOwner::Thread(_) => None,
        }
    }

    fn slack_stored_owned_composer_files_mut(
        &mut self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> Option<&mut SlackComposerFiles> {
        match owner {
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key)) => {
                let return_draft_matches = self
                    .slack_new_message_return_draft
                    .as_ref()
                    .is_some_and(|(return_key, draft)| return_key == key && draft.id == draft_id);
                if return_draft_matches {
                    return self
                        .slack_new_message_return_draft
                        .as_mut()
                        .map(|(_, draft)| &mut draft.files);
                }
                self.slack_composer_drafts
                    .get_mut(key)
                    .filter(|draft| draft.id == draft_id)
                    .map(|draft| &mut draft.files)
            }
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::NewMessage(key)) => self
                .slack_new_message_drafts
                .get_mut(key)
                .filter(|draft| draft.id == draft_id)
                .map(|draft| &mut draft.files),
            SlackFileStagingDraftOwner::Thread(key) => {
                let panel_matches = self.slack_thread_panel.as_ref().is_some_and(|panel| {
                    panel.reply_draft_key == *key && panel.reply_draft.borrow().id == draft_id
                });
                if panel_matches {
                    return self
                        .slack_thread_panel
                        .as_mut()
                        .map(|panel| &mut panel.reply_draft.get_mut().files);
                }
                self.slack_composer_drafts
                    .get_mut(key)
                    .filter(|draft| draft.id == draft_id)
                    .map(|draft| &mut draft.files)
            }
        }
    }
}
