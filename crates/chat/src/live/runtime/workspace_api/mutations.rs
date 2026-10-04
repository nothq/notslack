use crate::model::{
    SlackConversationSnapshot, SlackMessageDraft, SlackThreadSnapshot, SlackUploadFile,
};

use crate::live::payload::SlackFileShareMutationOutcome;

use super::super::merge_slack_conversation_windows;
use super::SlackWorkspaceRuntime;

impl SlackWorkspaceRuntime {
    pub(super) fn update_and_commit_message(
        &self,
        conversation_id: &str,
        message_timestamp: &crate::model::SlackMessageTimestamp,
        draft: &SlackMessageDraft,
    ) -> Result<SlackConversationSnapshot, String> {
        let conversation = self.reconcile_remote_conversation(self.loader.update_message(
            conversation_id,
            message_timestamp,
            draft,
        )?)?;
        self.commit_conversation(&conversation)?;
        Ok(conversation)
    }

    pub(super) fn delete_and_commit_message(
        &self,
        conversation_id: &str,
        message_timestamp: &crate::model::SlackMessageTimestamp,
    ) -> Result<SlackConversationSnapshot, String> {
        self.confirmed_send_ledger(conversation_id)?;
        let conversation = self
            .loader
            .delete_message(conversation_id, message_timestamp)?;
        self.forget_confirmed_send(conversation_id, message_timestamp)?;
        let conversation = self.reconcile_remote_conversation(conversation)?;
        self.commit_conversation(&conversation)?;
        Ok(conversation)
    }

    pub(super) fn mutate_and_commit_saved_state(
        &self,
        conversation_id: &str,
        message_timestamp: &crate::model::SlackMessageTimestamp,
        mutation: crate::model::SlackSavedMessageMutation,
    ) -> Result<SlackConversationSnapshot, String> {
        let latest =
            self.loader
                .mutate_saved_message(conversation_id, message_timestamp, mutation)?;
        let conversation = if latest
            .messages
            .iter()
            .any(|message| message.id == message_timestamp.as_str())
        {
            latest
        } else {
            let anchored = self
                .loader
                .load_conversation_with_anchor(conversation_id, Some(message_timestamp.as_str()))?;
            merge_slack_conversation_windows(latest, anchored)?
        };
        let conversation = self.reconcile_remote_conversation(conversation)?;
        let expected_saved_state = mutation
            .active_after()
            .then_some(crate::model::SlackLaterState::InProgress);
        if conversation
            .messages
            .iter()
            .find(|message| message.id == message_timestamp.as_str())
            .is_none_or(|message| message.saved_state != expected_saved_state)
        {
            return Err(
                "Slack saved-message refresh returned mismatched authoritative state".to_string(),
            );
        }
        self.commit_conversation(&conversation)?;
        Ok(conversation)
    }

    pub(super) fn mutate_and_commit_reaction(
        &self,
        target: &crate::model::SlackReactionTarget,
        reaction_name: &crate::model::SlackReactionName,
        mutation: crate::model::SlackReactionMutation,
    ) -> Result<
        crate::model::SlackReactionMutationReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
        String,
    > {
        match self
            .loader
            .mutate_reaction(target, reaction_name, mutation)?
        {
            crate::model::SlackReactionMutationReceipt::Conversation {
                snapshot: conversation,
                scope,
            } => {
                let conversation = match scope {
                    crate::model::SlackReactionConversationScope::Latest => conversation,
                    crate::model::SlackReactionConversationScope::TargetedWindow => {
                        let latest = self.loader.refresh_conversation(target.conversation_id())?;
                        merge_slack_conversation_windows(latest, conversation)?
                    }
                };
                let conversation = self.reconcile_remote_conversation(conversation)?;
                self.commit_conversation(&conversation)?;
                Ok(crate::model::SlackReactionMutationReceipt::Conversation {
                    snapshot: conversation,
                    scope: crate::model::SlackReactionConversationScope::Latest,
                })
            }
            crate::model::SlackReactionMutationReceipt::Thread(thread) => {
                Ok(crate::model::SlackReactionMutationReceipt::Thread(thread))
            }
        }
    }

    pub(super) fn upload_and_commit_files(
        &self,
        target: &crate::model::SlackFileUploadTarget,
        text: &str,
        files: Vec<SlackUploadFile>,
    ) -> Result<
        crate::model::SlackFileUploadReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
        String,
    > {
        match self.loader.upload_files(target, text, files)? {
            crate::model::SlackFileUploadReceipt::Conversation(conversation) => {
                let conversation = self.reconcile_remote_conversation(conversation)?;
                self.commit_conversation(&conversation)?;
                Ok(crate::model::SlackFileUploadReceipt::Conversation(
                    conversation,
                ))
            }
            crate::model::SlackFileUploadReceipt::Thread(thread) => {
                Ok(crate::model::SlackFileUploadReceipt::Thread(thread))
            }
        }
    }

    pub(super) fn share_and_commit_files(
        &self,
        request: &crate::model::SlackFileShareRequest<crate::model::SlackMessageDraft>,
    ) -> Result<
        crate::model::SlackFileShareReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
        String,
    > {
        let reservation = self
            .loader
            .reserve_staged_file_share(request.file_ids())
            .map_err(|failure| failure.to_string())?;
        match self.loader.share_files(request) {
            SlackFileShareMutationOutcome::NotSent { diagnostic }
            | SlackFileShareMutationOutcome::Rejected { diagnostic } => {
                self.loader
                    .abort_staged_file_share(reservation)
                    .map_err(|failure| {
                        format!(
                            "{diagnostic}; failed to release staged-file share reservation: {failure}"
                        )
                    })?;
                Err(diagnostic)
            }
            SlackFileShareMutationOutcome::Unknown { diagnostic } => {
                self.loader
                    .protect_staged_file_share_unknown(reservation, diagnostic.clone())
                    .map_err(|failure| {
                        format!(
                            "{diagnostic}; failed to protect ambiguous staged-file share: {failure}"
                        )
                    })?;
                Err(diagnostic)
            }
            SlackFileShareMutationOutcome::Confirmed(receipt) => {
                let committed = self.commit_file_share_receipt(*receipt);
                match committed {
                    Ok(receipt) => {
                        self.loader
                            .confirm_staged_file_share(reservation)
                            .map_err(|failure| failure.to_string())?;
                        Ok(receipt)
                    }
                    Err(diagnostic) => {
                        self.loader
                            .protect_staged_file_share_unknown(
                                reservation,
                                diagnostic.clone(),
                            )
                            .map_err(|failure| {
                                format!(
                                    "{diagnostic}; failed to protect remotely accepted staged-file share: {failure}"
                                )
                            })?;
                        Err(diagnostic)
                    }
                }
            }
        }
    }

    fn commit_file_share_receipt(
        &self,
        receipt: crate::model::SlackFileShareReceipt<
            SlackConversationSnapshot,
            SlackThreadSnapshot,
        >,
    ) -> Result<
        crate::model::SlackFileShareReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
        String,
    > {
        match receipt {
            crate::model::SlackFileShareReceipt::Conversation(conversation) => self
                .reconcile_remote_conversation(conversation)
                .and_then(|conversation| {
                    self.commit_conversation(&conversation)?;
                    Ok(crate::model::SlackFileShareReceipt::Conversation(
                        conversation,
                    ))
                }),
            crate::model::SlackFileShareReceipt::Thread(thread) => {
                Ok(crate::model::SlackFileShareReceipt::Thread(thread))
            }
        }
    }
}
