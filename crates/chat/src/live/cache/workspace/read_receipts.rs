use std::collections::HashMap;

use crate::model::{
    SlackConversationReadReceipt, SlackConversationSnapshot, SlackDmInboxSnapshot,
    SlackLastReadTimestamp, SlackSidebarSnapshot, SlackWorkspace,
};

use super::{
    SlackDmInboxCache, SlackReadReceiptsCache, SlackWorkspaceCache, SlackWorkspaceCacheStore,
    SLACK_DM_INBOX_CACHE_ITEM_LIMIT, SLACK_DM_INBOX_CACHE_SCHEMA_VERSION,
    SLACK_READ_RECEIPTS_CACHE_LIMIT, SLACK_READ_RECEIPTS_CACHE_SCHEMA_VERSION,
    SLACK_WORKSPACE_CACHE_SCHEMA_VERSION,
};

impl SlackWorkspaceCacheStore {
    pub fn persist_conversation_read_receipt(
        &self,
        receipt: &SlackConversationReadReceipt,
    ) -> Result<(), String> {
        if receipt.team_id != self.team_id || receipt.conversation_id.is_empty() {
            return Err("Slack read receipt did not match its cache workspace".to_string());
        }
        let _write_guard = self
            .write_lock
            .lock()
            .map_err(|_| "Slack cache write mutex poisoned".to_string())?;
        let mut receipts = self.load_read_receipts()?;
        if receipts.len() >= SLACK_READ_RECEIPTS_CACHE_LIMIT
            && !receipts.contains_key(&receipt.conversation_id)
        {
            return Err(format!(
                "Slack read receipt cache exceeded {SLACK_READ_RECEIPTS_CACHE_LIMIT} conversations"
            ));
        }
        let should_write = receipts.get(&receipt.conversation_id) != Some(&receipt.last_read);
        if should_write {
            receipts.insert(receipt.conversation_id.clone(), receipt.last_read.clone());
            self.write_read_receipts(&receipts)?;
        }
        self.reconcile_cached_workspace_read_receipts(&receipt.conversation_id, &receipts)?;
        self.reconcile_cached_dm_read_receipts(&receipts)
    }

    pub fn reconcile_sidebar_read_receipts(
        &self,
        snapshot: &mut SlackSidebarSnapshot,
    ) -> Result<(), String> {
        let receipts = self.load_read_receipts()?;
        apply_read_receipts_to_sidebar(Some(snapshot), &receipts);
        Ok(())
    }

    pub fn reconcile_conversation_read_receipts(
        &self,
        snapshot: &mut SlackConversationSnapshot,
    ) -> Result<(), String> {
        let receipts = self.load_read_receipts()?;
        apply_read_receipts_to_conversation(Some(snapshot), &receipts);
        Ok(())
    }

    pub fn reconcile_dm_inbox_read_receipts(
        &self,
        snapshot: &mut SlackDmInboxSnapshot,
    ) -> Result<(), String> {
        self.apply_read_receipts_to_dm_inbox(snapshot)
    }

    pub fn reconcile_workspace_read_receipts(
        &self,
        workspace: &mut SlackWorkspace,
    ) -> Result<(), String> {
        let receipts = self.load_read_receipts()?;
        for (conversation_id, last_read) in &receipts {
            workspace.apply_conversation_read_receipt(conversation_id, last_read, None);
        }
        Ok(())
    }

    pub(super) fn load_read_receipts(
        &self,
    ) -> Result<HashMap<String, SlackLastReadTimestamp>, String> {
        let Some(cache) = local_cache::read_encrypted_json::<SlackReadReceiptsCache>(
            &self.paths.read_receipts_path(),
            &self.key,
        )?
        else {
            return Ok(HashMap::new());
        };
        if cache.schema_version != SLACK_READ_RECEIPTS_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.receipts.len() > SLACK_READ_RECEIPTS_CACHE_LIMIT
            || cache
                .receipts
                .keys()
                .any(|conversation_id| conversation_id.is_empty())
        {
            return Ok(HashMap::new());
        }
        Ok(cache.receipts)
    }

