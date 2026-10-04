use std::collections::BTreeSet;

use crate::model::SlackMessage;
use crate::model::SlackMessageTimestamp;
use serde_json::{Map, Value};

use super::{
    decode::{PinDecodeContext, SlackFileCommentWire, SlackPinWire},
    PINS_LIST,
};
use crate::live::payload::message::slack_message_from_value_with_context_in_timezone;

pub(super) struct SlackPinnedFileDescriptor {
    pub(super) file_id: String,
    pub(super) owner_user_id: String,
    pub(super) created: u64,
}

pub(super) fn validate_pins_response(
    items: &[SlackPinWire],
    expected_channel: &str,
) -> Result<BTreeSet<String>, String> {
    let mut user_ids = BTreeSet::new();
    for item in items {
        if item.created() == 0 {
            return Err(format!(
                "Slack {PINS_LIST} returned {} without a valid pin-created timestamp",
                item.kind_label()
            ));
        }
        validate_pin_item(item, expected_channel, &mut user_ids)?;
        user_ids.insert(item.created_by().to_string());
    }
    Ok(user_ids)
}

fn validate_pin_item(
    item: &SlackPinWire,
    expected_channel: &str,
    user_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    match item {
        SlackPinWire::Message {
            channel,
            created_by,
            message,
            ..
        } => {
            validate_pin_actor(created_by)?;
            validate_pinned_message(channel, expected_channel, message)?;
            collect_pinned_message_user_ids(message, user_ids);
        }
        SlackPinWire::File {
            created_by, file, ..
        } => {
            validate_pin_actor(created_by)?;
            user_ids.insert(pinned_file_descriptor(file)?.owner_user_id);
        }
        SlackPinWire::FileComment {
            created_by,
            file,
            comment,
            ..
        } => {
            validate_pin_actor(created_by)?;
            user_ids.insert(pinned_file_descriptor(file)?.owner_user_id);
            validate_file_comment(comment)?;
            user_ids.insert(comment.user.clone());
        }
    }
    Ok(())
}

fn validate_pin_actor(created_by: &str) -> Result<(), String> {
    require_non_empty(created_by, "created_by")
}

fn validate_pinned_message(
    channel: &str,
    expected_channel: &str,
    message: &Value,
) -> Result<(), String> {
    require_non_empty(channel, "channel")?;
    if channel != expected_channel {
        return Err(format!(
            "Slack {PINS_LIST} returned channel {channel} for requested conversation {expected_channel}"
        ));
    }
    pinned_message_timestamp(message)?;
    Ok(())
}

pub(super) fn pinned_message_timestamp(message: &Value) -> Result<&str, String> {
    let timestamp = required_string_field(message, "ts", "message")?;
    SlackMessageTimestamp::parse(timestamp).map_err(|error| {
        format!("Slack {PINS_LIST} returned an invalid message timestamp: {error}")
    })?;
    Ok(timestamp)
}

pub(super) fn pinned_file_descriptor(file: &Value) -> Result<SlackPinnedFileDescriptor, String> {
    let file_id = required_string_field(file, "id", "file")?.to_string();
    let owner_user_id = required_string_field(file, "user", "file")?.to_string();
    if ["title", "name"].into_iter().all(|field| {
        file.get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
    }) {
        return Err(format!(
            "Slack {PINS_LIST} returned file {file_id} without a title or name"
        ));
    }
    let created = file
        .get("created")
        .and_then(Value::as_u64)
        .or_else(|| file.get("timestamp").and_then(Value::as_u64))
        .filter(|created| *created > 0)
        .ok_or_else(|| {
            format!("Slack {PINS_LIST} returned file {file_id} without a valid created timestamp")
        })?;
    Ok(SlackPinnedFileDescriptor {
        file_id,
        owner_user_id,
        created,
    })
}

pub(super) fn validate_file_comment(comment: &SlackFileCommentWire) -> Result<(), String> {
    if comment.comment_type != "file_comment" {
        return Err(format!(
            "Slack {PINS_LIST} returned file_comment item with comment type {}",
            comment.comment_type
        ));
    }
    require_non_empty(&comment.id, "file comment id")?;
    require_non_empty(&comment.user, "file comment user")?;
    require_non_empty(&comment.comment, "file comment body")?;
    if comment.created == 0 || comment.timestamp == 0 {
        return Err(format!(
            "Slack {PINS_LIST} returned file comment {} without valid timestamps",
            comment.id
        ));
    }
    Ok(())
}

