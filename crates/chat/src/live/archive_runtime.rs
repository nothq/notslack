use std::path::Path;

use crate::model::{
    SlackActivityCursor, SlackActivitySnapshot, SlackAllThreadsCursor, SlackAllThreadsSnapshot,
    SlackConversationFilesRequest, SlackConversationFilesSnapshot, SlackConversationHistoryCursor,
    SlackConversationHistoryPage, SlackConversationOpenReceipt, SlackConversationOpenRequest,
    SlackConversationSnapshot, SlackDestinationDirectorySnapshot, SlackDmInboxSnapshot,
    SlackDraftsSentRequest, SlackDraftsSentSnapshot, SlackFilesRequest, SlackFilesSnapshot,
    SlackLaterCursor, SlackLaterFilter, SlackLaterHydratedItem, SlackLaterHydrationTarget,
    SlackLaterSnapshot, SlackMessageDraft, SlackMessageSendReceipt, SlackPinsRequest,
    SlackPinsSnapshot, SlackProfile, SlackScheduledDraftReceipt, SlackSearchSnapshot,
    SlackShellSnapshot, SlackSidebarSnapshot, SlackThreadReplyReceipt, SlackThreadSnapshot,
    SlackWorkspace,
};
#[cfg(test)]
use crate::model::{SlackAttachment, SlackMessage};

use crate::live::{load_slack_archive, load_slack_remote_image};

#[derive(Clone)]
pub struct SlackArchiveWorkspaceRuntime {
    state_path: String,
}

impl SlackArchiveWorkspaceRuntime {
    pub fn new(state_path: String) -> Self {
        Self { state_path }
    }
}

impl crate::model::SlackWorkspaceApi for SlackArchiveWorkspaceRuntime {
    fn capabilities(&self) -> crate::model::SlackWorkspaceApiCapabilities {
        crate::model::SlackWorkspaceApiCapabilities::ARCHIVE
    }

    fn load_slack_shell(&self) -> Result<SlackShellSnapshot, String> {
        Ok(load_slack_archive(Path::new(&self.state_path))?.shell_snapshot())
    }

    fn load_slack_sidebar(&self, conversation_id: &str) -> Result<SlackSidebarSnapshot, String> {
        Ok(self
            .load_slack_workspace(conversation_id)?
            .sidebar_snapshot())
    }