    fn write_read_receipts(
        &self,
        receipts: &HashMap<String, SlackLastReadTimestamp>,
    ) -> Result<(), String> {
        local_cache::write_encrypted_json(
            &self.paths.read_receipts_path(),
            &self.key,
            &SlackReadReceiptsCache {
                schema_version: SLACK_READ_RECEIPTS_CACHE_SCHEMA_VERSION,
                team_id: self.team_id.clone(),
                receipts: receipts.clone(),
            },
        )
    }

    pub(super) fn apply_read_receipts_to_workspace_cache(
        &self,
        cache: &mut SlackWorkspaceCache,
    ) -> Result<(), String> {
        let receipts = self.load_read_receipts()?;
        apply_read_receipts_to_sidebar(cache.sidebar.as_mut(), &receipts);
        apply_read_receipts_to_conversation(cache.conversation.as_mut(), &receipts);
        Ok(())
    }

    pub(super) fn apply_read_receipts_to_dm_inbox(
        &self,
        snapshot: &mut SlackDmInboxSnapshot,
    ) -> Result<(), String> {
        let receipts = self.load_read_receipts()?;
        apply_read_receipts_to_dm_inbox(snapshot, &receipts);
        Ok(())
    }

    fn reconcile_cached_workspace_read_receipts(
        &self,
        conversation_id: &str,
        receipts: &HashMap<String, SlackLastReadTimestamp>,
    ) -> Result<(), String> {
        let Some(mut cache) = local_cache::read_encrypted_json::<SlackWorkspaceCache>(
            &self.paths.workspace_path(conversation_id),
            &self.key,
        )?
        else {
            return Ok(());
        };
        if cache.schema_version != SLACK_WORKSPACE_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.conversation_id != conversation_id
        {
            return Ok(());
        }
        apply_read_receipts_to_sidebar(cache.sidebar.as_mut(), receipts);
        apply_read_receipts_to_conversation(cache.conversation.as_mut(), receipts);
        local_cache::write_encrypted_json(
            &self.paths.workspace_path(conversation_id),
            &self.key,
            &cache,
        )
    }

    fn reconcile_cached_dm_read_receipts(
        &self,
        receipts: &HashMap<String, SlackLastReadTimestamp>,
    ) -> Result<(), String> {
        let Some(mut cache) = local_cache::read_encrypted_json::<SlackDmInboxCache>(
            &self.paths.dm_inbox_path(),
            &self.key,
        )?
        else {
            return Ok(());
        };
        if cache.schema_version != SLACK_DM_INBOX_CACHE_SCHEMA_VERSION
            || cache.team_id != self.team_id
            || cache.snapshot.team_id != self.team_id
            || cache.snapshot.items.len() > SLACK_DM_INBOX_CACHE_ITEM_LIMIT
        {
            return Ok(());
        }
        apply_read_receipts_to_dm_inbox(&mut cache.snapshot, receipts);
        local_cache::write_encrypted_json(&self.paths.dm_inbox_path(), &self.key, &cache)
    }
}

pub(super) fn apply_read_receipts_to_sidebar(
    snapshot: Option<&mut SlackSidebarSnapshot>,
    receipts: &HashMap<String, SlackLastReadTimestamp>,
) {
    let Some(snapshot) = snapshot else {
        return;
    };
    for (conversation_id, last_read) in receipts {
        snapshot.apply_conversation_read_receipt(conversation_id, last_read, None);
    }
}

pub(super) fn apply_read_receipts_to_conversation(
    snapshot: Option<&mut SlackConversationSnapshot>,
    receipts: &HashMap<String, SlackLastReadTimestamp>,
) {
    let Some(snapshot) = snapshot else {
        return;
    };
    if let Some(last_read) = receipts.get(&snapshot.conversation_id) {
        let conversation_id = snapshot.conversation_id.clone();
        snapshot.apply_conversation_read_receipt(&conversation_id, last_read);
    }
}

fn apply_read_receipts_to_dm_inbox(
    snapshot: &mut SlackDmInboxSnapshot,
    receipts: &HashMap<String, SlackLastReadTimestamp>,
) {
    for (conversation_id, last_read) in receipts {
        snapshot.apply_conversation_read_receipt(conversation_id, last_read);
    }
}
