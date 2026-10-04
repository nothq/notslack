mod directory;
mod history_window;

use std::{thread, time::Instant};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::SlackMessageTimestamp;
use crate::model::{SlackConversationHistoryCursor, SlackLastReadTimestamp};

use crate::live::{
    api::SlackApiClient,
    payload::{
        message::{merge_slack_conversation_history_pages, slack_conversation_history_page_state},
        util::string_at,
        SLACK_CONVERSATION_HISTORY_PAGE_SIZE,
    },
};

use super::{
    ops, CachedSlackConversationDetail, CachedSlackConversations, CachedSlackDndStatus,
    SlackChannelPayloads, SlackConversationCorePayloads, SlackLiveWorkspaceLoader, SlackSelfUser,
    WorkspacePreloadTasks, SLACK_CONVERSATION_DETAIL_CACHE_INTERVAL,
    SLACK_DND_STATUS_CACHE_INTERVAL,
};

pub(super) use history_window::SlackInitialHistoryWindow;

#[derive(Deserialize)]
struct SlackDndInfoPayload {
    #[serde(default)]
    dnd_enabled: bool,
    #[serde(default)]
    snooze_enabled: bool,
}

impl SlackLiveWorkspaceLoader {
    pub(super) fn load_conversation_history_page_payload(
        &self,
        conversation_id: &str,
        cursor: &SlackConversationHistoryCursor,
    ) -> Result<Value, String> {
        let conversation_id = conversation_id.to_string();
        let cursor = cursor.as_str().to_string();
        self.api.post(
            "conversations.history",
            &[
                ("channel", conversation_id),
                ("limit", SLACK_CONVERSATION_HISTORY_PAGE_SIZE.to_string()),
                ("include_all_metadata", "true".to_string()),
                ("cursor", cursor),
            ],
        )
    }

    pub(super) fn spawn_workspace_preload_tasks(&self) -> WorkspacePreloadTasks {
        WorkspacePreloadTasks {
            team_info: {
                let loader = self.clone();
                thread::spawn(move || loader.load_team_info())
            },
            self_user: {
                let loader = self.clone();
                thread::spawn(move || loader.load_self_user())
            },
        }
    }

