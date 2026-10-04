use std::collections::HashSet;

use crate::model::SlackRemoteDraftFileReference;

use super::SurfaceState;
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraft, SlackComposerDraftId, SlackComposerDraftKey,
    SlackComposerFiles, SlackFileStagingDraftOwner, SlackMainComposerDraftOwner,
    SlackRemoteDraftFileLocator,
};

pub(super) struct SlackDraftFileHydrationContext {
    pub(super) draft_id: SlackComposerDraftId,
    pub(super) has_local_files: bool,
    remote_references: HashSet<SlackRemoteDraftFileReference>,
}

impl SurfaceState {
    pub(super) fn install_hydrated_slack_composer_draft(
        &mut self,
        key: &SlackComposerDraftKey,
        mut draft: SlackComposerDraft,
        token: u64,
    ) {
        draft.token = token;
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            self.restore_slack_send_draft(Some(draft));
            return;
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_mut()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            *panel.reply_draft.get_mut() = draft;
            return;
        }
        self.store_slack_composer_draft(key.clone(), draft);
    }

    pub(super) fn prepare_slack_remote_draft_files(
        &mut self,
        key: &SlackComposerDraftKey,
        draft_id: SlackComposerDraftId,
        references: &[SlackRemoteDraftFileReference],
    ) -> (SlackComposerFiles, Vec<SlackRemoteDraftFileLocator>) {
        let owner = slack_remote_draft_file_owner(key);
        let mut files = SlackComposerFiles::default();
        let mut locators = Vec::with_capacity(references.len());
        for reference in references {
            let file_id = self.next_slack_composer_file_id();
            files.push_remote_loading(file_id, reference.clone());
            locators.push(SlackRemoteDraftFileLocator {
                owner: owner.clone(),
                draft_id,
                file_id,
                reference: reference.clone(),
            });
        }
        (files, locators)
    }

    pub(super) fn merge_hydrated_slack_remote_draft_files(
        &mut self,
        key: &SlackComposerDraftKey,
        references: &[SlackRemoteDraftFileReference],
    ) -> Vec<SlackRemoteDraftFileLocator> {
        let Some(context) = self.slack_draft_file_hydration_context(key) else {
            return Vec::new();
        };
        let missing_references = references
            .iter()
            .filter(|reference| !context.remote_references.contains(*reference))
            .cloned()
            .collect::<Vec<_>>();
        if missing_references.is_empty() {
            return Vec::new();
        }
        let owner = slack_remote_draft_file_owner(key);
        let files = missing_references
            .iter()
            .map(|reference| (self.next_slack_composer_file_id(), reference.clone()))
            .collect::<Vec<_>>();
        let locators = files
            .iter()
            .map(|(file_id, reference)| SlackRemoteDraftFileLocator {
                owner: owner.clone(),
                draft_id: context.draft_id,
                file_id: *file_id,
                reference: reference.clone(),
            })
            .collect::<Vec<_>>();
        self.slack_files_for_owned_composer_draft_mut(&owner, context.draft_id)
            .expect("authoritative Slack draft must remain available while hydrating files")
            .prepend_remote_loading(files);
        locators
    }

    pub(super) fn slack_draft_file_hydration_context(
        &self,
        key: &SlackComposerDraftKey,
    ) -> Option<SlackDraftFileHydrationContext> {
        if self.current_slack_conversation_draft_key().as_ref() == Some(key) {
            if let Some(edit) = self.slack_active_scheduled_edit.as_ref() {
                return Some(slack_draft_file_hydration_context(
                    edit.prior_draft.id,
                    &edit.prior_draft.files,
                ));
            }
            let handle = self.current_slack_main_composer_draft_handle()?;
            return Some(slack_draft_file_hydration_context(
                handle.draft_id,
                &self.slack_composer_files,
            ));
        }
        if let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_draft_key == *key)
        {
            let draft = panel.reply_draft.borrow();
            return Some(slack_draft_file_hydration_context(draft.id, &draft.files));
        }
        self.slack_composer_drafts
            .get(key)
            .map(|draft| slack_draft_file_hydration_context(draft.id, &draft.files))
    }
}

fn slack_remote_draft_file_owner(key: &SlackComposerDraftKey) -> SlackFileStagingDraftOwner {
    match &key.destination {
        SlackComposerDestination::Conversation { .. } => {
            SlackFileStagingDraftOwner::Main(SlackMainComposerDraftOwner::Conversation(key.clone()))
        }
        SlackComposerDestination::Thread { .. } => SlackFileStagingDraftOwner::Thread(key.clone()),
    }
}

fn slack_draft_file_hydration_context(
    draft_id: SlackComposerDraftId,
    files: &SlackComposerFiles,
) -> SlackDraftFileHydrationContext {
    SlackDraftFileHydrationContext {
        draft_id,
        has_local_files: files
            .iter()
            .any(|file| file.state().remote_reference().is_none()),
        remote_references: files
            .iter()
            .filter_map(|file| file.state().remote_reference().cloned())
            .collect(),
    }
}
