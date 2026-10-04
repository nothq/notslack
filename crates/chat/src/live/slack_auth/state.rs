use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Default)]
pub(super) struct SlackSelectionState {
    active_team_id: Option<String>,
    conversation_id_by_team: BTreeMap<String, String>,
}

impl SlackSelectionState {
    pub(super) fn active_team_id(&self) -> Option<&str> {
        self.active_team_id.as_deref()
    }

    pub(super) fn conversation_id(&self, team_id: &str) -> Option<&str> {
        self.conversation_id_by_team
            .get(team_id)
            .map(String::as_str)
    }
}

pub(super) fn slack_selection() -> Result<SlackSelectionState, String> {
    let runtime_root = app_model::app_data_dir()?;
    let state = local_cache::load_runtime_state_json(&runtime_root)?;
    let root = state
        .as_object()
        .ok_or_else(|| "notslack runtime state root must be a JSON object".to_string())?;
    selection_from_state(root)
}

pub(super) fn store_slack_active_team(team_id: &str) -> Result<(), String> {
    let team_id = required_selection_field(team_id, "team id")?;
    let runtime_root = app_model::app_data_dir()?;
    local_cache::mutate_runtime_state_json(&runtime_root, |root| {
        let selection = selection_from_state(root)?;
        let active_conversation_id = selection.conversation_id(&team_id).map(str::to_string);
        let slack = root
            .entry("slack")
            .or_insert_with(|| Value::Object(Map::new()));
        let slack = slack
            .as_object_mut()
            .ok_or_else(|| "notslack runtime state slack entry must be a JSON object".to_string())?;
        slack.insert("last_team_id".to_string(), Value::String(team_id));
        match active_conversation_id {
            Some(conversation_id) => {
                slack.insert(
                    "last_conversation_id".to_string(),
                    Value::String(conversation_id),
                );
            }
            None => {
                slack.remove("last_conversation_id");
            }
        }
        Ok(())
    })
}

pub(super) fn store_slack_team_conversation(
    team_id: &str,
    conversation_id: &str,
) -> Result<(), String> {
    let team_id = required_selection_field(team_id, "team id")?;
    let conversation_id = required_selection_field(conversation_id, "conversation id")?;
    let runtime_root = app_model::app_data_dir()?;
    local_cache::mutate_runtime_state_json(&runtime_root, |root| {
        let mut selection = selection_from_state(root)?;
        selection
            .conversation_id_by_team
            .insert(team_id.clone(), conversation_id.clone());
        let team_is_active = selection.active_team_id.as_ref() == Some(&team_id);

        let slack = root
            .entry("slack")
            .or_insert_with(|| Value::Object(Map::new()));
        let slack = slack
            .as_object_mut()
            .ok_or_else(|| "notslack runtime state slack entry must be a JSON object".to_string())?;
        if team_is_active {
            slack.insert(
                "last_conversation_id".to_string(),
                Value::String(conversation_id),
            );
        }
        slack.insert(
            "last_conversation_id_by_team".to_string(),
            Value::Object(
                selection
                    .conversation_id_by_team
                    .into_iter()
                    .map(|(team_id, conversation_id)| (team_id, Value::String(conversation_id)))
                    .collect(),
            ),
        );
        Ok(())
    })
}

fn selection_from_state(root: &Map<String, Value>) -> Result<SlackSelectionState, String> {
    let Some(slack_value) = root.get("slack") else {
        return Ok(SlackSelectionState::default());
    };
    let slack = slack_value
        .as_object()
        .ok_or_else(|| "notslack runtime state slack entry must be a JSON object".to_string())?;
    let active_team_id = optional_selection_field(slack, "last_team_id")?;
    let legacy_conversation_id = optional_selection_field(slack, "last_conversation_id")?;
    if active_team_id.is_none() && legacy_conversation_id.is_some() {
        return Err(
            "notslack runtime state has a legacy Slack conversation without an active team".to_string(),
        );
    }

    let mut conversation_id_by_team = BTreeMap::new();
    if let Some(by_team_value) = slack.get("last_conversation_id_by_team") {
        let by_team = by_team_value.as_object().ok_or_else(|| {
            "notslack runtime state Slack conversation map must be a JSON object".to_string()
        })?;
        for (team_id, conversation_id) in by_team {
            let team_id = required_selection_field(team_id, "conversation map team id")?;
            let conversation_id = conversation_id.as_str().ok_or_else(|| {
                format!("notslack runtime state Slack conversation for team {team_id} must be a string")
            })?;
            let conversation_id =
                required_selection_field(conversation_id, "conversation map conversation id")?;
            conversation_id_by_team.insert(team_id, conversation_id);
        }
    }
    if let (Some(team_id), Some(conversation_id)) =
        (active_team_id.as_ref(), legacy_conversation_id)
    {
        conversation_id_by_team
            .entry(team_id.clone())
            .or_insert(conversation_id);
    }
    Ok(SlackSelectionState {
        active_team_id,
        conversation_id_by_team,
    })
}

fn optional_selection_field(
    state: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    let Some(value) = state.get(field) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("notslack runtime state Slack {field} must be a string"))?;
    required_selection_field(value, field).map(Some)
}

fn required_selection_field(value: &str, field: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(format!(
            "notslack runtime state Slack {field} must not be empty"
        ));
    }
    Ok(normalized.to_string())
}
