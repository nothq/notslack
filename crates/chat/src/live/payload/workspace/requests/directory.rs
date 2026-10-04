use serde_json::{json, Value};

use super::{ops, string_at, CachedSlackConversations, SlackLiveWorkspaceLoader};

impl SlackLiveWorkspaceLoader {
    pub(in crate::live::payload::workspace) fn load_workspace_conversations(
        &self,
        conversation_id: &str,
    ) -> Result<Value, String> {
        let mut conversations = self.load_conversations()?;
        if !ops::conversations_contain_id(&conversations, conversation_id) {
            conversations = self.refresh_conversations()?;
        }
        Ok(conversations)
    }

    pub(in crate::live::payload::workspace) fn load_conversations(&self) -> Result<Value, String> {
        if let Some(cached) = self
            .conversations_cache
            .lock()
            .map_err(|_| "slack conversations cache mutex poisoned".to_string())?
            .as_ref()
            .map(|cached| cached.payload.clone())
        {
            return Ok(cached);
        }
        self.refresh_conversations()
    }

    fn refresh_conversations(&self) -> Result<Value, String> {
        let mut channels = self.load_conversation_channels(
            "users.conversations",
            "public_channel,private_channel,im,mpim",
        )?;
        merge_slack_conversation_channels(
            &mut channels,
            self.load_conversation_channels("conversations.list", "im,mpim")?,
        );
        let conversations = json!({
            "ok": true,
            "channels": channels,
        });
        *self
            .conversations_cache
            .lock()
            .map_err(|_| "slack conversations cache mutex poisoned".to_string())? =
            Some(CachedSlackConversations {
                payload: conversations.clone(),
            });
        Ok(conversations)
    }

    fn load_conversation_channels(&self, method: &str, types: &str) -> Result<Vec<Value>, String> {
        let mut channels = Vec::new();
        let mut cursor = None;
        loop {
            let mut params = vec![
                ("types", types.to_string()),
                ("exclude_archived", "false".to_string()),
                ("limit", "999".to_string()),
            ];
            if let Some(next_cursor) = cursor.take() {
                params.push(("cursor", next_cursor));
            }
            let page = self.api.post(method, &params)?;
            channels.extend(
                page.get("channels")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
            cursor = string_at(&page, &["response_metadata", "next_cursor"])
                .filter(|cursor| !cursor.is_empty());
            if cursor.is_none() {
                break;
            }
        }
        Ok(channels)
    }
}

fn merge_slack_conversation_channels(channels: &mut Vec<Value>, updates: Vec<Value>) {
    for update in updates {
        let Some(update_id) = string_at(&update, &["id"]) else {
            continue;
        };
        if let Some(existing) = channels
            .iter_mut()
            .find(|channel| string_at(channel, &["id"]).as_deref() == Some(update_id.as_str()))
        {
            merge_slack_conversation_detail(existing, update);
        } else {
            channels.push(update);
        }
    }
}

fn merge_slack_conversation_detail(channel: &mut Value, detail_channel: Value) {
    let (Some(channel_object), Some(detail_object)) =
        (channel.as_object_mut(), detail_channel.as_object())
    else {
        return;
    };
    for (key, value) in detail_object {
        if !value.is_null() {
            channel_object.insert(key.clone(), value.clone());
        }
    }
}
