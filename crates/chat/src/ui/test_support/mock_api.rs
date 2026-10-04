use super::{
    wait_test_delay, Arc, HashMap, Mutex, RemoteImageData, SlackProfile, SlackUploadFile,
    SlackWorkspace,
};

mod messages;

pub type SentMessages = Arc<Mutex<Vec<SlackSendAttempt>>>;
pub type SlackLoadMetricsHandle = Arc<Mutex<MockSlackLoadMetrics>>;
pub type SlackTestConversations = Arc<Mutex<HashMap<String, SlackWorkspace>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackSendAttempt {
    pub conversation_id: String,
    pub client_message_id: crate::model::SlackMessageClientId,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MockSlackSendOutcome {
    #[default]
    Success,
    Rejected(String),
    AcceptedWithError(String),
    AcceptedWithInvalidReceipt,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MockSlackLoadMetrics {
    pub active_loads: usize,
    pub max_concurrent_loads: usize,
    pub requested_conversation_ids: Vec<String>,
}

#[derive(Clone)]
pub struct MockSlackWorkspaceApi {
    pub conversations: SlackTestConversations,
    pub profiles: HashMap<String, SlackProfile>,
    pub remote_images: HashMap<String, RemoteImageData>,
    pub sent_messages: SentMessages,
    pub load_delay: std::time::Duration,
    pub send_delay: std::time::Duration,
    pub send_outcome: MockSlackSendOutcome,
    pub load_metrics: Option<SlackLoadMetricsHandle>,
}

impl crate::model::SlackWorkspaceApi for MockSlackWorkspaceApi {
    fn capabilities(&self) -> crate::model::SlackWorkspaceApiCapabilities {
        crate::model::SlackWorkspaceApiCapabilities {
            subscribe_realtime: false,
            search_messages: false,
            search_quick_switch: false,
            load_cached_workspace: false,
            prepare_attachment_media: false,
            schedule_message: false,
            delete_scheduled_message: false,
            refresh_conversation: matches!(
                &self.send_outcome,
                MockSlackSendOutcome::AcceptedWithError(_)
                    | MockSlackSendOutcome::AcceptedWithInvalidReceipt
            ),
            mark_conversation_read: false,
            navigate_conversation_dates: false,
            load_pins: false,
            load_bookmark_folder: false,
            load_activity: false,
            load_later: false,
            mutate_later_reminders: false,
            load_files: false,
            load_conversation_files: false,
            load_conversation_members: false,
            load_channel_details: false,
            load_preferred_skin_tone: false,
            mutate_preferred_skin_tone: false,
            load_destination_directory: false,
            load_all_threads: false,
            load_thread: false,
            mark_thread_read: false,
            send_thread_reply: false,
            open_conversation: false,
            forward_message: false,
            load_message_permalink: false,
            update_message: false,
            delete_message: false,
            mutate_message_saved_state: false,
            load_reaction_catalog: false,
            ..crate::model::SlackWorkspaceApiCapabilities::LIVE
        }
    }

    fn load_slack_shell(&self) -> Result<crate::model::SlackShellSnapshot, String> {
        self.conversations
            .lock()
            .expect("test conversations mutex poisoned")
            .values()
            .next()
            .map(SlackWorkspace::shell_snapshot)
            .ok_or_else(|| "missing test Slack workspace".to_string())
    }

    fn load_slack_sidebar(
        &self,
        conversation_id: &str,
    ) -> Result<crate::model::SlackSidebarSnapshot, String> {
        self.conversations
            .lock()
            .expect("test conversations mutex poisoned")
            .get(conversation_id)
            .map(SlackWorkspace::sidebar_snapshot)
            .ok_or_else(|| format!("missing test conversation {conversation_id}"))
    }

    fn load_slack_dm_inbox(
        &self,
        _cursor: Option<&str>,
    ) -> Result<crate::model::SlackDmInboxSnapshot, String> {
        Ok(crate::model::SlackDmInboxSnapshot::default())
    }

    fn load_slack_all_threads(
        &self,
        _cursor: Option<&crate::model::SlackAllThreadsCursor>,
    ) -> Result<crate::model::SlackAllThreadsSnapshot, String> {
        Err("Slack All Threads is unavailable for the test workspace API".to_string())
    }

    fn load_slack_activity(
        &self,
        _cursor: Option<&crate::model::SlackActivityCursor>,
    ) -> Result<crate::model::SlackActivitySnapshot, String> {
        Err("Slack Activity is unavailable for the test workspace API".to_string())
    }

    fn load_slack_later(
        &self,
        _filter: crate::model::SlackLaterFilter,
        _cursor: Option<&crate::model::SlackLaterCursor>,
    ) -> Result<crate::model::SlackLaterSnapshot, String> {
        Err("Slack Later is unavailable for the test workspace API".to_string())
    }

    fn hydrate_slack_later_item(
        &self,
        _target: crate::model::SlackLaterHydrationTarget,
    ) -> Result<crate::model::SlackLaterHydratedItem, String> {
        Err("Slack Later hydration is unavailable for the test workspace API".to_string())
    }

    fn load_slack_files(
        &self,
        _request: crate::model::SlackFilesRequest,
    ) -> Result<crate::model::SlackFilesSnapshot, String> {
        Err("Slack Files is unavailable for the test workspace API".to_string())
    }

    fn load_slack_conversation_files(
        &self,
        _request: crate::model::SlackConversationFilesRequest,
    ) -> Result<crate::model::SlackConversationFilesSnapshot, String> {
        Err("Slack conversation files are unavailable for the test workspace API".to_string())
    }

    fn load_slack_drafts_sent(
        &self,
        _request: crate::model::SlackDraftsSentRequest,
    ) -> Result<crate::model::SlackDraftsSentSnapshot, String> {
        Err("Slack Drafts & sent is unavailable for the test workspace API".to_string())
    }

    fn load_slack_pins(
        &self,
        _request: crate::model::SlackPinsRequest,
    ) -> Result<crate::model::SlackPinsSnapshot, String> {
        Err("Slack Pins is unavailable for the test workspace API".to_string())
    }

    fn load_slack_destination_directory(
        &self,
    ) -> Result<crate::model::SlackDestinationDirectorySnapshot, String> {
        Err("Slack destination directory is unavailable for the test workspace API".to_string())
    }

    fn open_slack_conversation(
        &self,
        _request: crate::model::SlackConversationOpenRequest,
    ) -> Result<crate::model::SlackConversationOpenReceipt, String> {
        Err("Slack conversations.open is unavailable for the test workspace API".to_string())
    }

    fn load_slack_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<crate::model::SlackConversationSnapshot, String> {
        self.load_slack_workspace(conversation_id)
            .map(|workspace| workspace.conversation_snapshot())
    }

    fn refresh_slack_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<crate::model::SlackConversationSnapshot, String> {
        self.load_slack_conversation(conversation_id)
    }

    fn load_slack_conversation_history_page(
        &self,
        _conversation_id: &str,
        _cursor: &crate::model::SlackConversationHistoryCursor,
    ) -> Result<crate::model::SlackConversationHistoryPage, String> {
        Err(
            "Slack conversation history paging is unavailable for the test workspace API"
                .to_string(),
        )
    }

    fn load_slack_thread(
        &self,
        _conversation_id: &str,
        _thread_timestamp: &crate::model::SlackMessageTimestamp,
        _cursor: Option<&str>,
    ) -> Result<crate::model::SlackThreadSnapshot, String> {
        Err("Slack threads are unavailable for the test workspace API".to_string())
    }

    fn send_slack_thread_reply(
        &self,
        _target: crate::model::SlackThreadReplyTarget<'_>,
        _client_message_id: &crate::model::SlackMessageClientId,
        _draft: &crate::model::SlackMessageDraft,
    ) -> Result<crate::model::SlackThreadReplyReceipt, String> {
        Err("Slack thread replies are unavailable for the test workspace API".to_string())
    }

    fn search_slack_messages(
        &self,
        _query: &str,
        _cursor: Option<&str>,
    ) -> Result<crate::model::SlackSearchSnapshot, String> {
        Err("Slack message search is unavailable for the test workspace API".to_string())
    }

    fn load_slack_workspace(&self, conversation_id: &str) -> Result<SlackWorkspace, String> {
        if let Some(metrics) = self.load_metrics.as_ref() {
            let mut metrics = metrics.lock().expect("slack load metrics mutex poisoned");
            metrics.active_loads += 1;
            metrics.max_concurrent_loads = metrics.max_concurrent_loads.max(metrics.active_loads);
            metrics
                .requested_conversation_ids
                .push(conversation_id.to_string());
        }
        if !self.load_delay.is_zero() {
            wait_test_delay(self.load_delay);
        }
        let result = self
            .conversations
            .lock()
            .expect("test conversations mutex poisoned")
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| format!("missing test conversation {conversation_id}"));
        if let Some(metrics) = self.load_metrics.as_ref() {
            let mut metrics = metrics.lock().expect("slack load metrics mutex poisoned");
            metrics.active_loads = metrics.active_loads.saturating_sub(1);
        }
        result
    }

    fn send_slack_message(
        &self,
        conversation_id: &str,
        client_message_id: &crate::model::SlackMessageClientId,
        draft: &crate::model::SlackMessageDraft,
    ) -> Result<crate::model::SlackMessageSendReceipt, String> {
        messages::send_slack_message(self, conversation_id, client_message_id, draft)
    }

    fn create_slack_scheduled_draft(
        &self,
        _target: crate::model::SlackScheduledDraftCreateTarget<'_>,
        _content: crate::model::SlackDraftContent<'_, crate::model::SlackMessageDraft>,
        _file_ids: &[crate::model::SlackFileId],
    ) -> Result<
        crate::model::SlackScheduledDraftReceipt,
        crate::model::SlackScheduledDraftMutationFailure,
    > {
        Err(crate::model::SlackScheduledDraftMutationFailure::NotSent {
            diagnostic: "Slack message scheduling is unavailable for the test workspace API"
                .to_string(),
        })
    }

    fn upload_slack_files(
        &self,
        target: &crate::model::SlackFileUploadTarget,
        text: &str,
        files: Vec<SlackUploadFile>,
    ) -> Result<
        crate::model::SlackFileUploadReceipt<
            crate::model::SlackConversationSnapshot,
            crate::model::SlackThreadSnapshot,
        >,
        String,
    > {
        messages::upload_slack_files(self, target, text, files)
    }

    fn load_slack_profile(&self, user_id: &str) -> Result<SlackProfile, String> {
        self.profiles
            .get(user_id)
            .cloned()
            .ok_or_else(|| format!("missing test profile {user_id}"))
    }

    fn load_slack_remote_image(&self, url: &str) -> Result<Option<RemoteImageData>, String> {
        Ok(self.remote_images.get(url).cloned())
    }
}
