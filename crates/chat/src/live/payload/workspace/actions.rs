mod drafts;
mod quick_search;

use std::collections::HashMap;

use crate::model::{
    SlackFileShareReceipt, SlackFileShareRequest, SlackFileShareTargetView, SlackFileUploadReceipt,
    SlackFileUploadTarget, SlackReactionConversationScope, SlackReactionMutationReceipt,
    SlackReactionTarget,
};

use super::{
    enrich_slack_search_snapshot, ensure_reaction_mutation_reflected,
    ensure_thread_reaction_mutation_reflected, load_slack_search_users_with_cache, ops,
    slack_message_send_receipt_from_payload, slack_search_user_ids, SlackAttachmentPreview,
    SlackConversationSnapshot, SlackLiveWorkspaceLoader, SlackMessageDraft,
    SlackMessageForwardReceipt, SlackMessageSendReceipt, SlackMessageSendReceiptPayloads,
    SlackMessageTimestamp, SlackProfile, SlackReactionMutation, SlackReactionName,
    SlackSavedMessageMutation, SlackSearchSnapshot, SlackThreadSnapshot, SlackUploadFile,
};

impl SlackLiveWorkspaceLoader {
    pub fn create_sidebar_section(
        &self,
        request: &crate::model::SlackSidebarSectionCreateRequest,
    ) -> Result<(), String> {
        self.sidebar_api.create_channel_section(request)?;
        self.invalidate_realtime_sidebar_caches()
    }

    pub fn mutate_self_status(
        &self,
        status: &crate::model::SlackSelfStatus,
    ) -> Result<crate::model::SlackSelfStatus, String> {
        self.api.mutate_self_status(status)
    }

    pub fn mutate_self_presence(
        &self,
        presence: crate::model::SlackUserPresence,
    ) -> Result<crate::model::SlackUserPresence, String> {
        self.api.mutate_self_presence(presence)
    }

    pub fn post_message(
        &self,
        conversation_id: &str,
        client_message_id: &crate::model::SlackMessageClientId,
        draft: &SlackMessageDraft,
    ) -> Result<SlackMessageSendReceipt, String> {
        let (self_user_id, self_user) = self.load_self_user()?;
        let users = HashMap::from([(self_user_id.clone(), self_user)]);
        let response = ops::post_message(&self.api, conversation_id, client_message_id, draft)?;
        slack_message_send_receipt_from_payload(SlackMessageSendReceiptPayloads {
            team_id: &self.team_id,
            conversation_id,
            client_message_id,
            response: &response,
            users: &users,
            self_user_id: &self_user_id,
            self_timezone_id: self.timezone.id.as_deref(),
            timezone: self.timezone.value,
        })
    }

    pub fn forward_message(
        &self,
        source_conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
        destination_conversation_id: &str,
        note: Option<&SlackMessageDraft>,
    ) -> Result<SlackMessageForwardReceipt, String> {
        ops::forward_message(
            &self.api,
            source_conversation_id,
            message_timestamp,
            destination_conversation_id,
            note,
        )
    }

    pub fn message_permalink(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
    ) -> Result<String, String> {
        ops::message_permalink(&self.api, conversation_id, message_timestamp)
    }

    pub fn update_message(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
        draft: &SlackMessageDraft,
    ) -> Result<SlackConversationSnapshot, String> {
        self.mutate_and_refresh_conversation(conversation_id, "update_message", || {
            ops::update_message(&self.api, conversation_id, message_timestamp, draft)
        })
    }

    pub fn delete_message(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
    ) -> Result<SlackConversationSnapshot, String> {
        self.mutate_and_refresh_conversation(conversation_id, "delete_message", || {
            ops::delete_message(&self.api, conversation_id, message_timestamp)
        })
    }

    pub fn mutate_saved_message(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
        mutation: SlackSavedMessageMutation,
    ) -> Result<SlackConversationSnapshot, String> {
        self.mutate_and_refresh_conversation(conversation_id, "mutate_saved_message", || {
            ops::mutate_saved_message(&self.api, conversation_id, message_timestamp, mutation)
        })
    }

