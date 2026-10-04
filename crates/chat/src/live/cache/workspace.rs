mod read_receipts;

use std::sync::{Arc, Mutex};

use crate::model::{
    SlackConversationSnapshot, SlackDmInboxSnapshot, SlackMessage, SlackShellSnapshot,
    SlackSidebarSnapshot, SlackWorkspace,
};

use super::{
    default_slack_cache_root_dir, fetched_at_unix_secs, SlackCachePaths, SlackConfirmedSendCache,
    SlackDmInboxCache, SlackReadReceiptsCache, SlackWorkspaceCache,
    SLACK_CONFIRMED_SEND_CACHE_LIMIT, SLACK_CONFIRMED_SEND_CACHE_SCHEMA_VERSION,
    SLACK_DM_INBOX_CACHE_ITEM_LIMIT, SLACK_DM_INBOX_CACHE_SCHEMA_VERSION,
    SLACK_READ_RECEIPTS_CACHE_LIMIT, SLACK_READ_RECEIPTS_CACHE_SCHEMA_VERSION,
    SLACK_WORKSPACE_CACHE_SCHEMA_VERSION,
};
use read_receipts::{apply_read_receipts_to_conversation, apply_read_receipts_to_sidebar};

#[cfg(test)]
mod tests;

#[derive(Default)]
struct SlackWorkspaceCacheUpdate {
    shell: Option<SlackShellSnapshot>,
    sidebar: Option<SlackSidebarSnapshot>,
    conversation: Option<SlackConversationSnapshot>,
}

#[derive(Clone)]
pub struct SlackWorkspaceCacheStore {
    team_id: String,
    paths: SlackCachePaths,
    key: [u8; 32],
    write_lock: Arc<Mutex<()>>,
}

