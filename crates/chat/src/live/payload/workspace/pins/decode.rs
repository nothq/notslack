use std::collections::{HashMap, HashSet};

use crate::model::{
    SlackPinnedFile, SlackPinnedFileComment, SlackPinnedItem, SlackPinnedMessage, SlackPinsRequest,
    SlackPinsSnapshot,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    validation::{
        decode_pinned_file_comment_message, decode_pinned_file_message, pinned_file_descriptor,
        pinned_message_timestamp, synthetic_pin_timestamp, validate_file_comment,
        validate_pins_response,
    },
    PINS_LIST,
};
use crate::live::payload::{
    message::slack_message_from_value_with_context_in_timezone, sidebar_dom::SlackSidebarSnapshot,
    users::load_slack_search_users_with_cache, workspace::SlackLiveWorkspaceLoader,
};

#[derive(Deserialize)]
pub(super) struct SlackPinsListResponse {
    items: Vec<SlackPinWire>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum SlackPinWire {
    Message {
        channel: String,
        created: u64,
        created_by: String,
        message: Value,
    },
    File {
        created: u64,
        created_by: String,
        file: Value,
    },
    FileComment {
        created: u64,
        created_by: String,
        file: Value,
        comment: SlackFileCommentWire,
    },
}

impl SlackPinWire {
    pub(super) fn created(&self) -> u64 {
        match self {
            Self::Message { created, .. }
            | Self::File { created, .. }
            | Self::FileComment { created, .. } => *created,
        }
    }

    pub(super) fn created_by(&self) -> &str {
        match self {
            Self::Message { created_by, .. }
            | Self::File { created_by, .. }
            | Self::FileComment { created_by, .. } => created_by,
        }
    }

    pub(super) fn kind_label(&self) -> &'static str {
        match self {
            Self::Message { .. } => "message pin",
            Self::File { .. } => "file pin",
            Self::FileComment { .. } => "file-comment pin",
        }
    }
}

#[derive(Deserialize)]
pub(super) struct SlackFileCommentWire {
    #[serde(rename = "type")]
    pub(super) comment_type: String,
    pub(super) id: String,
    pub(super) comment: String,
    pub(super) created: u64,
    pub(super) timestamp: u64,
    pub(super) user: String,
    #[serde(default)]
    pub(super) reactions: Vec<Value>,
}

pub(super) struct PinDecodeContext<'a> {
    pub(super) conversation_id: &'a str,
    pub(super) users: &'a HashMap<String, Value>,
    pub(super) sidebar: &'a SlackSidebarSnapshot,
    pub(super) self_user_id: &'a str,
    pub(super) timezone: chrono_tz::Tz,
}

impl SlackLiveWorkspaceLoader {
    pub(super) fn decode_pins_response(
        &self,
        request: SlackPinsRequest,
        response: SlackPinsListResponse,
    ) -> Result<SlackPinsSnapshot, String> {
        let user_ids = validate_pins_response(&response.items, &request.conversation_id)?;
        let users = load_slack_search_users_with_cache(
            &self.api,
            user_ids,
            &self.user_cache,
            &self.user_fetch_lock,
            || self.ensure_user_directory_cache(),
        )?;
        let sidebar = self.load_sidebar_snapshot()?;
        let self_user_id = self.load_self_user_id()?;
        let context = PinDecodeContext {
            conversation_id: &request.conversation_id,
            users: &users,
            sidebar: &sidebar,
            self_user_id: &self_user_id,
            timezone: self.timezone.value,
        };
        let items = decode_unique_pin_items(response.items, &context)?;
        Ok(SlackPinsSnapshot {
            team_id: self.team_id.clone(),
            conversation_id: request.conversation_id,
            items,
        })
    }
}

fn decode_unique_pin_items(
    items: Vec<SlackPinWire>,
    context: &PinDecodeContext<'_>,
) -> Result<Vec<SlackPinnedItem>, String> {
    let mut item_ids = HashSet::with_capacity(items.len());
    items
        .into_iter()
        .map(|item| {
            let item = decode_pin_item(item, context)?;
            let id = item.id().to_string();
            if !item_ids.insert(id.clone()) {
                return Err(format!(
                    "Slack {PINS_LIST} returned duplicate pinned item {id}"
                ));
            }
            Ok(item)
        })
        .collect()
}