    pub fn mutate_reaction(
        &self,
        target: &SlackReactionTarget,
        reaction_name: &SlackReactionName,
        mutation: SlackReactionMutation,
    ) -> Result<SlackReactionMutationReceipt<SlackConversationSnapshot, SlackThreadSnapshot>, String>
    {
        ops::mutate_reaction(&self.api, target, reaction_name, mutation)?;
        self.invalidate_reaction_catalog();
        let receipt = match target {
            SlackReactionTarget::ConversationMessage {
                conversation_id, ..
            } => {
                let conversation = self.refresh_conversation(conversation_id)?;
                let (conversation, scope) = if conversation
                    .messages
                    .iter()
                    .any(|message| message.id == target.message_timestamp().as_str())
                {
                    (conversation, SlackReactionConversationScope::Latest)
                } else {
                    (
                        self.load_conversation_with_anchor(
                            conversation_id,
                            Some(target.message_timestamp().as_str()),
                        )?,
                        SlackReactionConversationScope::TargetedWindow,
                    )
                };
                ensure_reaction_mutation_reflected(&conversation, target, reaction_name, mutation)?;
                SlackReactionMutationReceipt::Conversation {
                    snapshot: conversation,
                    scope,
                }
            }
            SlackReactionTarget::ThreadReply {
                conversation_id,
                thread_timestamp,
                message_timestamp,
            } => {
                let thread = self.load_thread_containing_reply(
                    conversation_id,
                    thread_timestamp,
                    message_timestamp,
                )?;
                ensure_thread_reaction_mutation_reflected(
                    &thread,
                    target,
                    reaction_name,
                    mutation,
                )?;
                SlackReactionMutationReceipt::Thread(thread)
            }
        };
        Ok(receipt)
    }

    pub fn upload_files(
        &self,
        target: &SlackFileUploadTarget,
        text: &str,
        files: Vec<SlackUploadFile>,
    ) -> Result<SlackFileUploadReceipt<SlackConversationSnapshot, SlackThreadSnapshot>, String>
    {
        match target.view() {
            crate::model::SlackFileUploadTargetView::Conversation { conversation_id } => self
                .mutate_and_refresh_conversation(conversation_id, "upload_files", || {
                    ops::upload_files(&self.api, target, text, files)
                })
                .map(SlackFileUploadReceipt::Conversation),
            crate::model::SlackFileUploadTargetView::Thread {
                conversation_id,
                thread_timestamp,
            } => {
                ops::upload_files(&self.api, target, text, files)?;
                self.load_thread(conversation_id, thread_timestamp, None)
                    .map(SlackFileUploadReceipt::Thread)
            }
        }
    }

    pub(crate) fn share_files(
        &self,
        request: &SlackFileShareRequest<SlackMessageDraft>,
    ) -> SlackFileShareMutationOutcome {
        match request.target().view() {
            SlackFileShareTargetView::Conversation { conversation_id } => {
                self.share_files_to_conversation(request, conversation_id)
            }
            SlackFileShareTargetView::Thread {
                conversation_id,
                thread_timestamp,
                ..
            } => self.share_files_to_thread(request, conversation_id, thread_timestamp),
        }
    }

    fn share_files_to_conversation(
        &self,
        request: &SlackFileShareRequest<SlackMessageDraft>,
        conversation_id: &str,
    ) -> SlackFileShareMutationOutcome {
        match ops::share_files(&self.api, request) {
            ops::SlackFileSharePostOutcome::Accepted => {
                match self
                    .refresh_conversation(conversation_id)
                    .and_then(|conversation| {
                        ensure_file_share_conversation_identity(
                            &conversation,
                            &self.team_id,
                            conversation_id,
                        )?;
                        Ok(conversation)
                    }) {
                    Ok(conversation) => SlackFileShareMutationOutcome::Confirmed(Box::new(
                        SlackFileShareReceipt::Conversation(conversation),
                    )),
                    Err(diagnostic) => SlackFileShareMutationOutcome::Unknown { diagnostic },
                }
            }
            ops::SlackFileSharePostOutcome::NotSent { diagnostic } => {
                SlackFileShareMutationOutcome::NotSent { diagnostic }
            }
            ops::SlackFileSharePostOutcome::Rejected { diagnostic } => {
                SlackFileShareMutationOutcome::Rejected { diagnostic }
            }
            ops::SlackFileSharePostOutcome::Unknown { diagnostic } => {
                SlackFileShareMutationOutcome::Unknown { diagnostic }
            }
        }
    }