impl SlackWorkspaceCacheStore {
    pub fn for_team(team_id: &str) -> Result<Self, String> {
        let cache_key = format!("slack-v1|{team_id}");
        let key = local_cache::load_or_create_cache_key(&cache_key, "Slack")?;
        let cache_root = default_slack_cache_root_dir()?;
        Ok(Self {
            team_id: team_id.to_string(),
            paths: SlackCachePaths::new(&cache_root, team_id),
            key,
            write_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(test)]
    fn for_test(cache_root: &std::path::Path, team_id: &str, key: [u8; 32]) -> Self {
        Self {
            team_id: team_id.to_string(),
            paths: SlackCachePaths::new(cache_root, team_id),
            key,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn load_workspace(&self, conversation_id: &str) -> Result<Option<SlackWorkspace>, String> {
        let Some(cache) = self.load_cache(conversation_id)? else {
            return Ok(None);
        };
        let Some(shell) = cache.shell else {
            return Ok(None);
        };
        let Some(conversation) = cache.conversation else {
            return Ok(None);
        };
        Ok(Some(SlackWorkspace::from_snapshots(
            shell,
            cache.sidebar,
            conversation,
        )))
    }

    pub fn persist_workspace(&self, workspace: &SlackWorkspace) {
        let result = self.persist_snapshots(
            &workspace.conversation_id,
            SlackWorkspaceCacheUpdate {
                shell: Some(workspace.shell_snapshot()),
                sidebar: Some(workspace.sidebar_snapshot()),
                conversation: Some(workspace.conversation_snapshot()),
            },
        );
        if let Err(error) = result {
            eprintln!("Slack cache write failed: {error}");
        }
    }

    pub fn persist_shell(&self, conversation_id: &str, shell: SlackShellSnapshot) {
        if let Err(error) = self.persist_snapshots(
            conversation_id,
            SlackWorkspaceCacheUpdate {
                shell: Some(shell),
                ..Default::default()
            },
        ) {
            eprintln!("Slack shell cache write failed: {error}");
        }
    }

    pub fn persist_sidebar(&self, sidebar: SlackSidebarSnapshot) {
        let conversation_id = sidebar.conversation_id.clone();
        if let Err(error) = self.persist_snapshots(
            &conversation_id,
            SlackWorkspaceCacheUpdate {
                sidebar: Some(sidebar),
                ..Default::default()
            },
        ) {
            eprintln!("Slack sidebar cache write failed: {error}");
        }
    }

    pub fn load_dm_inbox(&self) -> Result<Option<SlackDmInboxSnapshot>, String> {
        let Some(cache) = local_cache::read_encrypted_json::<SlackDmInboxCache>(
            &self.paths.dm_inbox_path(),
            &self.key,
        )?
        else {
            return Ok(None);
        };
        if cache.schema_version != SLACK_DM_INBOX_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.snapshot.team_id != self.team_id
            || cache.snapshot.items.len() > SLACK_DM_INBOX_CACHE_ITEM_LIMIT
        {
            return Ok(None);
        }
        let mut snapshot = cache.snapshot;
        self.apply_read_receipts_to_dm_inbox(&mut snapshot)?;
        Ok(Some(snapshot))
    }

    pub fn persist_dm_inbox(&self, mut dm_inbox: SlackDmInboxSnapshot) {
        let result = self
            .write_lock
            .lock()
            .map_err(|_| "Slack cache write mutex poisoned".to_string())
            .and_then(|_write_guard| {
                self.apply_read_receipts_to_dm_inbox(&mut dm_inbox)?;
                dm_inbox.items.truncate(SLACK_DM_INBOX_CACHE_ITEM_LIMIT);
                let cache = SlackDmInboxCache {
                    schema_version: SLACK_DM_INBOX_CACHE_SCHEMA_VERSION,
                    fetched_at_unix_secs: fetched_at_unix_secs(),
                    team_id: self.team_id.clone(),
                    snapshot: dm_inbox,
                };
                local_cache::write_encrypted_json(&self.paths.dm_inbox_path(), &self.key, &cache)
            });
        if let Err(error) = result {
            eprintln!("Slack DMs inbox cache write failed: {error}");
        }
    }

    pub fn persist_conversation(&self, conversation: SlackConversationSnapshot) {
        let conversation_id = conversation.conversation_id.clone();
        if let Err(error) = self.persist_snapshots(
            &conversation_id,
            SlackWorkspaceCacheUpdate {
                conversation: Some(conversation),
                ..Default::default()
            },
        ) {
            eprintln!("Slack conversation cache write failed: {error}");
        }
    }

    pub fn load_confirmed_sends(&self, conversation_id: &str) -> Result<Vec<SlackMessage>, String> {
        let Some(cache) = local_cache::read_encrypted_json::<SlackConfirmedSendCache>(
            &self.paths.confirmed_sends_path(conversation_id),
            &self.key,
        )?
        else {
            return Ok(Vec::new());
        };
        if cache.schema_version != SLACK_CONFIRMED_SEND_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.conversation_id != conversation_id
            || !valid_confirmed_sends(&cache.messages)
        {
            return Ok(Vec::new());
        }
        Ok(cache.messages)
    }

    pub fn persist_confirmed_sends(&self, conversation_id: &str, messages: &[SlackMessage]) {
        if let Err(error) = self.try_persist_confirmed_sends(conversation_id, messages) {
            eprintln!("Slack confirmed-send cache write failed: {error}");
        }
    }

    pub fn persist_conversation_tab_metadata(&self, conversation: &SlackConversationSnapshot) {
        if let Err(error) = self.try_persist_conversation_tab_metadata(conversation) {
            eprintln!("Slack conversation tab cache write failed: {error}");
        }
    }

    fn load_cache(&self, conversation_id: &str) -> Result<Option<SlackWorkspaceCache>, String> {
        let Some(mut cache) = local_cache::read_encrypted_json::<SlackWorkspaceCache>(
            &self.paths.workspace_path(conversation_id),
            &self.key,
        )?
        else {
            return Ok(None);
        };
        if cache.schema_version != SLACK_WORKSPACE_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.conversation_id != conversation_id
            || cache
                .shell
                .as_ref()
                .is_some_and(|shell| shell.team_id != self.team_id)
            || cache.sidebar.as_ref().is_some_and(|sidebar| {
                sidebar.team_id != self.team_id || sidebar.conversation_id != conversation_id
            })
            || cache.conversation.as_ref().is_some_and(|conversation| {
                conversation.team_id != self.team_id
                    || conversation.conversation_id != conversation_id
            })
        {
            return Ok(None);
        }
        self.apply_read_receipts_to_workspace_cache(&mut cache)?;
        Ok(Some(cache))
    }

    fn persist_snapshots(
        &self,
        conversation_id: &str,
        update: SlackWorkspaceCacheUpdate,
    ) -> Result<(), String> {
        let SlackWorkspaceCacheUpdate {
            shell,
            mut sidebar,
            mut conversation,
        } = update;
        let _write_guard = self
            .write_lock
            .lock()
            .map_err(|_| "Slack cache write mutex poisoned".to_string())?;
        let mut cache = self
            .load_cache(conversation_id)?
            .unwrap_or_else(|| SlackWorkspaceCache {
                schema_version: SLACK_WORKSPACE_CACHE_SCHEMA_VERSION,
                fetched_at_unix_secs: fetched_at_unix_secs(),
                team_id: self.team_id.clone(),
                conversation_id: conversation_id.to_string(),
                shell: None,
                sidebar: None,
                conversation: None,
            });
        let receipts = self.load_read_receipts()?;
        apply_read_receipts_to_sidebar(sidebar.as_mut(), &receipts);
        apply_read_receipts_to_conversation(conversation.as_mut(), &receipts);
        cache.fetched_at_unix_secs = fetched_at_unix_secs();
        if let Some(shell) = shell {
            cache.shell = Some(shell);
        }
        if let Some(sidebar) = sidebar {
            cache.sidebar = Some(sidebar);
        }
        if let Some(mut conversation) = conversation {
            if let Some(existing) = cache.conversation.as_ref() {
                conversation.merge_resolved_tab_metadata_from(existing);
            }
            cache.conversation = Some(conversation);
        }
        local_cache::write_encrypted_json(
            &self.paths.workspace_path(conversation_id),
            &self.key,
            &cache,
        )
    }

    fn try_persist_confirmed_sends(
        &self,
        conversation_id: &str,
        messages: &[SlackMessage],
    ) -> Result<(), String> {
        if !valid_confirmed_sends(messages) {
            return Err("Slack confirmed sends must use unique ascending timestamps".to_string());
        }
        let _write_guard = self
            .write_lock
            .lock()
            .map_err(|_| "Slack cache write mutex poisoned".to_string())?;
        let cache = SlackConfirmedSendCache {
            schema_version: SLACK_CONFIRMED_SEND_CACHE_SCHEMA_VERSION,
            team_id: self.team_id.clone(),
            conversation_id: conversation_id.to_string(),
            messages: messages.to_vec(),
        };
        local_cache::write_encrypted_json(
            &self.paths.confirmed_sends_path(conversation_id),
            &self.key,
            &cache,
        )
    }

    fn try_persist_conversation_tab_metadata(
        &self,
        conversation: &SlackConversationSnapshot,
    ) -> Result<(), String> {
        let _write_guard = self
            .write_lock
            .lock()
            .map_err(|_| "Slack cache write mutex poisoned".to_string())?;
        let Some(mut cache) = self.load_cache(&conversation.conversation_id)? else {
            return Ok(());
        };
        let Some(cached_conversation) = cache.conversation.as_mut() else {
            return Ok(());
        };
        let mut tabs = conversation.tabs.clone();
        for tab in &mut tabs {
            let Some(existing) = cached_conversation
                .tabs
                .iter()
                .find(|existing| existing.id == tab.id)
            else {
                continue;
            };
            tab.merge_resolved_metadata_from(existing);
        }
        if tabs == cached_conversation.tabs {
            return Ok(());
        }
        cached_conversation.tabs = tabs;
        cache.fetched_at_unix_secs = fetched_at_unix_secs();
        local_cache::write_encrypted_json(
            &self.paths.workspace_path(&conversation.conversation_id),
            &self.key,
            &cache,
        )
    }
}

fn valid_confirmed_sends(messages: &[SlackMessage]) -> bool {
    if messages.len() > SLACK_CONFIRMED_SEND_CACHE_LIMIT {
        return false;
    }
    let mut previous = None;
    for message in messages {
        let Ok(timestamp) = slack_message_timestamp_key(&message.id) else {
            return false;
        };
        if previous.is_some_and(|previous| previous >= timestamp) {
            return false;
        }
        previous = Some(timestamp);
    }
    true
}

fn slack_message_timestamp_key(timestamp: &str) -> Result<(u64, u32), String> {
    crate::model::SlackMessageTimestamp::parse(timestamp).map(|timestamp| timestamp.sort_key())
}
