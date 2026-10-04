use crate::live::SlackApiClient;
use serde_json::Value;

pub(crate) fn resolve_default_conversation_id(
    api: &SlackApiClient,
    preferred_conversation_id: Option<&str>,
) -> Result<String, String> {
    if let Some(preferred_conversation_id) = preferred_conversation_id
        .map(str::trim)
        .filter(|conversation_id| !conversation_id.is_empty())
    {
        return Ok(preferred_conversation_id.to_string());
    }
    let conversations = api.post(
        "users.conversations",
        &[
            (
                "types",
                "public_channel,private_channel,im,mpim".to_string(),
            ),
            ("exclude_archived", "true".to_string()),
            ("limit", "999".to_string()),
        ],
    )?;
    let channels = conversations
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack users.conversations response was missing channels".to_string())?;
    channels
        .iter()
        .filter_map(conversation_id)
        .min_by_key(|conversation_id| conversation_priority(channels, conversation_id))
        .map(str::to_string)
        .ok_or_else(|| {
            "failed to resolve a default Slack conversation from users.conversations".to_string()
        })
}

fn conversation_priority(channels: &[Value], target_id: &&str) -> usize {
    channels
        .iter()
        .find(|conversation| conversation_id(conversation) == Some(*target_id))
        .map(|conversation| {
            if conversation
                .get("is_channel")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || conversation
                    .get("is_group")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                0
            } else if conversation
                .get("is_im")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || conversation
                    .get("is_mpim")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                1
            } else {
                2
            }
        })
        .unwrap_or(usize::MAX)
}

fn conversation_id(conversation: &Value) -> Option<&str> {
    conversation
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
}
