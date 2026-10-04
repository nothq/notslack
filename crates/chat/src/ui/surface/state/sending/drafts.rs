use super::SurfaceState;
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDocument, SlackComposerDraft, SlackComposerDraftId,
    SlackComposerFileId, SlackComposerFiles, SlackMainComposerDraftHandle,
    SlackMainComposerDraftOwner, SlackNewMessageDraftKey, SlackSendDraftSource, SlackSendRequest,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn advance_slack_send_draft_revision(&mut self) {
        self.slack_send_draft_revision = self
            .slack_send_draft_revision
            .checked_add(1)
            .expect("Slack send draft revision overflowed");
        self.slack_send_draft_token = self.next_slack_composer_draft_token();
        self.slack_send_client_message_id = None;
    }

    pub(crate) fn next_slack_composer_draft_id(&mut self) -> SlackComposerDraftId {
        self.slack_composer_ids.next_draft_id()
    }

    pub(crate) fn next_slack_composer_file_id(&mut self) -> SlackComposerFileId {
        self.slack_composer_ids.next_file_id()
    }

    pub(crate) fn next_slack_composer_draft_token(&mut self) -> u64 {
        self.slack_send_draft_token_generation = self
            .slack_send_draft_token_generation
            .checked_add(1)
            .expect("Slack send draft token overflowed");
        self.slack_send_draft_token_generation
    }

    pub(crate) fn replace_slack_send_draft_text(&mut self, text: String) -> bool {
        if self.slack_composer_text == text {
            return false;
        }
        self.slack_composer_text = text;
        self.advance_slack_send_draft_revision();
        true
    }

    pub(crate) fn replace_slack_send_draft_document(
        &mut self,
        document: SlackComposerDocument,
    ) -> bool {
        let text = document.text().to_string();
        let changed = self.slack_composer_text != text
            || !self
                .slack_composer_document
                .borrow()
                .same_draft_state(&document);
        if !changed {
            return false;
        }
        self.slack_composer_text = text;
        *self.slack_composer_document.get_mut() = document;
        self.advance_slack_send_draft_revision();
        true
    }

    pub(crate) fn replace_slack_send_draft_with_plain_text(&mut self, text: String) -> bool {
        self.replace_slack_send_draft_document(SlackComposerDocument::plain_text(text))
    }

    pub(crate) fn snapshot_slack_send_draft_document(&self) -> SlackComposerDocument {
        let mut document = self.slack_composer_document.borrow_mut();
        document.reset_to_text_if_changed(&self.slack_composer_text);
        document.clone()
    }

    pub(crate) fn snapshot_slack_send_draft(&self) -> SlackComposerDraft {
        SlackComposerDraft {
            id: self.slack_composer_ids.active_draft_id(),
            token: self.slack_send_draft_token,
            client_message_id: self.slack_send_client_message_id.clone(),
            document: self.snapshot_slack_send_draft_document(),
            files: self.slack_composer_files.clone(),
            broadcast: false,
        }
    }

    pub(crate) fn take_slack_send_draft(&mut self) -> SlackComposerDraft {
        let replacement_id = self.next_slack_composer_draft_id();
        let composer_text = std::mem::take(&mut self.slack_composer_text);
        let document = {
            let document = self.slack_composer_document.get_mut();
            document.reset_to_text_if_changed(&composer_text);
            std::mem::take(document)
        };
        self.slack_composer_text = composer_text;
        let draft = SlackComposerDraft {
            id: self.slack_composer_ids.active_draft_id(),
            token: self.slack_send_draft_token,
            client_message_id: self.slack_send_client_message_id.take(),
            document,
            files: std::mem::take(&mut self.slack_composer_files),
            broadcast: false,
        };
        self.slack_composer_ids.set_active_draft_id(replacement_id);
        draft
    }

    pub(crate) fn restore_slack_send_draft(&mut self, draft: Option<SlackComposerDraft>) {
        let token_generation = self.slack_send_draft_token_generation;
        match draft {
            Some(draft) => {
                assert!(
                    self.slack_composer_files.is_empty()
                        || self
                            .slack_composer_files
                            .same_identity_and_order(&draft.files),
                    "restoring a Slack send draft cannot overwrite a different nonempty file collection"
                );
                let SlackComposerDraft {
                    id,
                    token,
                    client_message_id,
                    document,
                    files,
                    broadcast,
                } = draft;
                assert!(
                    !broadcast,
                    "conversation composer cannot restore a thread broadcast draft"
                );
                let files_changed = !self.slack_composer_files.same_identity_and_order(&files);
                self.replace_slack_send_draft_document(document);
                if files_changed {
                    self.advance_slack_send_draft_revision();
                }
                self.slack_composer_ids.set_active_draft_id(id);
                self.slack_composer_files = files;
                self.slack_send_draft_token_generation =
                    self.slack_send_draft_token_generation.max(token);
                self.slack_send_draft_token = token;
                self.slack_send_client_message_id = client_message_id;
            }
            None => {
                assert!(
                    self.slack_composer_files.is_empty(),
                    "clearing a Slack send draft requires its files to be moved or discarded first"
                );
                self.replace_slack_send_draft_document(SlackComposerDocument::default());
                self.slack_composer_ids.replace_active_draft_id();
                self.slack_send_client_message_id = None;
                if self.slack_send_draft_token_generation == token_generation {
                    self.advance_slack_send_draft_revision();
                }
            }
        }
    }

    pub(crate) fn current_slack_main_composer_draft_handle(
        &self,
    ) -> Option<SlackMainComposerDraftHandle> {
        let owner = self
            .slack_active_main_composer_context
            .as_ref()?
            .owner
            .clone();
        Some(SlackMainComposerDraftHandle {
            owner,
            draft_id: self.slack_composer_ids.active_draft_id(),
        })
    }

    pub(in crate::ui::surface::state) fn clear_matching_slack_stored_send_draft(
        &mut self,
        request: &SlackSendRequest,
    ) {
        let (accepted_handle, accepted_files) = self
            .slack_outbound_delivery_ledger
            .iter()
            .find(|delivery| delivery.request == *request)
            .map(|delivery| {
                (
                    delivery.accepted_draft_handle.clone(),
                    delivery.accepted_draft.files.clone(),
                )
            })
            .expect("clearing a stored Slack send draft requires its outbound delivery owner");
        match &request.draft_source {
            SlackSendDraftSource::Conversation { conversation_id }
            | SlackSendDraftSource::Activity {
                conversation_id, ..
            } => self.clear_matching_slack_conversation_stored_send_draft(
                request,
                conversation_id,
                &accepted_handle,
                &accepted_files,
            ),
            SlackSendDraftSource::NewMessage { draft_key } => self
                .clear_matching_slack_new_message_stored_send_draft(
                    request,
                    draft_key,
                    &accepted_handle,
                    &accepted_files,
                ),
        }
    }

    fn clear_matching_slack_conversation_stored_send_draft(
        &mut self,
        request: &SlackSendRequest,
        conversation_id: &str,
        accepted_handle: &SlackMainComposerDraftHandle,
        accepted_files: &SlackComposerFiles,
    ) {
        let SlackMainComposerDraftOwner::Conversation(key) = &accepted_handle.owner else {
            panic!("conversation delivery must retain a conversation draft owner");
        };
        assert!(
            key.team_id == request.team_id
                && key.self_user_id == request.self_user_id
                && matches!(
                    &key.destination,
                    SlackComposerDestination::Conversation {
                        conversation_id: owner_conversation_id
                    } if owner_conversation_id == conversation_id
                        && owner_conversation_id == &request.conversation_id
                ),
            "stored Slack conversation draft cleanup must use the accepted exact owner"
        );
        let Some(draft) = self.slack_composer_drafts.get(key) else {
            return;
        };
        if draft.token != request.draft_token
            || draft.client_message_id.as_ref() != Some(&request.client_message_id)
            || draft.document.text() != request.draft_text
        {
            return;
        }
        assert_eq!(
            &accepted_handle.owner,
            &SlackMainComposerDraftOwner::Conversation(key.clone()),
            "stored Slack conversation draft must transfer to the exact outbound owner"
        );
        assert!(
            draft.id == accepted_handle.draft_id
                && draft.files.same_identity_and_order(accepted_files),
            "stored Slack conversation draft cannot be dropped unless the outbound delivery owns its exact files"
        );
        self.slack_composer_drafts.remove(key);
    }

    fn clear_matching_slack_new_message_stored_send_draft(
        &mut self,
        request: &SlackSendRequest,
        draft_key: &str,
        accepted_handle: &SlackMainComposerDraftHandle,
        accepted_files: &SlackComposerFiles,
    ) {
        let key = SlackNewMessageDraftKey {
            team_id: request.team_id.clone(),
            self_user_id: request.self_user_id.clone(),
            draft_key: draft_key.to_string(),
        };
        let Some(draft) = self.slack_new_message_drafts.get(&key) else {
            return;
        };
        if draft.token != request.draft_token
            || draft.client_message_id.as_ref() != Some(&request.client_message_id)
            || draft.document.text() != request.draft_text
        {
            return;
        }
        assert_eq!(
            accepted_handle.owner,
            SlackMainComposerDraftOwner::NewMessage(key.clone()),
            "stored Slack new-message draft must transfer to the exact outbound owner"
        );
        assert!(
            draft.id == accepted_handle.draft_id
                && draft.files.same_identity_and_order(accepted_files),
            "stored Slack new-message draft cannot be dropped unless the outbound delivery owns its exact files"
        );
        self.slack_new_message_drafts.remove(&key);
    }
}
