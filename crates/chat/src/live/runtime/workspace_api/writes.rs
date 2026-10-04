macro_rules! impl_workspace_api_writes {
    () => {
        fn send_slack_message(
            &self,
            conversation_id: &str,
            client_message_id: &crate::model::SlackMessageClientId,
            draft: &SlackMessageDraft,
        ) -> Result<SlackMessageSendReceipt, String> {
            let confirmed_sends = self.confirmed_send_ledger(conversation_id)?;
            let receipt = self
                .loader
                .post_message(conversation_id, client_message_id, draft)?;
            self.record_confirmed_send(&confirmed_sends, &receipt);
            Ok(receipt)
        }

        fn create_slack_scheduled_draft(
            &self,
            target: crate::model::SlackScheduledDraftCreateTarget<'_>,
            content: crate::model::SlackDraftContent<'_, SlackMessageDraft>,
            file_ids: &[crate::model::SlackFileId],
        ) -> Result<SlackScheduledDraftReceipt, crate::model::SlackScheduledDraftMutationFailure> {
            self.loader
                .create_scheduled_draft(target, content, file_ids)
        }

        fn create_slack_draft(
            &self,
            client_message_id: &crate::model::SlackMessageClientId,
            write_target: &crate::model::SlackDraftWriteTarget,
            content: crate::model::SlackDraftContent<'_, SlackMessageDraft>,
            file_ids: &[crate::model::SlackFileId],
        ) -> Result<crate::model::SlackDraftReceipt, String> {
            self.loader
                .create_draft(client_message_id, write_target, content, file_ids)
        }

        fn update_slack_draft(
            &self,
            update_target: crate::model::SlackDraftUpdateTarget<'_>,
            write_target: &crate::model::SlackDraftWriteTarget,
            content: crate::model::SlackDraftContent<'_, SlackMessageDraft>,
            file_ids: &[crate::model::SlackFileId],
        ) -> Result<crate::model::SlackDraftReceipt, String> {
            self.loader
                .update_draft(update_target, write_target, content, file_ids)
        }

        fn delete_slack_draft(
            &self,
            target: &crate::model::SlackDraftTarget,
            file_deletion: crate::model::SlackDraftFileDeletion,
        ) -> Result<(), String> {
            self.loader.delete_draft(target, file_deletion)
        }

        fn update_slack_scheduled_draft(
            &self,
            target: crate::model::SlackScheduledDraftUpdateTarget<'_>,
            content: crate::model::SlackDraftContent<'_, SlackMessageDraft>,
            file_ids: &[crate::model::SlackFileId],
        ) -> Result<SlackScheduledDraftReceipt, crate::model::SlackScheduledDraftMutationFailure> {
            self.loader
                .update_scheduled_draft(target, content, file_ids)
        }

        fn reconcile_slack_scheduled_draft(
            &self,
            target: crate::model::SlackScheduledDraftReconcileTarget<'_>,
            file_ids: &[crate::model::SlackFileId],
            local_files: &[crate::model::SlackScheduledDraftLocalFile],
        ) -> Result<Option<SlackScheduledDraftReceipt>, String> {
            self.loader
                .reconcile_scheduled_draft(target, file_ids, local_files)
        }

        fn delete_slack_scheduled_draft(
            &self,
            target: &crate::model::SlackDraftTarget,
        ) -> Result<(), String> {
            self.loader.delete_scheduled_draft(target)
        }

        fn forward_slack_message(
            &self,
            source_conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
            destination_conversation_id: &str,
            note: Option<&SlackMessageDraft>,
        ) -> Result<SlackMessageForwardReceipt, String> {
            self.loader.forward_message(
                source_conversation_id,
                message_timestamp,
                destination_conversation_id,
                note,
            )
        }

        fn load_slack_message_permalink(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<String, String> {
            self.loader
                .message_permalink(conversation_id, message_timestamp)
        }

        fn update_slack_message(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
            draft: &SlackMessageDraft,
        ) -> Result<SlackConversationSnapshot, String> {
            self.update_and_commit_message(conversation_id, message_timestamp, draft)
        }

        fn delete_slack_message(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<SlackConversationSnapshot, String> {
            self.delete_and_commit_message(conversation_id, message_timestamp)
        }

        fn mutate_slack_message_saved_state(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
            mutation: crate::model::SlackSavedMessageMutation,
        ) -> Result<SlackConversationSnapshot, String> {
            self.mutate_and_commit_saved_state(conversation_id, message_timestamp, mutation)
        }

        fn mutate_slack_reaction(
            &self,
            target: &crate::model::SlackReactionTarget,
            reaction_name: &crate::model::SlackReactionName,
            mutation: crate::model::SlackReactionMutation,
        ) -> Result<
            crate::model::SlackReactionMutationReceipt<
                SlackConversationSnapshot,
                SlackThreadSnapshot,
            >,
            String,
        > {
            self.mutate_and_commit_reaction(target, reaction_name, mutation)
        }

        fn mutate_slack_channel_star(
            &self,
            conversation_id: &str,
            mutation: crate::model::SlackStarMutation,
        ) -> Result<SlackSidebarSnapshot, String> {
            self.mutate_channel_star(conversation_id, mutation)
        }

        fn load_slack_channel_permalink(&self, conversation_id: &str) -> Result<String, String> {
            self.loader.channel_permalink(conversation_id)
        }

        fn upload_slack_files(
            &self,
            target: &crate::model::SlackFileUploadTarget,
            text: &str,
            files: Vec<SlackUploadFile>,
        ) -> Result<
            crate::model::SlackFileUploadReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
            String,
        > {
            self.upload_and_commit_files(target, text, files)
        }

        fn stage_slack_file(
            &self,
            operation_id: &crate::model::SlackFileStagingOperationId,
            file: SlackUploadFile,
        ) -> crate::model::SlackFileStagingOutcome {
            self.loader.stage_file(operation_id, file)
        }

        fn cancel_slack_file_staging(
            &self,
            operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingCancellationOutcome {
            self.loader.cancel_file_staging(operation_id)
        }

        fn reconcile_slack_file_staging(
            &self,
            operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingReconcileOutcome {
            self.loader.reconcile_file_staging(operation_id)
        }

        fn cleanup_slack_staged_file(
            &self,
            operation_id: &crate::model::SlackFileStagingOperationId,
        ) -> crate::model::SlackFileStagingCleanupOutcome {
            self.loader.cleanup_staged_file(operation_id)
        }

        fn cleanup_slack_remote_draft_file(
            &self,
            reference: &crate::model::SlackRemoteDraftFileReference,
        ) -> crate::model::SlackRemoteDraftFileCleanupOutcome {
            self.loader.cleanup_remote_draft_file(reference)
        }

        fn share_slack_files(
            &self,
            request: &crate::model::SlackFileShareRequest<SlackMessageDraft>,
        ) -> Result<
            crate::model::SlackFileShareReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
            String,
        > {
            self.share_and_commit_files(request)
        }

        fn load_slack_profile(&self, user_id: &str) -> Result<SlackProfile, String> {
            self.loader.load_profile(user_id)
        }

        fn load_slack_remote_image(&self, url: &str) -> Result<Option<RemoteImageData>, String> {
            self.load_remote_image(url)
        }

        fn load_slack_remote_image_with_timeout(
            &self,
            url: &str,
            timeout: std::time::Duration,
        ) -> Result<Option<RemoteImageData>, String> {
            self.load_remote_image_with_timeout(url, timeout)
        }

        fn prepare_slack_attachment_media(
            &self,
            media: &crate::model::SlackAttachmentMedia,
            declared_mimetype: &str,
        ) -> Result<crate::model::SlackPreparedMediaSource, String> {
            self.media_proxy.prepare(media, declared_mimetype)
        }

        fn release_slack_attachment_media(
            &self,
            source: &crate::model::SlackPreparedMediaSource,
        ) -> Result<(), String> {
            self.media_proxy.release(source)
        }
    };
}

pub(super) use impl_workspace_api_writes;
