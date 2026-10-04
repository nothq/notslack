macro_rules! slack_workspace_discovery_api {
    () => {
        fn capabilities(&self) -> crate::model::SlackWorkspaceApiCapabilities;
        fn subscribe_slack_realtime(
            &self,
        ) -> Result<Box<dyn crate::model::SlackRealtimeSubscription>, String> {
            Err("Slack realtime events are unavailable for this workspace runtime".to_string())
        }
        fn load_slack_shell(&self) -> Result<crate::model::SlackShellSnapshot, String>;
        fn mutate_slack_self_status(
            &self,
            _status: &crate::model::SlackSelfStatus,
        ) -> Result<crate::model::SlackSelfStatus, String> {
            Err("Slack status mutation is unavailable for this workspace runtime".to_string())
        }
        fn mutate_slack_self_presence(
            &self,
            _presence: crate::model::SlackUserPresence,
        ) -> Result<crate::model::SlackUserPresence, String> {
            Err("Slack presence mutation is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_sidebar(
            &self,
            conversation_id: &str,
        ) -> Result<crate::model::SlackSidebarSnapshot, String>;
        fn create_slack_sidebar_section(
            &self,
            _request: &crate::model::SlackSidebarSectionCreateRequest,
            _conversation_id: &str,
        ) -> Result<crate::model::SlackSidebarSnapshot, String> {
            Err("Slack sidebar section creation is unavailable for this workspace runtime".to_string())
        }
        fn refresh_slack_sidebar_from_realtime(
            &self,
            conversation_id: &str,
        ) -> Result<crate::model::SlackSidebarSnapshot, String> {
            self.load_slack_sidebar(conversation_id)
        }
        fn load_slack_dm_inbox(
            &self,
            cursor: Option<&str>,
        ) -> Result<crate::model::SlackDmInboxSnapshot, String>;
        fn load_cached_slack_dm_inbox(&self) -> Result<Option<crate::model::SlackDmInboxSnapshot>, String> {
            Ok(None)
        }
        fn load_slack_all_threads(
            &self,
            cursor: Option<&crate::model::SlackAllThreadsCursor>,
        ) -> Result<crate::model::SlackAllThreadsSnapshot, String>;
        fn load_slack_activity(
            &self,
            cursor: Option<&crate::model::SlackActivityCursor>,
        ) -> Result<crate::model::SlackActivitySnapshot, String>;
        fn mark_slack_activity_item_read(
            &self,
            _target: &crate::model::SlackActivityReadTarget,
        ) -> Result<(), String> {
            Err(
                "Slack Activity item read-state mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn mark_slack_activity_item_unread(
            &self,
            _target: &crate::model::SlackActivityReadTarget,
        ) -> Result<(), String> {
            Err(
                "Slack Activity item unread-state mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn archive_slack_activity_item(
            &self,
            _target: &crate::model::SlackActivityArchiveTarget,
            _reason: &str,
        ) -> Result<(), String> {
            Err("Slack Activity item archival is unavailable for this workspace runtime".to_string())
        }
        fn unarchive_slack_activity_item(
            &self,
            _target: &crate::model::SlackActivityArchiveTarget,
            _reason: &str,
        ) -> Result<(), String> {
            Err(
                "Slack Activity item restoration is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_later(
            &self,
            filter: crate::model::SlackLaterFilter,
            cursor: Option<&crate::model::SlackLaterCursor>,
        ) -> Result<crate::model::SlackLaterSnapshot, String>;
        fn hydrate_slack_later_item(
            &self,
            target: crate::model::SlackLaterHydrationTarget,
        ) -> Result<crate::model::SlackLaterHydratedItem, String>;
        fn mutate_slack_reminder(
            &self,
            _mutation: &crate::model::SlackReminderMutation,
        ) -> Result<(), String> {
            Err("Slack reminder mutation is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_files(
            &self,
            request: crate::model::SlackFilesRequest,
        ) -> Result<crate::model::SlackFilesSnapshot, String>;
        fn load_slack_file_metadata(
            &self,
            _request: &crate::model::SlackFileMetadataRequest,
        ) -> Result<crate::model::SlackFileMetadataBatch, String> {
            Err("Slack file metadata loading is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_conversation_files(
            &self,
            request: crate::model::SlackConversationFilesRequest,
        ) -> Result<crate::model::SlackConversationFilesSnapshot, String>;
        fn load_slack_drafts_sent(
            &self,
            request: crate::model::SlackDraftsSentRequest,
        ) -> Result<crate::model::SlackDraftsSentSnapshot, String>;
        fn load_slack_pins(
            &self,
            request: crate::model::SlackPinsRequest,
        ) -> Result<crate::model::SlackPinsSnapshot, String>;
        fn load_slack_bookmark_folder(
            &self,
            _request: crate::model::SlackBookmarkFolderRequest,
        ) -> Result<crate::model::SlackBookmarkFolderSnapshot, String> {
            Err("Slack bookmark folders are unavailable for this workspace runtime".to_string())
        }
        fn load_slack_conversation_members(
            &self,
            _conversation_id: &str,
            _cursor: Option<&crate::model::SlackConversationMembersCursor>,
        ) -> Result<crate::model::SlackConversationMembersSnapshot, String> {
            Err("Slack conversation members are unavailable for this workspace runtime".to_string())
        }
        fn load_slack_channel_details(
            &self,
            _conversation_id: &str,
        ) -> Result<crate::model::SlackChannelDetailsSnapshot, String> {
            Err("Slack channel details are unavailable for this workspace runtime".to_string())
        }
        fn load_slack_channel_notification_preference(
            &self,
            _conversation_id: &str,
        ) -> Result<crate::model::SlackChannelNotificationPreference, String> {
            Err(
                "Slack channel notification preferences are unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn mutate_slack_channel_notification_preference(
            &self,
            _conversation_id: &str,
            _mutation: crate::model::SlackChannelNotificationMutation,
        ) -> Result<crate::model::SlackChannelNotificationPreference, String> {
            Err(
                "Slack channel notification preference mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_preferred_skin_tone(
            &self,
        ) -> Result<crate::model::SlackPreferredSkinTone, String> {
            Err("Slack preferred skin tone is unavailable for this workspace runtime".to_string())
        }
        fn mutate_slack_preferred_skin_tone(
            &self,
            _selection: crate::model::SlackSkinTone,
        ) -> Result<crate::model::SlackPreferredSkinTone, String> {
            Err(
                "Slack preferred skin-tone mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_destination_directory(
            &self,
        ) -> Result<crate::model::SlackDestinationDirectorySnapshot, String>;
        fn load_slack_reaction_catalog(
            &self,
        ) -> Result<std::sync::Arc<crate::model::SlackReactionCatalogSnapshot>, String> {
            Err("Slack reaction catalog loading is unavailable for this workspace runtime"
                .to_string())
        }
    };
}

pub(super) use slack_workspace_discovery_api;
