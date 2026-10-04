mod conversation;
mod workspace;

macro_rules! impl_workspace_api_reads {
    () => {
        fn mutate_slack_self_status(
            &self,
            status: &crate::model::SlackSelfStatus,
        ) -> Result<crate::model::SlackSelfStatus, String> {
            self.loader.mutate_self_status(status)
        }

        fn mutate_slack_self_presence(
            &self,
            presence: crate::model::SlackUserPresence,
        ) -> Result<crate::model::SlackUserPresence, String> {
            self.loader.mutate_self_presence(presence)
        }

        fn capabilities(&self) -> crate::model::SlackWorkspaceApiCapabilities {
            crate::model::SlackWorkspaceApiCapabilities::LIVE
        }

        fn subscribe_slack_realtime(
            &self,
        ) -> Result<Box<dyn crate::model::SlackRealtimeSubscription>, String> {
            Ok(Box::new(
                crate::live::events::SlackRealtimeSubscription::new_presentation(
                    self.presentation_realtime.subscribe(),
                    self.presentation_presence_snapshot.subscribe(),
                    self.presentation_realtime_closed
                        .load(std::sync::atomic::Ordering::Acquire),
                ),
            ))
        }

        fn load_slack_shell(&self) -> Result<SlackShellSnapshot, String> {
            self.load_and_cache_shell()
        }

        fn load_slack_sidebar(
            &self,
            conversation_id: &str,
        ) -> Result<SlackSidebarSnapshot, String> {
            self.load_and_cache_sidebar(conversation_id)
        }

        fn create_slack_sidebar_section(
            &self,
            request: &crate::model::SlackSidebarSectionCreateRequest,
            conversation_id: &str,
        ) -> Result<SlackSidebarSnapshot, String> {
            self.loader.create_sidebar_section(request)?;
            self.load_and_cache_sidebar(conversation_id)
        }

        fn refresh_slack_sidebar_from_realtime(
            &self,
            conversation_id: &str,
        ) -> Result<SlackSidebarSnapshot, String> {
            self.refresh_sidebar_from_realtime(conversation_id)
        }

        fn load_slack_dm_inbox(
            &self,
            cursor: Option<&str>,
        ) -> Result<SlackDmInboxSnapshot, String> {
            self.load_and_cache_dm_inbox(cursor)
        }

        fn load_cached_slack_dm_inbox(&self) -> Result<Option<SlackDmInboxSnapshot>, String> {
            self.load_cached_dm_inbox_with_presence()
        }

        fn load_slack_all_threads(
            &self,
            cursor: Option<&SlackAllThreadsCursor>,
        ) -> Result<SlackAllThreadsSnapshot, String> {
            let mut snapshot = self.loader.load_all_threads(cursor)?;
            self.overlay_all_threads_presence(&mut snapshot);
            Ok(snapshot)
        }

        fn load_slack_activity(
            &self,
            cursor: Option<&SlackActivityCursor>,
        ) -> Result<SlackActivitySnapshot, String> {
            self.loader.load_activity(cursor)
        }

        fn mark_slack_activity_item_read(
            &self,
            target: &crate::model::SlackActivityReadTarget,
        ) -> Result<(), String> {
            self.loader.mark_activity_item_read(target)
        }

        fn mark_slack_activity_item_unread(
            &self,
            target: &crate::model::SlackActivityReadTarget,
        ) -> Result<(), String> {
            self.loader.mark_activity_item_unread(target)
        }

        fn archive_slack_activity_item(
            &self,
            target: &crate::model::SlackActivityArchiveTarget,
            reason: &str,
        ) -> Result<(), String> {
            self.loader.archive_activity_item(target, reason)
        }

        fn unarchive_slack_activity_item(
            &self,
            target: &crate::model::SlackActivityArchiveTarget,
            reason: &str,
        ) -> Result<(), String> {
            self.loader.unarchive_activity_item(target, reason)
        }

        fn load_slack_later(
            &self,
            filter: SlackLaterFilter,
            cursor: Option<&SlackLaterCursor>,
        ) -> Result<SlackLaterSnapshot, String> {
            self.loader.load_later(filter, cursor)
        }

        fn hydrate_slack_later_item(
            &self,
            target: SlackLaterHydrationTarget,
        ) -> Result<SlackLaterHydratedItem, String> {
            self.loader.hydrate_later_item(target)
        }

        fn mutate_slack_reminder(
            &self,
            mutation: &crate::model::SlackReminderMutation,
        ) -> Result<(), String> {
            self.loader.mutate_later_reminder(mutation)
        }

        fn load_slack_files(
            &self,
            request: SlackFilesRequest,
        ) -> Result<SlackFilesSnapshot, String> {
            self.loader.load_files(request)
        }

        fn load_slack_file_metadata(
            &self,
            request: &crate::model::SlackFileMetadataRequest,
        ) -> Result<crate::model::SlackFileMetadataBatch, String> {
            self.loader.load_file_metadata(request)
        }

        fn load_slack_conversation_files(
            &self,
            request: SlackConversationFilesRequest,
        ) -> Result<SlackConversationFilesSnapshot, String> {
            self.loader.load_conversation_files(request)
        }

        fn load_slack_drafts_sent(
            &self,
            request: SlackDraftsSentRequest,
        ) -> Result<SlackDraftsSentSnapshot, String> {
            self.loader.load_drafts_sent(request)
        }

        fn load_slack_pins(&self, request: SlackPinsRequest) -> Result<SlackPinsSnapshot, String> {
            self.loader.load_pins(request)
        }

        fn load_slack_bookmark_folder(
            &self,
            request: SlackBookmarkFolderRequest,
        ) -> Result<SlackBookmarkFolderSnapshot, String> {
            self.loader.load_bookmark_folder(request)
        }

        fn load_slack_conversation_members(
            &self,
            conversation_id: &str,
            cursor: Option<&SlackConversationMembersCursor>,
        ) -> Result<SlackConversationMembersSnapshot, String> {
            self.load_slack_conversation_members_with_presence(conversation_id, cursor)
        }

        fn load_slack_channel_details(
            &self,
            conversation_id: &str,
        ) -> Result<SlackChannelDetailsSnapshot, String> {
            self.loader.load_channel_details(conversation_id)
        }

        fn load_slack_channel_notification_preference(
            &self,
            conversation_id: &str,
        ) -> Result<SlackChannelNotificationPreference, String> {
            self.load_and_cache_channel_notification_preference(conversation_id)
        }

        fn mutate_slack_channel_notification_preference(
            &self,
            conversation_id: &str,
            mutation: SlackChannelNotificationMutation,
        ) -> Result<SlackChannelNotificationPreference, String> {
            self.mutate_and_cache_channel_notification_preference(conversation_id, mutation)
        }

        fn load_slack_preferred_skin_tone(
            &self,
        ) -> Result<crate::model::SlackPreferredSkinTone, String> {
            self.loader.load_preferred_skin_tone()
        }

        fn mutate_slack_preferred_skin_tone(
            &self,
            selection: crate::model::SlackSkinTone,
        ) -> Result<crate::model::SlackPreferredSkinTone, String> {
            self.loader.mutate_preferred_skin_tone(selection)
        }

        fn load_slack_destination_directory(
            &self,
        ) -> Result<crate::model::SlackDestinationDirectorySnapshot, String> {
            self.load_slack_destination_directory_with_presence()
        }

        fn load_slack_reaction_catalog(
            &self,
        ) -> Result<std::sync::Arc<SlackReactionCatalogSnapshot>, String> {
            self.loader.load_reaction_catalog()
        }

        fn open_slack_conversation(
            &self,
            request: SlackConversationOpenRequest,
        ) -> Result<SlackConversationOpenReceipt, String> {
            self.loader.open_conversation(request)
        }

        fn load_slack_conversation(
            &self,
            conversation_id: &str,
        ) -> Result<SlackConversationSnapshot, String> {
            self.load_and_remember_conversation(conversation_id, None)
        }

        fn refresh_slack_conversation(
            &self,
            conversation_id: &str,
        ) -> Result<SlackConversationSnapshot, String> {
            self.refresh_conversation(conversation_id)
        }

        fn mark_slack_conversation_read(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<SlackConversationReadReceipt, String> {
            self.mark_slack_conversation_read_and_publish(conversation_id, message_timestamp)
        }

        fn load_slack_conversation_at(
            &self,
            conversation_id: &str,
            message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<SlackConversationSnapshot, String> {
            self.load_and_remember_conversation(conversation_id, Some(message_timestamp.as_str()))
        }

        fn load_slack_conversation_beginning(
            &self,
            conversation_id: &str,
        ) -> Result<SlackConversationSnapshot, String> {
            let conversation = beginning::load(&self.loader, conversation_id)?;
            self.remember_conversation_metadata(&conversation)?;
            Ok(conversation)
        }

        fn load_slack_conversation_history_page(
            &self,
            conversation_id: &str,
            cursor: &SlackConversationHistoryCursor,
        ) -> Result<SlackConversationHistoryPage, String> {
            self.loader
                .load_conversation_history_page(conversation_id, cursor)
        }

        fn load_slack_thread(
            &self,
            conversation_id: &str,
            thread_timestamp: &crate::model::SlackMessageTimestamp,
            cursor: Option<&str>,
        ) -> Result<SlackThreadSnapshot, String> {
            self.loader
                .load_thread(conversation_id, thread_timestamp, cursor)
        }

        fn load_slack_thread_with_read_metadata(
            &self,
            conversation_id: &str,
            thread_timestamp: &crate::model::SlackMessageTimestamp,
            cursor: Option<&str>,
        ) -> Result<SlackThreadLoad, String> {
            self.loader
                .load_thread_with_read_metadata(conversation_id, thread_timestamp, cursor)
        }

        fn mark_slack_thread_read(&self, target: &SlackThreadReadTarget) -> Result<(), String> {
            self.loader.mark_thread_read(target)?;
            self.publish_notification_read_receipt(
                crate::live::SlackNotificationReadReceipt::Thread {
                    team_id: self.loader.team_id().to_string(),
                    conversation_id: target.conversation_id().to_string(),
                    thread_timestamp: target.thread_timestamp().clone(),
                    read_through: target.message_timestamp().clone(),
                },
            );
            Ok(())
        }

        fn load_slack_thread_containing_reply(
            &self,
            conversation_id: &str,
            thread_timestamp: &crate::model::SlackMessageTimestamp,
            reply_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<SlackThreadSnapshot, String> {
            self.loader.load_thread_containing_reply(
                conversation_id,
                thread_timestamp,
                reply_timestamp,
            )
        }

        fn send_slack_thread_reply(
            &self,
            target: crate::model::SlackThreadReplyTarget<'_>,
            client_message_id: &crate::model::SlackMessageClientId,
            draft: &SlackMessageDraft,
        ) -> Result<SlackThreadReplyReceipt, String> {
            self.loader
                .post_thread_reply(target, client_message_id, draft)
        }

        fn search_slack_messages(
            &self,
            query: &str,
            cursor: Option<&str>,
        ) -> Result<SlackSearchSnapshot, String> {
            self.loader.search_messages(query, cursor)
        }

        fn search_slack_messages_with_options(
            &self,
            request: &crate::model::SlackMessageSearchRequest,
        ) -> Result<SlackSearchSnapshot, String> {
            self.loader.search_messages_with_options(request)
        }

        fn search_slack_quick_switch(
            &self,
            query: &str,
            recent_channels: &[String],
        ) -> Result<SlackQuickSearchSnapshot, String> {
            self.loader.search_quick_switch(query, recent_channels)
        }

        fn search_slack_quick_messages(
            &self,
            query: &crate::model::SlackQuickMessageQuery,
            recent_channels: &[String],
        ) -> Result<Vec<crate::model::SlackQuickSearchMessage>, String> {
            self.loader.search_quick_messages(query, recent_channels)
        }

        fn load_slack_workspace(&self, conversation_id: &str) -> Result<SlackWorkspace, String> {
            self.load_slack_workspace_with_presence(conversation_id)
        }

        fn load_cached_slack_workspace(
            &self,
            conversation_id: &str,
        ) -> Result<Option<SlackWorkspace>, String> {
            self.cached_workspace(conversation_id)
        }
    };
}

pub(super) use impl_workspace_api_reads;
