use crate::model::{SlackConversationTab, SlackConversationTabTarget};
use serde_json::{Map, Value};

pub(super) fn slack_conversation_tabs(
    channel: &Value,
    conversation_id: &str,
) -> Result<Vec<SlackConversationTab>, String> {
    let Some(tabs) = channel.pointer("/properties/tabs") else {
        return Ok(Vec::new());
    };
    if tabs.is_null() {
        return Ok(Vec::new());
    }
    let tabs = tabs.as_array().ok_or_else(|| {
        format!("Slack conversations.info returned non-array tabs for {conversation_id}")
    })?;
    tabs.iter()
        .enumerate()
        .map(|(index, tab)| slack_conversation_tab(tab, conversation_id, index))
        .collect()
}

fn slack_conversation_tab(
    tab: &Value,
    conversation_id: &str,
    index: usize,
) -> Result<SlackConversationTab, String> {
    let tab = tab.as_object().ok_or_else(|| {
        format!("Slack conversations.info returned non-object tab {index} for {conversation_id}")
    })?;
    let id = required_nonempty_tab_string(tab.get("id"), conversation_id, index, "id")?;
    let source_type =
        required_nonempty_tab_string(tab.get("type"), conversation_id, index, "type")?;
    let label =
        optional_tab_string(tab.get("label"), conversation_id, index, "label")?.unwrap_or_default();
    let is_disabled = optional_tab_bool(
        tab.get("is_disabled"),
        conversation_id,
        index,
        "is_disabled",
    )?
    .unwrap_or(false);
    let target =
        slack_conversation_tab_target(tab, conversation_id, index, source_type, is_disabled)?;
    let mut tab = SlackConversationTab {
        id,
        label,
        target,
        is_disabled,
    };
    super::super::tabs::normalize_conversation_tab_label(&mut tab);
    Ok(tab)
}

fn slack_conversation_tab_target(
    tab: &Map<String, Value>,
    conversation_id: &str,
    index: usize,
    source_type: String,
    is_disabled: bool,
) -> Result<SlackConversationTabTarget, String> {
    if is_disabled {
        return Ok(SlackConversationTabTarget::Unsupported { source_type });
    }
    let target = match source_type.as_str() {
        "canvas" | "channel_canvas" => {
            let data = required_tab_object(tab.get("data"), conversation_id, index, &source_type)?;
            SlackConversationTabTarget::Canvas {
                file_id: required_nonempty_tab_string(
                    data.get("file_id"),
                    conversation_id,
                    index,
                    "data.file_id",
                )?,
                shared_timestamp: optional_nonempty_tab_string(
                    data.get("shared_ts"),
                    conversation_id,
                    index,
                    "data.shared_ts",
                )?,
                title: None,
                permalink: None,
            }
        }
        "folder" => {
            let data = required_tab_object(tab.get("data"), conversation_id, index, &source_type)?;
            SlackConversationTabTarget::Folder {
                bookmark_id: required_nonempty_tab_string(
                    data.get("folder_bookmark_id"),
                    conversation_id,
                    index,
                    "data.folder_bookmark_id",
                )?,
            }
        }
        "files" => SlackConversationTabTarget::Files,
        "bookmarks" | "pins" => SlackConversationTabTarget::Pins,
        _ => SlackConversationTabTarget::Unsupported { source_type },
    };
    Ok(target)
}

fn required_tab_object<'a>(
    value: Option<&'a Value>,
    conversation_id: &str,
    index: usize,
    source_type: &str,
) -> Result<&'a Map<String, Value>, String> {
    value.and_then(Value::as_object).ok_or_else(|| {
        format!(
            "Slack conversations.info returned {source_type} tab {index} without object data for {conversation_id}"
        )
    })
}

fn required_nonempty_tab_string(
    value: Option<&Value>,
    conversation_id: &str,
    index: usize,
    field: &str,
) -> Result<String, String> {
    optional_nonempty_tab_string(value, conversation_id, index, field)?.ok_or_else(|| {
        format!(
            "Slack conversations.info returned tab {index} without {field} for {conversation_id}"
        )
    })
}

fn optional_nonempty_tab_string(
    value: Option<&Value>,
    conversation_id: &str,
    index: usize,
    field: &str,
) -> Result<Option<String>, String> {
    optional_tab_string(value, conversation_id, index, field)?.map_or(Ok(None), |value| {
        if value.trim().is_empty() {
            Err(format!(
                "Slack conversations.info returned empty tab {index} {field} for {conversation_id}"
            ))
        } else {
            Ok(Some(value))
        }
    })
}

fn optional_tab_string(
    value: Option<&Value>,
    conversation_id: &str,
    index: usize,
    field: &str,
) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| {
            format!(
            "Slack conversations.info returned non-string tab {index} {field} for {conversation_id}"
        )
        })
}

fn optional_tab_bool(
    value: Option<&Value>,
    conversation_id: &str,
    index: usize,
    field: &str,
) -> Result<Option<bool>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value.as_bool().map(Some).ok_or_else(|| {
        format!(
            "Slack conversations.info returned non-boolean tab {index} {field} for {conversation_id}"
        )
    })
}
