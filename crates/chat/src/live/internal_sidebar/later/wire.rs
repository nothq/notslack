use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use super::{LATER_PAGE_SIZE, SAVED_LIST};
use crate::live::payload::slack_rich_text_body_from_blocks;
use crate::model::{
    SlackLaterCounts, SlackLaterDates, SlackLaterFileReference, SlackLaterFilter, SlackLaterItem,
    SlackLaterItemKey, SlackLaterMessageReference, SlackLaterReferenceContent, SlackLaterReminder,
    SlackLaterSnapshot, SlackLaterState, SlackLaterTombstoneKind, SlackMessageTimestamp,
    SlackReminderId,
};

pub(super) fn decode_saved_mutation(method: &str, body: &str) -> Result<(), String> {
    let response = serde_json::from_str::<SlackSavedMutationResponse>(body)
        .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
    if response.ok {
        Ok(())
    } else {
        Err(format!(
            "Slack {method} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ))
    }
}

pub(super) fn decode_saved_list(
    filter: SlackLaterFilter,
    body: &str,
) -> Result<SlackLaterSnapshot, String> {
    let response = serde_json::from_str::<SlackSavedListResponse>(body)
        .map_err(|error| format!("failed to decode Slack {SAVED_LIST} response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "Slack {SAVED_LIST} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    let saved_items = response
        .saved_items
        .ok_or_else(|| format!("Slack {SAVED_LIST} response omitted saved_items"))?;
    if saved_items.len() > LATER_PAGE_SIZE {
        return Err(format!(
            "Slack {SAVED_LIST} returned {} items for a {LATER_PAGE_SIZE}-item page",
            saved_items.len()
        ));
    }
    let counts = response
        .counts
        .ok_or_else(|| format!("Slack {SAVED_LIST} response omitted counts"))?
        .into();
    let response_metadata = response
        .response_metadata
        .ok_or_else(|| format!("Slack {SAVED_LIST} response omitted response_metadata"))?;
    let next_cursor = if response_metadata.next_cursor.is_empty() {
        None
    } else {
        Some(crate::model::SlackLaterCursor::new(
            response_metadata.next_cursor,
        )?)
    };
    let items = saved_items
        .into_iter()
        .map(|item| parse_later_item(filter, item))
        .collect::<Result<Vec<_>, _>>()?;
    reject_duplicate_item_keys(&items)?;
    Ok(SlackLaterSnapshot {
        filter,
        items,
        counts,
        next_cursor,
    })
}

fn reject_duplicate_item_keys(items: &[SlackLaterItem]) -> Result<(), String> {
    let mut item_keys = HashSet::with_capacity(items.len());
    let duplicate = items
        .iter()
        .map(|item| item.key.clone())
        .find(|key| !item_keys.insert(key.clone()));
    let Some(duplicate) = duplicate else {
        return Ok(());
    };
    Err(format!(
        "Slack {SAVED_LIST} returned duplicate saved item {}",
        duplicate.as_str()
    ))
}

fn parse_later_item(
    filter: SlackLaterFilter,
    item: SlackSavedWireItem,
) -> Result<SlackLaterItem, String> {
    if item.date_created == 0 || item.date_updated == 0 {
        return Err(format!(
            "Slack {SAVED_LIST} returned zero date_created/date_updated for {}",
            item.item_id
        ));
    }
    let state = parse_later_state(
        filter,
        item.state.as_str(),
        item.todo_state.as_str(),
        item.is_archived.unwrap_or(false),
        item.date_completed,
    )?;
    let key_item_type = item.item_type.clone();
    let key_timestamp = item.ts.clone();
    let content = parse_later_reference_content(
        item.item_type,
        item.item_id.clone(),
        item.ts,
        item.is_tombstoned.unwrap_or(false),
        item.description,
    )?;
    let key = later_item_key(&key_item_type, &item.item_id, key_timestamp.as_deref())?;
    Ok(SlackLaterItem {
        key,
        item_id: item.item_id,
        state,
        dates: SlackLaterDates {
            created: item.date_created,
            updated: item.date_updated,
            due: item.date_due,
            snoozed_until: item.date_snoozed_until,
            completed: item.date_completed,
        },
        content,
    })
}

fn parse_later_state(
    filter: SlackLaterFilter,
    state: &str,
    todo_state: &str,
    archived: bool,
    completed_at: u64,
) -> Result<SlackLaterState, String> {
    let parsed = match state {
        "in_progress" if todo_state == "saved" && !archived && completed_at == 0 => {
            SlackLaterState::InProgress
        }
        "archived" if todo_state == "saved" && archived && completed_at == 0 => {
            SlackLaterState::Archived
        }
        "completed" if !archived && todo_state == "completed" && completed_at > 0 => {
            SlackLaterState::Completed
        }
        _ => {
            return Err(format!(
                "Slack {SAVED_LIST} returned inconsistent state={state}, todo_state={todo_state}, is_archived={archived}, date_completed={completed_at}"
            ));
        }
    };
    let expected = match filter {
        SlackLaterFilter::Saved => SlackLaterState::InProgress,
        SlackLaterFilter::Archived => SlackLaterState::Archived,
        SlackLaterFilter::Completed => SlackLaterState::Completed,
    };
    if parsed != expected {
        return Err(format!(
            "Slack {SAVED_LIST} returned {state} item for {} filter",
            filter.as_api_value()
        ));
    }
    Ok(parsed)
}

fn parse_later_reference_content(
    kind: String,
    item_id: String,
    timestamp: Option<String>,
    tombstoned: bool,
    description: Option<String>,
) -> Result<SlackLaterReferenceContent, String> {
    match kind.as_str() {
        "message" => parse_later_message_reference(item_id, timestamp, tombstoned),
        "file" => parse_later_file_reference(item_id, timestamp, tombstoned),
        "reminder" => parse_later_reminder_reference(item_id, timestamp, tombstoned, description),
        _ => Ok(SlackLaterReferenceContent::Unsupported { kind, item_id }),
    }
}

fn parse_later_message_reference(
    item_id: String,
    timestamp: Option<String>,
    tombstoned: bool,
) -> Result<SlackLaterReferenceContent, String> {
    require_slack_id(&item_id, &['C', 'D', 'G'], "message conversation")?;
    let timestamp =
        timestamp.ok_or_else(|| format!("Slack {SAVED_LIST} message {item_id} omitted ts"))?;
    SlackMessageTimestamp::parse(&timestamp)?;
    Ok(if tombstoned {
        SlackLaterReferenceContent::Tombstone {
            kind: SlackLaterTombstoneKind::Message,
        }
    } else {
        SlackLaterReferenceContent::Message(SlackLaterMessageReference {
            conversation_id: item_id,
            timestamp,
        })
    })
}

fn parse_later_file_reference(
    item_id: String,
    timestamp: Option<String>,
    tombstoned: bool,
) -> Result<SlackLaterReferenceContent, String> {
    require_absent_timestamp(&item_id, timestamp.as_deref())?;
    require_slack_id(&item_id, &['F'], "file")?;
    Ok(if tombstoned {
        SlackLaterReferenceContent::Tombstone {
            kind: SlackLaterTombstoneKind::File,
        }
    } else {
        SlackLaterReferenceContent::File(SlackLaterFileReference { file_id: item_id })
    })
}

fn parse_later_reminder_reference(
    item_id: String,
    timestamp: Option<String>,
    tombstoned: bool,
    description: Option<String>,
) -> Result<SlackLaterReferenceContent, String> {
    require_absent_timestamp(&item_id, timestamp.as_deref())?;
    let reminder_id = SlackReminderId::parse(item_id)?;
    Ok(if tombstoned {
        SlackLaterReferenceContent::Tombstone {
            kind: SlackLaterTombstoneKind::Reminder,
        }
    } else {
        let description = description
            .ok_or_else(|| format!("Slack {SAVED_LIST} reminder omitted description"))?;
        let description = decode_reminder_description(&description)?;
        SlackLaterReferenceContent::Reminder(SlackLaterReminder::new(reminder_id, description)?)
    })
}

fn decode_reminder_description(description: &str) -> Result<String, String> {
    let blocks = serde_json::from_str::<Vec<Value>>(description)
        .map_err(|error| format!("failed to decode Slack reminder description: {error}"))?;
    let users = HashMap::new();
    let body = slack_rich_text_body_from_blocks(&blocks, &users, None)
        .ok_or_else(|| "Slack reminder description omitted rich-text content".to_string())?;
    Ok(body.plain_text())
}

fn later_item_key(
    item_type: &str,
    item_id: &str,
    timestamp: Option<&str>,
) -> Result<SlackLaterItemKey, String> {
    let value = match item_type {
        "message" => format!(
            "message:{item_id}:{}",
            timestamp.ok_or_else(|| {
                format!("Slack {SAVED_LIST} message {item_id} omitted ts while building its key")
            })?
        ),
        "file" => format!("file:{item_id}"),
        "reminder" => format!("reminder:{item_id}"),
        kind => format!("unsupported:{}:{kind}:{item_id}", kind.len()),
    };
    SlackLaterItemKey::new(value)
}

fn require_absent_timestamp(item_id: &str, timestamp: Option<&str>) -> Result<(), String> {
    if timestamp.is_some() {
        return Err(format!(
            "Slack {SAVED_LIST} non-message item {item_id} unexpectedly included ts"
        ));
    }
    Ok(())
}

fn require_slack_id(item_id: &str, prefixes: &[char], label: &str) -> Result<(), String> {
    if item_id.len() < 2
        || !prefixes.iter().any(|prefix| item_id.starts_with(*prefix))
        || !item_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(format!(
            "Slack {SAVED_LIST} returned malformed {label} item_id {item_id}"
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct SlackSavedListResponse {
    ok: bool,
    error: Option<String>,
    saved_items: Option<Vec<SlackSavedWireItem>>,
    counts: Option<SlackSavedWireCounts>,
    response_metadata: Option<SlackSavedResponseMetadata>,
}

#[derive(Deserialize)]
struct SlackSavedMutationResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SlackSavedResponseMetadata {
    next_cursor: String,
}

#[derive(Deserialize)]
struct SlackSavedWireCounts {
    uncompleted_count: u32,
    uncompleted_overdue_count: u32,
    archived_count: u32,
    completed_count: u32,
    total_count: u32,
}

impl From<SlackSavedWireCounts> for SlackLaterCounts {
    fn from(counts: SlackSavedWireCounts) -> Self {
        Self {
            in_progress: counts.uncompleted_count,
            overdue: counts.uncompleted_overdue_count,
            archived: counts.archived_count,
            completed: counts.completed_count,
            total: counts.total_count,
        }
    }
}

#[derive(Deserialize)]
struct SlackSavedWireItem {
    date_completed: u64,
    date_created: u64,
    date_due: u64,
    #[serde(default)]
    date_snoozed_until: u64,
    date_updated: u64,
    #[serde(default)]
    is_archived: Option<bool>,
    item_id: String,
    item_type: String,
    state: String,
    todo_state: String,
    ts: Option<String>,
    is_tombstoned: Option<bool>,
    #[serde(default)]
    description: Option<String>,
}