fn decode_pin_item(
    item: SlackPinWire,
    context: &PinDecodeContext<'_>,
) -> Result<SlackPinnedItem, String> {
    match item {
        SlackPinWire::Message {
            channel,
            created,
            created_by,
            message,
        } => decode_message_pin(channel, created, created_by, message, context),
        SlackPinWire::File {
            created,
            created_by,
            file,
        } => decode_file_pin(created, created_by, file, context),
        SlackPinWire::FileComment {
            created,
            created_by,
            file,
            comment,
        } => decode_file_comment_pin(created, created_by, file, comment, context),
    }
}

fn decode_message_pin(
    channel: String,
    created: u64,
    created_by: String,
    message: Value,
    context: &PinDecodeContext<'_>,
) -> Result<SlackPinnedItem, String> {
    let timestamp = pinned_message_timestamp(&message)?.to_string();
    let thread_timestamp = message
        .get("thread_ts")
        .and_then(Value::as_str)
        .filter(|thread_timestamp| *thread_timestamp != timestamp)
        .map(|thread_timestamp| {
            crate::model::SlackMessageTimestamp::parse(thread_timestamp)
                .map(|_| thread_timestamp.to_string())
                .map_err(|error| {
                    format!("Slack {PINS_LIST} returned an invalid thread timestamp: {error}")
                })
        })
        .transpose()?;
    let id = format!("{channel}:message:{timestamp}");
    let message = slack_message_from_value_with_context_in_timezone(
        message,
        context.users,
        Some(context.sidebar),
        Some(context.self_user_id),
        context.timezone,
    );
    if message.id != timestamp {
        return Err(format!(
            "Slack {PINS_LIST} decoded message {} for timestamp {timestamp}",
            message.id
        ));
    }
    Ok(SlackPinnedItem::Message(SlackPinnedMessage {
        id,
        conversation_id: channel,
        created_unix_seconds: created,
        created_by_user_id: created_by,
        thread_timestamp,
        message,
    }))
}

fn decode_file_pin(
    created: u64,
    created_by: String,
    file: Value,
    context: &PinDecodeContext<'_>,
) -> Result<SlackPinnedItem, String> {
    let descriptor = pinned_file_descriptor(&file)?;
    let id = format!("{}:file:{}", context.conversation_id, descriptor.file_id);
    let timestamp = synthetic_pin_timestamp(descriptor.created, &descriptor.file_id);
    let file_message =
        decode_pinned_file_message(file, &descriptor.owner_user_id, timestamp, context)?;
    Ok(SlackPinnedItem::File(SlackPinnedFile {
        id,
        conversation_id: context.conversation_id.to_string(),
        created_unix_seconds: created,
        created_by_user_id: created_by,
        file_id: descriptor.file_id,
        file_message,
    }))
}

fn decode_file_comment_pin(
    created: u64,
    created_by: String,
    file: Value,
    comment: SlackFileCommentWire,
    context: &PinDecodeContext<'_>,
) -> Result<SlackPinnedItem, String> {
    let descriptor = pinned_file_descriptor(&file)?;
    validate_file_comment(&comment)?;
    let id = format!(
        "{}:file-comment:{}:{}",
        context.conversation_id, descriptor.file_id, comment.id
    );
    let timestamp = synthetic_pin_timestamp(comment.timestamp, &comment.id);
    let comment_id = comment.id.clone();
    let comment_message = decode_pinned_file_comment_message(file, comment, timestamp, context)?;
    Ok(SlackPinnedItem::FileComment(SlackPinnedFileComment {
        id,
        conversation_id: context.conversation_id.to_string(),
        created_unix_seconds: created,
        created_by_user_id: created_by,
        file_id: descriptor.file_id,
        comment_id,
        comment_message,
    }))
}
