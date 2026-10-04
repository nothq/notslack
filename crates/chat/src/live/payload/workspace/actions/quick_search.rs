use std::sync::{Arc, Mutex, Weak};

use serde_json::Value;

use crate::model::{
    SlackQuickMessageQuery, SlackQuickMessageQueryScopeRef, SlackQuickSearchMessage,
    SlackQuickSearchSnapshot,
};

use super::super::{
    enrich_slack_quick_search_messages, finish_slack_quick_search_snapshot,
    load_slack_quick_search_bots, load_slack_search_users_with_cache,
    resolve_slack_quick_message_request, slack_quick_search_bot_ids,
    slack_quick_search_conversation_user_ids, slack_quick_search_user_ids,
    CachedSlackQuickMessages, SlackLiveWorkspaceLoader, SlackQuickSearchDirectory,
    SLACK_QUICK_MESSAGE_CACHE_INTERVAL, SLACK_QUICK_MESSAGE_CACHE_LIMIT,
};

type ResolvedSlackQuickMessageRequest = (
    Option<crate::live::api::SlackQuickMessageRequest>,
    Option<Value>,
);

impl SlackLiveWorkspaceLoader {
    pub fn search_quick_switch(
        &self,
        query: &str,
        _recent_channels: &[String],
    ) -> Result<SlackQuickSearchSnapshot, String> {
        let snapshot = self.api.search_quick_switch(query)?;
        Ok(finish_slack_quick_search_snapshot(snapshot, Vec::new()))
    }

    pub fn search_quick_messages(
        &self,
        query: &SlackQuickMessageQuery,
        recent_channels: &[String],
    ) -> Result<Vec<SlackQuickSearchMessage>, String> {
        let (request, conversations) =
            self.resolve_quick_message_request(query, recent_channels)?;
        let Some(request) = request else {
            return Ok(Vec::new());
        };
        let key = request.cache_key();
        if let Some(messages) = self.cached_quick_messages(&key)? {
            return Ok(messages);
        }
        let fetch_lock = self.quick_message_fetch_lock(&key)?;
        let _fetch_guard = fetch_lock
            .lock()
            .map_err(|_| "Slack quick-message fetch mutex poisoned".to_string())?;
        if let Some(messages) = self.cached_quick_messages(&key)? {
            return Ok(messages);
        }
        let messages = self.load_uncached_quick_messages(&request, conversations)?;
        self.cache_quick_messages(key, messages.clone())?;
        Ok(messages)
    }

    fn resolve_quick_message_request(
        &self,
        query: &SlackQuickMessageQuery,
        recent_channels: &[String],
    ) -> Result<ResolvedSlackQuickMessageRequest, String> {
        let conversations = matches!(query.scope(), SlackQuickMessageQueryScopeRef::User(_))
            .then(|| self.load_conversations())
            .transpose()?;
        let request =
            resolve_slack_quick_message_request(query, recent_channels, conversations.as_ref())?;
        Ok((request, conversations))
    }

    fn load_uncached_quick_messages(
        &self,
        request: &crate::live::api::SlackQuickMessageRequest,
        conversations: Option<Value>,
    ) -> Result<Vec<SlackQuickSearchMessage>, String> {
        let references = self.api.search_quick_message_references(request)?;
        if references.is_empty() {
            return Ok(Vec::new());
        }
        let conversations = conversations.map_or_else(|| self.load_conversations(), Ok)?;
        let mut user_ids = slack_quick_search_user_ids(&references);
        user_ids.extend(slack_quick_search_conversation_user_ids(
            &references,
            &conversations,
        ));
        let users = load_slack_search_users_with_cache(
            &self.api,
            user_ids,
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let bots = load_slack_quick_search_bots(
            &self.api,
            slack_quick_search_bot_ids(&references),
            &self.bot_cache,
            &self.bot_fetch_lock,
        )?;
        let sidebar = self.load_sidebar_snapshot()?;
        Ok(enrich_slack_quick_search_messages(
            &self.team_id,
            references,
            &SlackQuickSearchDirectory {
                users: &users,
                bots: &bots,
                sidebar: &sidebar,
            },
            &conversations,
        ))
    }

    fn quick_message_fetch_lock(&self, key: &str) -> Result<Arc<Mutex<()>>, String> {
        let mut locks = self
            .quick_message_fetch_locks
            .lock()
            .map_err(|_| "Slack quick-message fetch-lock map mutex poisoned".to_string())?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(key.to_string(), Arc::downgrade(&lock));
        Ok(lock)
    }

    fn cached_quick_messages(
        &self,
        key: &str,
    ) -> Result<Option<Vec<SlackQuickSearchMessage>>, String> {
        let mut cache = self
            .quick_message_cache
            .lock()
            .map_err(|_| "Slack quick-message cache mutex poisoned".to_string())?;
        cache.retain(|entry| entry.loaded_at.elapsed() < SLACK_QUICK_MESSAGE_CACHE_INTERVAL);
        let Some(index) = cache.iter().position(|entry| entry.key == key) else {
            return Ok(None);
        };
        let entry = cache
            .remove(index)
            .expect("located Slack quick-message cache entry must exist");
        let messages = entry.messages.clone();
        cache.push_back(entry);
        Ok(Some(messages))
    }

    fn cache_quick_messages(
        &self,
        key: String,
        messages: Vec<SlackQuickSearchMessage>,
    ) -> Result<(), String> {
        let mut cache = self
            .quick_message_cache
            .lock()
            .map_err(|_| "Slack quick-message cache mutex poisoned".to_string())?;
        cache.retain(|entry| entry.key != key);
        if cache.len() == SLACK_QUICK_MESSAGE_CACHE_LIMIT {
            cache.pop_front();
        }
        cache.push_back(CachedSlackQuickMessages {
            key,
            messages,
            loaded_at: std::time::Instant::now(),
        });
        Ok(())
    }
}
