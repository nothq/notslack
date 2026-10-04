use super::super::SlackWorkspaceRuntime;
use crate::model::{
    SlackConversationMembersCursor, SlackConversationMembersSnapshot,
    SlackDestinationDirectorySnapshot, SlackDmInboxSnapshot, SlackWorkspace,
};

impl SlackWorkspaceRuntime {
    pub(in crate::live::runtime::workspace_api) fn load_cached_dm_inbox_with_presence(
        &self,
    ) -> Result<Option<SlackDmInboxSnapshot>, String> {
        let mut snapshot = self
            .cache()
            .map(|cache| cache.load_dm_inbox())
            .transpose()
            .map(Option::flatten)?;
        if let Some(snapshot) = snapshot.as_mut() {
            self.overlay_dm_inbox_presence(snapshot);
        }
        Ok(snapshot)
    }

    pub(in crate::live::runtime::workspace_api) fn load_slack_workspace_with_presence(
        &self,
        conversation_id: &str,
    ) -> Result<SlackWorkspace, String> {
        let mut workspace = self.loader.load_workspace(conversation_id)?;
        self.overlay_workspace_presence(&mut workspace);
        let conversation = self.reconcile_remote_conversation(workspace.conversation_snapshot())?;
        workspace.apply_shaped_conversation_snapshot(conversation);
        if let Some(cache) = self.cache() {
            cache.reconcile_workspace_read_receipts(&mut workspace)?;
        }
        self.commit_workspace(&workspace)?;
        Ok(workspace)
    }

    pub(in crate::live::runtime::workspace_api) fn load_slack_destination_directory_with_presence(
        &self,
    ) -> Result<SlackDestinationDirectorySnapshot, String> {
        let mut snapshot = self.loader.load_destination_directory()?;
        self.overlay_destination_directory_presence(&mut snapshot);
        Ok(snapshot)
    }

    pub(in crate::live::runtime::workspace_api) fn load_slack_conversation_members_with_presence(
        &self,
        conversation_id: &str,
        cursor: Option<&SlackConversationMembersCursor>,
    ) -> Result<SlackConversationMembersSnapshot, String> {
        let mut snapshot = self
            .loader
            .load_conversation_members(conversation_id, cursor)?;
        self.overlay_members_presence(&mut snapshot);
        Ok(snapshot)
    }
}