    fn load_slack_dm_inbox(&self, _cursor: Option<&str>) -> Result<SlackDmInboxSnapshot, String> {
        Err("Slack DM inbox loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_all_threads(
        &self,
        _cursor: Option<&SlackAllThreadsCursor>,
    ) -> Result<SlackAllThreadsSnapshot, String> {
        Err("Slack All Threads loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_activity(
        &self,
        _cursor: Option<&SlackActivityCursor>,
    ) -> Result<SlackActivitySnapshot, String> {
        Err("Slack Activity loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_later(
        &self,
        _filter: SlackLaterFilter,
        _cursor: Option<&SlackLaterCursor>,
    ) -> Result<SlackLaterSnapshot, String> {
        Err("Slack Later loading is unavailable for archived workspaces".to_string())
    }

    fn hydrate_slack_later_item(
        &self,
        _target: SlackLaterHydrationTarget,
    ) -> Result<SlackLaterHydratedItem, String> {
        Err("Slack Later hydration is unavailable for archived workspaces".to_string())
    }

    fn load_slack_files(&self, _request: SlackFilesRequest) -> Result<SlackFilesSnapshot, String> {
        Err("Slack Files loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_conversation_files(
        &self,
        _request: SlackConversationFilesRequest,
    ) -> Result<SlackConversationFilesSnapshot, String> {
        Err("Slack conversation Files loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_drafts_sent(
        &self,
        _request: SlackDraftsSentRequest,
    ) -> Result<SlackDraftsSentSnapshot, String> {
        Err("Slack Drafts & sent loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_pins(&self, _request: SlackPinsRequest) -> Result<SlackPinsSnapshot, String> {
        Err("Slack Pins loading is unavailable for archived workspaces".to_string())
    }

    fn load_slack_destination_directory(
        &self,
    ) -> Result<SlackDestinationDirectorySnapshot, String> {
        Err("Slack destination directory is unavailable for archived workspaces".to_string())
    }

    fn open_slack_conversation(
        &self,
        _request: SlackConversationOpenRequest,
    ) -> Result<SlackConversationOpenReceipt, String> {
        Err("Slack conversations.open is unavailable for archived workspaces".to_string())
    }

    fn load_slack_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<SlackConversationSnapshot, String> {
        Ok(self
            .load_slack_workspace(conversation_id)?
            .conversation_snapshot())
    }

    fn load_slack_conversation_history_page(
        &self,
        _conversation_id: &str,
        _cursor: &SlackConversationHistoryCursor,
    ) -> Result<SlackConversationHistoryPage, String> {
        Err("Slack conversation history paging is unavailable for archived workspaces".to_string())
    }

    fn load_slack_thread(
        &self,
        _conversation_id: &str,
        _thread_timestamp: &crate::model::SlackMessageTimestamp,
        _cursor: Option<&str>,
    ) -> Result<SlackThreadSnapshot, String> {
        Err("Slack thread loading is unavailable for archived workspaces".to_string())
    }

    fn send_slack_thread_reply(
        &self,
        _target: crate::model::SlackThreadReplyTarget<'_>,
        _client_message_id: &crate::model::SlackMessageClientId,
        _draft: &SlackMessageDraft,
    ) -> Result<SlackThreadReplyReceipt, String> {
        Err("Slack thread replies are unavailable for archived workspaces".to_string())
    }

    fn search_slack_messages(
        &self,
        _query: &str,
        _cursor: Option<&str>,
    ) -> Result<SlackSearchSnapshot, String> {
        Err("Slack message search is unavailable for archived workspaces".to_string())
    }

    fn load_slack_workspace(&self, conversation_id: &str) -> Result<SlackWorkspace, String> {
        let archived = load_slack_archive(Path::new(&self.state_path))?;
        archived_workspace_for_conversation(&archived, conversation_id)
    }

    fn load_cached_slack_workspace(
        &self,
        _conversation_id: &str,
    ) -> Result<Option<SlackWorkspace>, String> {
        Err("Slack cached workspace loading is unavailable for archived workspaces".to_string())
    }

    fn send_slack_message(
        &self,
        _conversation_id: &str,
        _client_message_id: &crate::model::SlackMessageClientId,
        _draft: &SlackMessageDraft,
    ) -> Result<SlackMessageSendReceipt, String> {
        Err("Slack message sending is unavailable for archived workspaces".to_string())
    }

    fn create_slack_scheduled_draft(
        &self,
        _target: crate::model::SlackScheduledDraftCreateTarget<'_>,
        _content: crate::model::SlackDraftContent<'_, SlackMessageDraft>,
        _file_ids: &[crate::model::SlackFileId],
    ) -> Result<SlackScheduledDraftReceipt, crate::model::SlackScheduledDraftMutationFailure> {
        Err(crate::model::SlackScheduledDraftMutationFailure::NotSent {
            diagnostic: "Slack message scheduling is unavailable for archived workspaces"
                .to_string(),
        })
    }

    fn mutate_slack_reaction(
        &self,
        _target: &crate::model::SlackReactionTarget,
        _reaction_name: &crate::model::SlackReactionName,
        _mutation: crate::model::SlackReactionMutation,
    ) -> Result<
        crate::model::SlackReactionMutationReceipt<SlackConversationSnapshot, SlackThreadSnapshot>,
        String,
    > {
        Err("Slack reaction mutation is unavailable for archived workspaces".to_string())
    }

    fn load_slack_profile(&self, user_id: &str) -> Result<SlackProfile, String> {
        let archived = load_slack_archive(Path::new(&self.state_path))?;
        archived_slack_profile(&archived, user_id)
    }

    fn load_slack_remote_image(
        &self,
        url: &str,
    ) -> Result<Option<remote_image_model::RemoteImageData>, String> {
        let preview = load_slack_remote_image(url);
        eprintln!("archive remote image {url} => {}", preview.is_ok());
        Ok(preview
            .ok()
            .map(|preview| remote_image_model::RemoteImageData {
                bytes: preview.bytes,
                mimetype: preview.mimetype,
            }))
    }
}

pub fn archived_workspace_for_conversation(
    archived: &SlackWorkspace,
    conversation_id: &str,
) -> Result<SlackWorkspace, String> {
    let selected_item = archived
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.target_id == conversation_id);
    let is_original_conversation = archived.conversation_id == conversation_id;
    if !is_original_conversation && selected_item.is_none() {
        return Err(format!(
            "missing archived Slack conversation {conversation_id}"
        ));
    }

    let mut workspace = archived.clone();
    let (channel_name, channel_kind) = selected_item
        .map(|item| (item.label.clone(), item.target_kind))
        .unwrap_or_else(|| (archived.channel_name.clone(), archived.channel_kind));
    workspace.conversation_id = conversation_id.to_string();
    workspace.channel_name = channel_name.clone();
    workspace.channel_kind = channel_kind;
    workspace.composer_placeholder = channel_kind.composer_placeholder(&channel_name);
    if !is_original_conversation {
        workspace.channel_topic.clear();
        workspace.member_count = None;
        workspace.messages.clear();
        workspace.last_read = None;
        workspace.last_read_boundary_loaded = true;
    }
    for item in workspace
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        item.active = item.target_id == conversation_id;
    }
    Ok(workspace)
}

#[cfg(test)]
pub(crate) fn append_archived_slack_message(
    mut workspace: SlackWorkspace,
    text: &str,
    attachments: Vec<SlackAttachment>,
) -> SlackWorkspace {
    workspace.messages.push(SlackMessage {
        id: format!("archive-sent-{}", workspace.messages.len() + 1),
        client_message_id: None,
        author: "You".to_string(),
        timestamp: "Now".to_string(),
        user_id: Some("U_ARCHIVE_SELF".to_string()),
        avatar_label: Some("YO".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: text.to_string(),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments,
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    });
    workspace
}

fn archived_slack_profile(
    archived: &SlackWorkspace,
    user_id: &str,
) -> Result<SlackProfile, String> {
    let from_messages = archived
        .messages
        .iter()
        .find(|message| message.user_id.as_deref() == Some(user_id))
        .map(|message| (message.author.clone(), message.avatar_label.clone()));
    let from_sidebar = archived
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.user_id.as_deref() == Some(user_id))
        .map(|item| (item.label.clone(), None));
    let Some((display_name, avatar_label)) = from_messages.or(from_sidebar) else {
        return Err(format!("missing archived Slack profile {user_id}"));
    };
    Ok(SlackProfile {
        user_id: user_id.to_string(),
        display_name: display_name.clone(),
        real_name: display_name,
        title: None,
        status_text: None,
        email: None,
        phone: None,
        timezone_label: None,
        avatar_label,
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
    })
}
