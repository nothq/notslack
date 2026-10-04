use std::time::Duration;

use remote_image_model::RemoteImageData;

use crate::model::{
    SlackConversationSnapshot, SlackDmInboxSnapshot, SlackShellSnapshot, SlackSidebarSnapshot,
    SlackStarMutation, SlackWorkspace, SlackWorkspaceShell,
};

use crate::live::SlackWorkspaceCacheStore;

use super::SlackWorkspaceRuntime;
use super::{merge_slack_conversation_windows, presence::overlay_sidebar_presence};

impl SlackWorkspaceRuntime {
    pub(super) fn cache(&self) -> Option<&SlackWorkspaceCacheStore> {
        self.cache
            .get_or_init(|| {
                SlackWorkspaceCacheStore::for_team(self.loader.team_id()).inspect_err(|error| {
                    eprintln!("Slack cache unavailable: {error}");
                })
            })
            .as_ref()
            .ok()
    }

    pub fn startup_shell(&self, conversation_id: &str) -> SlackWorkspaceShell {
        SlackWorkspaceShell {
            team_id: self.loader.team_id().to_string(),
            conversation_id: conversation_id.to_string(),
        }
    }

    pub(super) fn cached_workspace(
        &self,
        conversation_id: &str,
    ) -> Result<Option<SlackWorkspace>, String> {
        let Some(cache) = self.cache() else {
            return Ok(None);
        };
        let mut workspace = cache.load_workspace(conversation_id)?;
        if let Some(workspace) = workspace.as_mut() {
            self.overlay_workspace_presence(workspace);
            let conversation =
                self.merge_cached_confirmed_sends(workspace.conversation_snapshot())?;
            workspace.apply_shaped_conversation_snapshot(conversation);
            self.loader.prepare_conversation_tabs(&mut workspace.tabs);
        }
        Ok(workspace)
    }

    pub(super) fn load_and_cache_shell(&self) -> Result<SlackShellSnapshot, String> {
        let shell = self.loader.load_shell()?;
        self.update_presence_self_user(shell.self_user_id.as_deref())?;
        *self
            .shell_snapshot
            .lock()
            .map_err(|_| "Slack shell snapshot mutex poisoned".to_string())? = Some(shell.clone());
        if let Some(cache) = self.cache() {
            cache.persist_shell(&self.resolve_startup_conversation_id()?, shell.clone());
        }
        Ok(shell)
    }

    pub(super) fn load_and_cache_sidebar(
        &self,
        conversation_id: &str,
    ) -> Result<SlackSidebarSnapshot, String> {
        let roster_generation = self.next_presence_roster_generation();
        let mut sidebar = self.loader.load_sidebar(conversation_id)?;
        let _commit = self
            .presence_roster_commit
            .lock()
            .map_err(|_| "Slack presence roster commit mutex poisoned".to_string())?;
        let accepted = self.update_and_overlay_sidebar_presence(&mut sidebar, roster_generation)?;
        if let Some(cache) = self.cache() {
            cache.reconcile_sidebar_read_receipts(&mut sidebar)?;
            if accepted {
                if let Some(shell) = self
                    .shell_snapshot
                    .lock()
                    .map_err(|_| "Slack shell snapshot mutex poisoned".to_string())?
                    .clone()
                {
                    cache.persist_shell(conversation_id, shell);
                }
                cache.persist_sidebar(sidebar.clone());
            }
        }
        Ok(sidebar)
    }

    pub(super) fn refresh_sidebar_from_realtime(
        &self,
        conversation_id: &str,
    ) -> Result<SlackSidebarSnapshot, String> {
        self.loader.invalidate_realtime_sidebar_caches()?;
        self.load_and_cache_sidebar(conversation_id)
    }

    pub(super) fn load_and_cache_dm_inbox(
        &self,
        cursor: Option<&str>,
    ) -> Result<SlackDmInboxSnapshot, String> {
        let mut snapshot = self.loader.load_dm_inbox(cursor)?;
        self.overlay_dm_inbox_presence(&mut snapshot);
        if let Some(cache) = self.cache() {
            cache.reconcile_dm_inbox_read_receipts(&mut snapshot)?;
            if cursor.is_none() {
                cache.persist_dm_inbox(snapshot.clone());
            }
        }
        Ok(snapshot)
    }