    fn share_files_to_thread(
        &self,
        request: &SlackFileShareRequest<SlackMessageDraft>,
        conversation_id: &str,
        thread_timestamp: &SlackMessageTimestamp,
    ) -> SlackFileShareMutationOutcome {
        match ops::share_files(&self.api, request) {
            ops::SlackFileSharePostOutcome::Accepted => {
                match self
                    .load_thread(conversation_id, thread_timestamp, None)
                    .and_then(|thread| {
                        ensure_file_share_thread_identity(
                            &thread,
                            &self.team_id,
                            conversation_id,
                            thread_timestamp,
                        )?;
                        Ok(thread)
                    }) {
                    Ok(thread) => SlackFileShareMutationOutcome::Confirmed(Box::new(
                        SlackFileShareReceipt::Thread(thread),
                    )),
                    Err(diagnostic) => SlackFileShareMutationOutcome::Unknown { diagnostic },
                }
            }
            ops::SlackFileSharePostOutcome::NotSent { diagnostic } => {
                SlackFileShareMutationOutcome::NotSent { diagnostic }
            }
            ops::SlackFileSharePostOutcome::Rejected { diagnostic } => {
                SlackFileShareMutationOutcome::Rejected { diagnostic }
            }
            ops::SlackFileSharePostOutcome::Unknown { diagnostic } => {
                SlackFileShareMutationOutcome::Unknown { diagnostic }
            }
        }
    }

    pub fn load_profile(&self, user_id: &str) -> Result<SlackProfile, String> {
        ops::load_profile(&self.api, &self.user_cache, user_id)
    }

    pub fn search_messages(
        &self,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<SlackSearchSnapshot, String> {
        self.search_messages_with_options(&crate::model::SlackMessageSearchRequest {
            query: query.to_string(),
            cursor: cursor.map(ToString::to_string),
            options: crate::model::SlackMessageSearchOptions::default(),
        })
    }

    pub fn search_messages_with_options(
        &self,
        request: &crate::model::SlackMessageSearchRequest,
    ) -> Result<SlackSearchSnapshot, String> {
        let self_user_id = self.load_self_user_id()?;
        let snapshot = self
            .api
            .search_messages(request, &self.team_id, &self_user_id)?;
        let users = load_slack_search_users_with_cache(
            &self.api,
            slack_search_user_ids(&snapshot),
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let sidebar = self.load_sidebar_snapshot()?;
        Ok(enrich_slack_search_snapshot(snapshot, &users, &sidebar))
    }

    pub fn load_attachment_preview(
        &self,
        url: &str,
    ) -> Result<Option<SlackAttachmentPreview>, String> {
        self.load_attachment_preview_with_timeout(url, std::time::Duration::from_secs(15))
    }

    pub fn load_attachment_preview_with_timeout(
        &self,
        url: &str,
        timeout: std::time::Duration,
    ) -> Result<Option<SlackAttachmentPreview>, String> {
        ops::load_attachment_preview(&self.api, &self.attachment_preview_cache, url, timeout)
    }
}

pub(crate) enum SlackFileShareMutationOutcome {
    Confirmed(Box<SlackFileShareReceipt<SlackConversationSnapshot, SlackThreadSnapshot>>),
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

fn ensure_file_share_conversation_identity(
    conversation: &SlackConversationSnapshot,
    team_id: &str,
    conversation_id: &str,
) -> Result<(), String> {
    if conversation.team_id != team_id || conversation.conversation_id != conversation_id {
        return Err("Slack files.share refresh returned a mismatched conversation".to_string());
    }
    Ok(())
}

fn ensure_file_share_thread_identity(
    thread: &SlackThreadSnapshot,
    team_id: &str,
    conversation_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
) -> Result<(), String> {
    if thread.team_id != team_id
        || thread.conversation_id != conversation_id
        || thread.thread_timestamp != thread_timestamp.as_str()
    {
        return Err("Slack files.share refresh returned a mismatched thread".to_string());
    }
    Ok(())
}
