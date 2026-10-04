macro_rules! slack_workspace_conversation_api {
    () => {
        fn open_slack_conversation(
            &self,
            request: crate::model::SlackConversationOpenRequest,
        ) -> Result<crate::model::SlackConversationOpenReceipt, String>;
        fn load_slack_conversation(
            &self,
            conversation_id: &str,
        ) -> Result<crate::model::SlackConversationSnapshot, String>;
        fn refresh_slack_conversation(
            &self,
            _conversation_id: &str,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err("Slack conversation refresh is unavailable for this workspace runtime".to_string())
        }
        fn mark_slack_conversation_read(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<crate::model::SlackConversationReadReceipt, String> {
            Err(
                "Slack conversation read-state mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_conversation_at(
            &self,
            _conversation_id: &str,
            _message_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err(
                "Slack conversation date navigation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_conversation_beginning(
            &self,
            _conversation_id: &str,
        ) -> Result<crate::model::SlackConversationSnapshot, String> {
            Err(
                "Slack conversation date navigation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_conversation_history_page(
            &self,
            conversation_id: &str,
            cursor: &crate::model::SlackConversationHistoryCursor,
        ) -> Result<crate::model::SlackConversationHistoryPage, String>;
        fn load_slack_thread(
            &self,
            conversation_id: &str,
            thread_timestamp: &crate::model::SlackMessageTimestamp,
            cursor: Option<&str>,
        ) -> Result<crate::model::SlackThreadSnapshot, String>;
        fn load_slack_thread_with_read_metadata(
            &self,
            conversation_id: &str,
            thread_timestamp: &crate::model::SlackMessageTimestamp,
            cursor: Option<&str>,
        ) -> Result<crate::model::SlackThreadLoad, String> {
            self.load_slack_thread(conversation_id, thread_timestamp, cursor)
                .map(|snapshot| crate::model::SlackThreadLoad {
                    snapshot,
                    read_metadata: None,
                })
        }
        fn mark_slack_thread_read(
            &self,
            _target: &crate::model::SlackThreadReadTarget,
        ) -> Result<(), String> {
            Err(
                "Slack thread read-state mutation is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn load_slack_thread_containing_reply(
            &self,
            _conversation_id: &str,
            _thread_timestamp: &crate::model::SlackMessageTimestamp,
            _reply_timestamp: &crate::model::SlackMessageTimestamp,
        ) -> Result<crate::model::SlackThreadSnapshot, String> {
            Err(
                "Slack anchored thread loading is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn search_slack_messages(
            &self,
            query: &str,
            cursor: Option<&str>,
        ) -> Result<crate::model::SlackSearchSnapshot, String>;
        fn search_slack_messages_with_options(
            &self,
            request: &crate::model::SlackMessageSearchRequest,
        ) -> Result<crate::model::SlackSearchSnapshot, String> {
            self.search_slack_messages(&request.query, request.cursor.as_deref())
        }
        fn search_slack_quick_switch(
            &self,
            _query: &str,
            _recent_channels: &[String],
        ) -> Result<crate::model::SlackQuickSearchSnapshot, String> {
            Err("Slack quick search is unavailable for this workspace runtime".to_string())
        }
        fn search_slack_quick_messages(
            &self,
            _query: &crate::model::SlackQuickMessageQuery,
            _recent_channels: &[String],
        ) -> Result<Vec<crate::model::SlackQuickSearchMessage>, String> {
            Err("Slack quick-message search is unavailable for this workspace runtime".to_string())
        }
        fn load_slack_workspace(
            &self,
            conversation_id: &str,
        ) -> Result<crate::model::SlackWorkspace, String>;
        fn load_cached_slack_workspace(
            &self,
            _conversation_id: &str,
        ) -> Result<Option<crate::model::SlackWorkspace>, String> {
            Ok(None)
        }
        fn load_slack_profile(&self, user_id: &str) -> Result<crate::model::SlackProfile, String>;
        fn load_slack_remote_image(
            &self,
            url: &str,
        ) -> Result<Option<remote_image_model::RemoteImageData>, String>;
        fn load_slack_remote_image_with_timeout(
            &self,
            url: &str,
            timeout: std::time::Duration,
        ) -> Result<Option<remote_image_model::RemoteImageData>, String> {
            let _ = timeout;
            self.load_slack_remote_image(url)
        }
        fn prepare_slack_attachment_media(
            &self,
            _media: &crate::model::SlackAttachmentMedia,
            _declared_mimetype: &str,
        ) -> Result<crate::model::SlackPreparedMediaSource, String> {
            Err(
                "Slack attachment media playback is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
        fn release_slack_attachment_media(
            &self,
            _source: &crate::model::SlackPreparedMediaSource,
        ) -> Result<(), String> {
            Err(
                "Slack attachment media playback is unavailable for this workspace runtime"
                    .to_string(),
            )
        }
    };
}

pub(super) use slack_workspace_conversation_api;