    pub(super) fn refresh_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<SlackConversationSnapshot, String> {
        let conversation =
            self.reconcile_remote_conversation(self.loader.refresh_conversation(conversation_id)?)?;
        self.loader
            .remember_conversation_tab_metadata(&conversation.tabs);
        if let Some(cache) = self.cache() {
            cache.persist_conversation_tab_metadata(&conversation);
        }
        Ok(conversation)
    }

    pub(super) fn mutate_channel_star(
        &self,
        conversation_id: &str,
        mutation: SlackStarMutation,
    ) -> Result<SlackSidebarSnapshot, String> {
        let mut sidebar = self.loader.mutate_channel_star(conversation_id, mutation)?;
        let Ok(state) = self.presence_state.lock() else {
            return Err(
                "Slack presence state mutex poisoned while applying starred channel".to_string(),
            );
        };
        overlay_sidebar_presence(&mut sidebar, &state);
        drop(state);
        if let Some(cache) = self.cache() {
            cache.persist_sidebar(sidebar.clone());
        }
        Ok(sidebar)
    }

    pub(super) fn load_remote_image(&self, url: &str) -> Result<Option<RemoteImageData>, String> {
        self.load_remote_image_with_timeout(url, Duration::from_secs(15))
    }

    pub(super) fn load_remote_image_with_timeout(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<Option<RemoteImageData>, String> {
        Ok(self
            .loader
            .load_attachment_preview_with_timeout(url, timeout)?
            .map(|preview| RemoteImageData {
                bytes: preview.bytes,
                mimetype: preview.mimetype,
            }))
    }

    pub(super) fn commit_workspace(&self, workspace: &SlackWorkspace) -> Result<(), String> {
        self.selection_store
            .remember_slack_team_conversation(self.loader.team_id(), &workspace.conversation_id)?;
        self.loader
            .remember_conversation_tab_metadata(&workspace.tabs);
        if let Some(cache) = self.cache() {
            cache.persist_workspace(workspace);
        }
        Ok(())
    }

    pub(super) fn commit_conversation(
        &self,
        conversation: &SlackConversationSnapshot,
    ) -> Result<(), String> {
        self.selection_store.remember_slack_team_conversation(
            self.loader.team_id(),
            &conversation.conversation_id,
        )?;
        self.loader
            .remember_conversation_tab_metadata(&conversation.tabs);
        if let Some(cache) = self.cache() {
            cache.persist_conversation(conversation.clone());
        }
        Ok(())
    }

    pub(super) fn remember_conversation_metadata(
        &self,
        conversation: &SlackConversationSnapshot,
    ) -> Result<(), String> {
        self.selection_store.remember_slack_team_conversation(
            self.loader.team_id(),
            &conversation.conversation_id,
        )?;
        self.loader
            .remember_conversation_tab_metadata(&conversation.tabs);
        Ok(())
    }

    pub(super) fn load_and_remember_conversation(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<SlackConversationSnapshot, String> {
        let conversation = match anchor_timestamp {
            Some(anchor_timestamp) => {
                let latest_remote = self.loader.load_conversation(conversation_id)?;
                if latest_remote
                    .messages
                    .iter()
                    .any(|message| message.id == anchor_timestamp)
                {
                    let latest = self.reconcile_remote_conversation(latest_remote)?;
                    self.commit_conversation(&latest)?;
                    latest
                } else {
                    let anchored = self
                        .loader
                        .load_conversation_with_anchor(conversation_id, Some(anchor_timestamp))?;
                    let merged_remote =
                        merge_slack_conversation_windows(latest_remote.clone(), anchored)?;
                    let conversation = self.reconcile_remote_conversation(merged_remote)?;
                    let latest_for_cache = self.merge_cached_confirmed_sends(latest_remote)?;
                    self.commit_conversation(&latest_for_cache)?;
                    conversation
                }
            }
            None => {
                let latest = self.reconcile_remote_conversation(
                    self.loader.load_conversation(conversation_id)?,
                )?;
                self.commit_conversation(&latest)?;
                latest
            }
        };
        if let Some(cache) = self.cache() {
            if let Some(shell) = self
                .shell_snapshot
                .lock()
                .map_err(|_| "Slack shell snapshot mutex poisoned".to_string())?
                .clone()
            {
                cache.persist_shell(conversation_id, shell);
            }
        }
        Ok(conversation)
    }
}