pub(super) fn decode_pinned_file_message(
    file: Value,
    owner_user_id: &str,
    timestamp: String,
    context: &PinDecodeContext<'_>,
) -> Result<SlackMessage, String> {
    let message = pinned_file_message_value(
        file,
        owner_user_id.to_string(),
        timestamp.clone(),
        None,
        Vec::new(),
    );
    decode_and_validate_file_message(message, context, &timestamp)
}

pub(super) fn decode_pinned_file_comment_message(
    file: Value,
    comment: SlackFileCommentWire,
    timestamp: String,
    context: &PinDecodeContext<'_>,
) -> Result<SlackMessage, String> {
    let message = pinned_file_message_value(
        file,
        comment.user,
        timestamp.clone(),
        Some(comment.comment),
        comment.reactions,
    );
    decode_and_validate_file_message(message, context, &timestamp)
}

fn decode_and_validate_file_message(
    message: Value,
    context: &PinDecodeContext<'_>,
    timestamp: &str,
) -> Result<SlackMessage, String> {
    let message = slack_message_from_value_with_context_in_timezone(
        message,
        context.users,
        Some(context.sidebar),
        Some(context.self_user_id),
        context.timezone,
    );
    validate_decoded_file_message(&message, timestamp)?;
    Ok(message)
}

fn pinned_file_message_value(
    file: Value,
    user_id: String,
    timestamp: String,
    text: Option<String>,
    reactions: Vec<Value>,
) -> Value {
    let mut message = Map::new();
    message.insert("ts".to_string(), Value::String(timestamp));
    message.insert("user".to_string(), Value::String(user_id));
    message.insert("files".to_string(), Value::Array(vec![file]));
    if let Some(text) = text {
        message.insert("text".to_string(), Value::String(text));
    }
    if !reactions.is_empty() {
        message.insert("reactions".to_string(), Value::Array(reactions));
    }
    Value::Object(message)
}

fn validate_decoded_file_message(message: &SlackMessage, timestamp: &str) -> Result<(), String> {
    if message.id != timestamp {
        return Err(format!(
            "Slack {PINS_LIST} decoded file item {} for timestamp {timestamp}",
            message.id
        ));
    }
    if message.attachments.len() != 1 {
        return Err(format!(
            "Slack {PINS_LIST} decoded file item {timestamp} with {} attachments",
            message.attachments.len()
        ));
    }
    Ok(())
}

pub(super) fn synthetic_pin_timestamp(unix_seconds: u64, stable_id: &str) -> String {
    let hash = stable_id
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    format!("{unix_seconds}.{:06}", hash % 1_000_000)
}

fn required_string_field<'a>(
    value: &'a Value,
    field: &str,
    object_label: &str,
) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Slack {PINS_LIST} returned {object_label} without a valid {field}"))
}

fn collect_pinned_message_user_ids(message: &Value, user_ids: &mut BTreeSet<String>) {
    match message {
        Value::Array(values) => {
            for value in values {
                collect_pinned_message_user_ids(value, user_ids);
            }
        }
        Value::Object(fields) => {
            for (field, value) in fields {
                collect_user_id_field(field, value, user_ids);
                collect_pinned_message_user_ids(value, user_ids);
            }
        }
        _ => {}
    }
}

fn collect_user_id_field(field: &str, value: &Value, user_ids: &mut BTreeSet<String>) {
    if matches!(field, "user" | "user_id" | "author_id" | "created_by") {
        if let Some(user_id) = value
            .as_str()
            .map(str::trim)
            .filter(|user_id| !user_id.is_empty())
        {
            user_ids.insert(user_id.to_string());
        }
    } else if matches!(field, "users" | "reply_users") {
        if let Some(users) = value.as_array() {
            user_ids.extend(
                users
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|user_id| !user_id.is_empty())
                    .map(str::to_string),
            );
        }
    }
}

fn require_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("Slack {PINS_LIST} returned an empty {field}"))
    } else {
        Ok(())
    }
}
