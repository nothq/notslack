use crate::live::SlackApiClient;
use crate::model::ResolvedSlackChannel;
use serde_json::Value;

const SLACK_CONVERSATION_TYPES: &str = "public_channel,private_channel,im,mpim";

pub(crate) fn resolve_slack_channel_by_name(
    api: &SlackApiClient,
    team_id: &str,
    channel_name: &str,
) -> Result<ResolvedSlackChannel, String> {
    let normalized = normalize_channel_name(channel_name)?;
    let conversations = api.post(
        "users.conversations",
        &[
            ("types", SLACK_CONVERSATION_TYPES.to_string()),
            ("exclude_archived", "true".to_string()),
            ("limit", "999".to_string()),
        ],
    )?;
    let channels = conversations
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack users.conversations response was missing channels".to_string())?;
    let matches = channels
        .iter()
        .filter_map(|channel| matching_channel(channel, normalized.as_str()))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [channel] => Ok(channel.with_team(team_id)),
        [] => Err(format!("Slack channel #{normalized} was not found")),
        _ => Err(format!(
            "Slack channel #{normalized} was ambiguous: {}",
            matches
                .iter()
                .map(|channel| channel.channel_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn normalize_channel_name(channel_name: &str) -> Result<String, String> {
    let normalized = channel_name.trim().trim_start_matches('#').to_string();
    if normalized.is_empty() {
        return Err("Slack channel name must not be empty".to_string());
    }
    Ok(normalized)
}

fn matching_channel(channel: &Value, normalized_name: &str) -> Option<PartialSlackChannel> {
    let name = channel
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| *name == normalized_name)?;
    let channel_id = channel
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    Some(PartialSlackChannel {
        channel_id: channel_id.to_string(),
        channel_name: name.to_string(),
    })
}

struct PartialSlackChannel {
    channel_id: String,
    channel_name: String,
}

impl PartialSlackChannel {
    fn with_team(&self, team_id: &str) -> ResolvedSlackChannel {
        ResolvedSlackChannel {
            team_id: team_id.to_string(),
            channel_id: self.channel_id.clone(),
            channel_name: self.channel_name.clone(),
            web_url: format!("https://app.slack.com/client/{team_id}/{}", self.channel_id),
            desktop_url: format!("slack://channel?team={team_id}&id={}", self.channel_id),
        }
    }
}
