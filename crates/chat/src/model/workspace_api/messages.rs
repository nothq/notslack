macro_rules! slack_workspace_message_api {
    () => {
        fn send_slack_thread_reply(
            &self,
            target: crate::model::SlackThreadReplyTarget<'_>,
            client_message_id: &crate::model::SlackMessageClientId,
            draft: &crate::model::SlackMessageDraft,
        ) -> Result<crate::model::SlackThreadReplyReceipt, String>;
        fn send_slack_message(
            &self,
            conversation_id: &str,
            client_message_id: &crate::model::SlackMessageClientId,
            draft: &crate::model::SlackMessageDraft,
        ) -> Result<crate::model::SlackMessageSendReceipt, String>;
        fn create_slack_draft(
            &self,
            _client_message_id: &crate::model::SlackMessageClientId,
            _write_target: &crate::model::SlackDraftWriteTarget,
            _content: crate::model::SlackDraftContent<'_, crate::model::SlackMessageDraft>,
            _file_ids: &[crate::model::SlackFileId],
        ) -> Result<crate::model::SlackDraftReceipt, String> {
            Err("Slack draft creation is unavailable for this workspace runtime".to_string())
        }
        fn update_slack_draft(
            &self,
            _update_target: crate::model::SlackDraftUpdateTarget<'_>,
            _write_target: &crate::model::SlackDraftWriteTarget,
            _content: crate::model::SlackDraftContent<'_, crate::model::SlackMessageDraft>,
            _file_ids: &[crate::model::SlackFileId],
        ) -> Result<crate::model::SlackDraftReceipt, String> {
            Err("Slack draft editing is unavailable for this workspace runtime".to_string())
        }
        fn delete_slack_draft(
            &self,
            _target: &crate::model::SlackDraftTarget,
            _file_deletion: crate::model::SlackDraftFileDeletion,
        ) -> Result<(), String> {
            Err("Slack draft deletion is unavailable for this workspace runtime".to_string())
        }
        fn create_slack_scheduled_draft(
            &self,
            target: crate::model::SlackScheduledDraftCreateTarget<'_>,
            content: crate::model::SlackDraftContent<'_, crate::model::SlackMessageDraft>,
            file_ids: &[crate::model::SlackFileId],
        ) -> Result<
            crate::model::SlackScheduledDraftReceipt,
            crate::model::SlackScheduledDraftMutationFailure,
        >;
        fn update_slack_scheduled_draft(
            &self,
            _target: crate::model::SlackScheduledDraftUpdateTarget<'_>,
            _content: crate::model::SlackDraftContent<'_, crate::model::SlackMessageDraft>,
            _file_ids: &[crate::model::SlackFileId],
        ) -> Result<
            crate::model::SlackScheduledDraftReceipt,
            crate::model::SlackScheduledDraftMutationFailure,
        > {
            Err(crate::model::SlackScheduledDraftMutationFailure::NotSent {
                diagnostic:
                    "Slack scheduled-draft editing is unavailable for this workspace runtime"
                        .to_string(),
            })
        }
        fn reconcile_slack_scheduled_draft(
            &self,
            _target: crate::model::SlackScheduledDraftReconcileTarget<'_>,
            _file_ids: &[crate::model::SlackFileId],
            _local_files: &[crate::model::SlackScheduledDraftLocalFile],
        ) -> Result<Option<crate::model::SlackScheduledDraftReceipt>, String> {
            Err(
                "Slack scheduled-draft reconciliation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn delete_slack_scheduled_draft(
            &self,
            _target: &crate::model::SlackDraftTarget,
        ) -> Result<(), String> {
            Err(
                "Slack scheduled-draft cancellation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn forward_slack_message(
            &self,
            _source_conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
            _destination_conversation_id: &str,
            _note: Option<&crate::model::SlackMessageDraft>,
        ) -> Result<crate::model::SlackMessageForwardReceipt, String> {
            Err("Slack message forwarding is unavailable for this workspace runtime".to_string())
        }
        fn mutate_slack_reaction(
            &self,
            _target: &crate::model::SlackReactionTarget,
            _reaction_name: &crate::model::SlackReactionName,
            _mutation: crate::model::SlackReactionMutation,
        ) -> Result<
            crate::model::SlackReactionMutationReceipt<
                crate::model::SlackConversationSnapshot,
                crate::model::SlackThreadSnapshot,
            >,
            String,
        > {
            Err("Slack reaction mutation is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_message_permalink(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<String, String> {
            Err("Slack message permalink is unavailable for this workspace runtime".to_string())
        }
        fn update_slack_message(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
            _draft: &crate::model::SlackMessageDraft,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err("Slack message editing is unavailable for this workspace runtime".to_string())
        }
        fn delete_slack_message(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err("Slack message deletion is unavailable for this workspace runtime".to_string())
        }
        fn mutate_slack_message_saved_state(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
            _mutation: crate::model::SlackSavedMessageMutation,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err(
                "Slack saved-message mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn mutate_slack_channel_star(
            &self,
            _conversation_id: &str,
            _mutation: crate::model::SlackStarMutation,
        ) -> Result<crate::model::SlackSidebarSnapshot, String> {
            Err("Slack channel star mutation is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_channel_permalink(&self, _conversation_id: &str) -> Result<String, String> {
            Err("Slack channel permalink is unavailable for this workspace runtime".to_string())
        }
        fn upload_slack_files(
            &self,
            _target: &crate::model::SlackFileUploadTarget,
            _text: &str,
            _files: Vec<crate::model::SlackUploadFile>,
        ) -> Result<
            crate::model::SlackFileUploadReceipt<
                crate::model::SlackConversationSnapshot,
                crate::model::SlackThreadSnapshot,
            >,
            String,
        > {
            Err("Slack file uploads are unavailable for this workspace runtime".to_string())
        }
        fn stage_slack_file(
            &self,
            _operation_id: &crate::model::SlackFileStagingOperationId,
            _file: crate::model::SlackUploadFile,
        ) -> crate::model::SlackFileStagingOutcome {
            crate::model::SlackFileStagingOutcome::Failed(
                crate::model::SlackFileStagingFailure::Operation(
                    crate::model::file_staging::unavailable_file_staging_operation("upload"),
                ),
            )
        }
        fn cancel_slack_file_staging(
            &self,
            _operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingCancellationOutcome {
            crate::model::SlackFileStagingCancellationOutcome::Failed(
                crate::model::file_staging::unavailable_file_staging_operation("cancellation"),
            )
        }
        fn reconcile_slack_file_staging(
            &self,
            _operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingReconcileOutcome {
            crate::model::SlackFileStagingReconcileOutcome::Failed(
                crate::model::file_staging::unavailable_file_staging_operation("reconciliation"),
            )
        }
        fn cleanup_slack_staged_file(
            &self,
            _operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingCleanupOutcome {
            crate::model::SlackFileStagingCleanupOutcome::Failed(
                crate::model::file_staging::unavailable_file_staging_operation("cleanup"),
            )
        }
        fn cleanup_slack_remote_draft_file(
            &self,
            _reference: &crate::model::SlackRemoteDraftFileReference,
        ) -> crate::model::SlackRemoteDraftFileCleanupOutcome {
            crate::model::SlackRemoteDraftFileCleanupOutcome::Failed {
                diagnostic:
                    "Slack remote draft file cleanup is unavailable for this workspace runtime"
                        .to_string(),
            }
        }
        fn share_slack_files(
            &self,
            _request: &crate::model::SlackFileShareRequest<crate::model::SlackMessageDraft>,
        ) -> Result<
            crate::model::SlackFileShareReceipt<
                crate::model::SlackConversationSnapshot,
                crate::model::SlackThreadSnapshot,
            >,
            String,
        > {
            Err("Slack staged file sharing is unavailable for this workspace runtime".to_string())
        }
    };
}

pub(super) use slack_workspace_message_api;