    pub(super) fn load_channel_payloads(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<SlackChannelPayloads, String> {
        let detail = self.spawn_conversation_detail_request(conversation_id);
        let history = self.spawn_conversation_history_request(conversation_id, anchor_timestamp);
        let channel_info = ops::join_slack_request("conversations.info", detail)?;
        let dnd_status = direct_message_peer_user_id(&channel_info).map(|user_id| {
            let loader = self.clone();
            thread::spawn(move || loader.load_peer_notifications_paused(user_id.as_str()))
        });
        let history = ops::join_slack_request("conversations.history", history)?;
        let peer_notifications_paused = dnd_status
            .map(|request| {
                request
                    .join()
                    .map_err(|_| "Slack dnd.info request thread panicked".to_string())?
            })
            .transpose()?
            .unwrap_or(false);
        Ok(SlackChannelPayloads {
            channel_info,
            history,
            peer_notifications_paused,
        })
    }

    pub(super) fn load_conversation_core_payloads(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> Result<SlackConversationCorePayloads, String> {
        let detail = self.spawn_conversation_detail_request(conversation_id);
        let history = self.spawn_conversation_history_request(conversation_id, anchor_timestamp);
        let channel_info = ops::join_slack_request("conversations.info", detail)?;
        let history = ops::join_slack_request("conversations.history", history)?;
        Ok(SlackConversationCorePayloads {
            channel_info,
            history,
        })
    }

    pub(super) fn load_conversation_detail(&self, conversation_id: &str) -> Result<Value, String> {
        if let Some(channel) = self.cached_conversation_detail(conversation_id)? {
            return Ok(json!({
                "ok": true,
                "channel": channel,
            }));
        }
        let fetch_lock = self
            .conversation_detail_fetch_locks
            .lock()
            .map_err(|_| "slack conversation detail fetch locks mutex poisoned".to_string())?
            .entry(conversation_id.to_string())
            .or_insert_with(|| std::sync::Arc::new(std::sync::Mutex::new(())))
            .clone();
        let _fetch_guard = fetch_lock.lock().map_err(|_| {
            format!("slack conversation detail fetch mutex poisoned for {conversation_id}")
        })?;
        if let Some(channel) = self.cached_conversation_detail(conversation_id)? {
            return Ok(json!({
                "ok": true,
                "channel": channel,
            }));
        }
        let detail = self.api.post(
            "conversations.info",
            &[
                ("channel", conversation_id.to_string()),
                ("include_num_members", "true".to_string()),
            ],
        )?;
        self.store_conversation_detail(conversation_id, &detail)?;
        Ok(detail)
    }

    fn cached_conversation_detail(&self, conversation_id: &str) -> Result<Option<Value>, String> {
        Ok(self
            .conversation_detail_cache
            .lock()
            .map_err(|_| "slack conversation detail cache mutex poisoned".to_string())?
            .get(conversation_id)
            .filter(|cached| cached.loaded_at.elapsed() < SLACK_CONVERSATION_DETAIL_CACHE_INTERVAL)
            .map(|cached| cached.channel.clone()))
    }

    fn store_conversation_detail(
        &self,
        conversation_id: &str,
        detail: &Value,
    ) -> Result<(), String> {
        if let Some(channel) = detail.get("channel").cloned() {
            self.conversation_detail_cache
                .lock()
                .map_err(|_| "slack conversation detail cache mutex poisoned".to_string())?
                .insert(
                    conversation_id.to_string(),
                    CachedSlackConversationDetail {
                        channel,
                        loaded_at: Instant::now(),
                    },
                );
        }
        Ok(())
    }

    fn load_peer_notifications_paused(&self, user_id: &str) -> Result<bool, String> {
        if let Some(notifications_paused) = self.cached_peer_notifications_paused(user_id)? {
            return Ok(notifications_paused);
        }
        let fetch_lock = self
            .dnd_status_fetch_locks
            .lock()
            .map_err(|_| "slack dnd status fetch locks mutex poisoned".to_string())?
            .entry(user_id.to_string())
            .or_insert_with(|| std::sync::Arc::new(std::sync::Mutex::new(())))
            .clone();
        let _fetch_guard = fetch_lock
            .lock()
            .map_err(|_| format!("slack dnd status fetch mutex poisoned for {user_id}"))?;
        if let Some(notifications_paused) = self.cached_peer_notifications_paused(user_id)? {
            return Ok(notifications_paused);
        }
        let payload = self
            .api
            .post("dnd.info", &[("user", user_id.to_string())])?;
        let status = serde_json::from_value::<SlackDndInfoPayload>(payload)
            .map_err(|error| format!("failed to decode Slack dnd.info response: {error}"))?;
        let notifications_paused = status.dnd_enabled || status.snooze_enabled;
        self.dnd_status_cache
            .lock()
            .map_err(|_| "slack dnd status cache mutex poisoned".to_string())?
            .insert(
                user_id.to_string(),
                CachedSlackDndStatus {
                    notifications_paused,
                    loaded_at: Instant::now(),
                },
            );
        Ok(notifications_paused)
    }

    fn cached_peer_notifications_paused(&self, user_id: &str) -> Result<Option<bool>, String> {
        Ok(self
            .dnd_status_cache
            .lock()
            .map_err(|_| "slack dnd status cache mutex poisoned".to_string())?
            .get(user_id)
            .filter(|cached| cached.loaded_at.elapsed() < SLACK_DND_STATUS_CACHE_INTERVAL)
            .map(|cached| cached.notifications_paused))
    }

    fn load_team_info(&self) -> Result<Value, String> {
        if let Some(team_info) = self
            .team_info_cache
            .lock()
            .map_err(|_| "slack team info cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(team_info);
        }
        let team_info = self
            .api
            .post("team.info", &[("team", self.team_id.clone())])?;
        *self
            .team_info_cache
            .lock()
            .map_err(|_| "slack team info cache mutex poisoned".to_string())? =
            Some(team_info.clone());
        Ok(team_info)
    }

    pub(super) fn load_auth_test(&self) -> Result<Value, String> {
        if let Some(auth_test) = self
            .auth_test_cache
            .lock()
            .map_err(|_| "slack auth.test cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(auth_test);
        }
        let _fetch_guard = self
            .auth_test_fetch_lock
            .lock()
            .map_err(|_| "slack auth.test fetch mutex poisoned".to_string())?;
        if let Some(auth_test) = self
            .auth_test_cache
            .lock()
            .map_err(|_| "slack auth.test cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(auth_test);
        }
        let auth_test = self.api.post("auth.test", &[])?;
        *self
            .auth_test_cache
            .lock()
            .map_err(|_| "slack auth.test cache mutex poisoned".to_string())? =
            Some(auth_test.clone());
        Ok(auth_test)
    }

    pub(super) fn load_self_user_id(&self) -> Result<String, String> {
        string_at(&self.load_auth_test()?, &["user_id"])
            .ok_or_else(|| "Slack auth.test did not return user_id".to_string())
    }

    pub(super) fn load_self_user(&self) -> Result<SlackSelfUser, String> {
        let user_id = self.load_self_user_id()?;
        if let Some(user) = self
            .user_cache
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?
            .get(&user_id)
            .cloned()
        {
            return Ok((user_id, user));
        }
        let _fetch_guard = self
            .user_fetch_lock
            .lock()
            .map_err(|_| "slack user fetch mutex poisoned".to_string())?;
        if let Some(user) = self
            .user_cache
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?
            .get(&user_id)
            .cloned()
        {
            return Ok((user_id, user));
        }
        let payload = self.api.post("users.info", &[("user", user_id.clone())])?;
        let user = payload.get("user").cloned().ok_or_else(|| {
            format!("Slack users.info response missing user payload for {user_id}")
        })?;
        self.user_cache.insert(user_id.clone(), user.clone())?;
        Ok((user_id, user))
    }

    fn spawn_slack_request<F>(&self, request: F) -> thread::JoinHandle<Result<Value, String>>
    where
        F: FnOnce(SlackApiClient) -> Result<Value, String> + Send + 'static,
    {
        let api = self.api.clone();
        thread::spawn(move || request(api))
    }

    fn spawn_conversation_detail_request(
        &self,
        conversation_id: &str,
    ) -> thread::JoinHandle<Result<Value, String>> {
        let loader = self.clone();
        let conversation_id = conversation_id.to_string();
        thread::spawn(move || loader.load_conversation_detail(conversation_id.as_str()))
    }

    fn spawn_conversation_history_request(
        &self,
        conversation_id: &str,
        anchor_timestamp: Option<&str>,
    ) -> thread::JoinHandle<Result<Value, String>> {
        let conversation_id = conversation_id.to_string();
        let anchor_timestamp = anchor_timestamp.map(str::to_string);
        self.spawn_slack_request(move |api| {
            let mut params = vec![
                ("channel", conversation_id),
                ("limit", SLACK_CONVERSATION_HISTORY_PAGE_SIZE.to_string()),
                ("include_all_metadata", "true".to_string()),
            ];
            if let Some(anchor_timestamp) = anchor_timestamp {
                params.extend([
                    ("latest", anchor_timestamp),
                    ("inclusive", "true".to_string()),
                ]);
            }
            api.post("conversations.history", &params)
        })
    }
}

fn direct_message_peer_user_id(channel_info: &Value) -> Option<String> {
    let channel = channel_info.get("channel")?;
    (channel.get("is_im").and_then(Value::as_bool) == Some(true))
        .then(|| string_at(channel, &["user"]))
        .flatten()
        .filter(|user_id| !user_id.is_empty())
}
