use crate::model::{SlackActivityCursor, SlackMessageTimestamp};
use serde::Deserialize;

use super::{
    SlackActivityFeedContent, SlackActivityFeedItem, SlackActivityFeedPage, ACTIVITY_FEED,
    ACTIVITY_PAGE_SIZE,
};
use crate::live::api::messages::SlackMessageReference;

pub(super) fn activity_request_metadata(reason: &str) -> Vec<(&'static str, String)> {
    vec![
        ("_x_reason", reason.to_string()),
        ("_x_mode", "online".to_string()),
        ("_x_sonic", "true".to_string()),
        ("_x_app_name", "client".to_string()),
    ]
}

pub(super) fn decode_activity_mutation(method: &str, body: &str) -> Result<(), String> {
    let response = serde_json::from_str::<SlackActivityMutationResponse>(body)
        .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "Slack {method} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    Ok(())
}

pub(super) fn decode_activity_feed(body: &str) -> Result<SlackActivityFeedPage, String> {
    let response = serde_json::from_str::<SlackActivityFeedResponse>(body)
        .map_err(|error| format!("failed to decode Slack {ACTIVITY_FEED} response: {error}"))?;
    if !response.ok {
        return Err(format!(
            "Slack {ACTIVITY_FEED} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    let items = response
        .items
        .ok_or_else(|| format!("Slack {ACTIVITY_FEED} response omitted items"))?;
    if items.len() > ACTIVITY_PAGE_SIZE {
        return Err(format!(
            "Slack {ACTIVITY_FEED} returned {} items for a {ACTIVITY_PAGE_SIZE}-item page",
            items.len()
        ));
    }
    let activity_views_date_updated = response.activity_views_date_updated.ok_or_else(|| {
        format!("Slack {ACTIVITY_FEED} response omitted activity_views_date_updated")
    })?;
    let response_metadata = response
        .response_metadata
        .ok_or_else(|| format!("Slack {ACTIVITY_FEED} response omitted response_metadata"))?;
    let next_cursor = if response_metadata.next_cursor.is_empty() {
        None
    } else {
        Some(SlackActivityCursor::new(response_metadata.next_cursor)?)
    };
    Ok(SlackActivityFeedPage {
        activity_views_date_updated,
        items: items
            .into_iter()
            .map(SlackActivityFeedItem::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        next_cursor,
    })
}

#[derive(Debug, Deserialize)]
struct SlackActivityFeedResponse {
    ok: bool,
    error: Option<String>,
    activity_views_date_updated: Option<String>,
    items: Option<Vec<SlackActivityWireItem>>,
    response_metadata: Option<SlackActivityResponseMetadata>,
}

#[derive(Debug, Deserialize)]
struct SlackActivityMutationResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackActivityResponseMetadata {
    next_cursor: String,
}

#[derive(Debug, Deserialize)]
struct SlackActivityWireItem {
    feed_ts: String,
    is_archived: bool,
    is_unread: bool,
    item: SlackActivityWireContent,
    key: String,
    version: String,
    is_bot: Option<bool>,
}

impl TryFrom<SlackActivityWireItem> for SlackActivityFeedItem {
    type Error = String;

    fn try_from(item: SlackActivityWireItem) -> Result<Self, Self::Error> {
        Ok(Self {
            key: item.key,
            feed_timestamp: item.feed_ts,
            version: item.version,
            archived: item.is_archived,
            unread: item.is_unread,
            bot: item.is_bot,
            content: SlackActivityFeedContent::try_from(item.item)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SlackActivityWireContent {
    Known(SlackActivityKnownWireContent),
    Unsupported(SlackActivityUnsupportedWireContent),
}

impl TryFrom<SlackActivityWireContent> for SlackActivityFeedContent {
    type Error = String;

    fn try_from(content: SlackActivityWireContent) -> Result<Self, Self::Error> {
        match content {
            SlackActivityWireContent::Known(content) => content.try_into(),
            SlackActivityWireContent::Unsupported(content)
                if is_observed_activity_type(&content.kind) =>
            {
                Err(format!(
                    "Slack {ACTIVITY_FEED} returned malformed {} item",
                    content.kind
                ))
            }
            SlackActivityWireContent::Unsupported(content) => {
                Ok(Self::Unsupported { kind: content.kind })
            }
        }
    }
}

fn is_observed_activity_type(kind: &str) -> bool {
    matches!(
        kind,
        "message_reaction" | "thread_v2" | "at_user" | "dm" | "bot_dm_bundle"
    )
}

#[derive(Debug, Deserialize)]
struct SlackActivityUnsupportedWireContent {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SlackActivityKnownWireContent {
    MessageReaction {
        message: SlackActivityReactionMessage,
        reaction: SlackActivityReaction,
    },
    ThreadV2 {
        bundle_info: SlackActivityThreadBundleInfo,
    },
    AtUser {
        message: SlackActivityMentionMessage,
    },
    Dm {
        bundle_info: SlackActivityDmBundleInfo,
    },
    BotDmBundle {
        bundle_info: SlackActivityBotDmBundleInfo,
    },
}

impl TryFrom<SlackActivityKnownWireContent> for SlackActivityFeedContent {
    type Error = String;

    fn try_from(content: SlackActivityKnownWireContent) -> Result<Self, Self::Error> {
        Ok(match content {
            SlackActivityKnownWireContent::MessageReaction { message, reaction } => {
                Self::MessageReaction {
                    reference: activity_message_reference(&message.channel, &message.ts)?,
                    reaction_name: reaction.name,
                    reaction_user_id: reaction.user,
                }
            }
            SlackActivityKnownWireContent::ThreadV2 { bundle_info } => {
                let entry = bundle_info.payload.thread_entry;
                Self::ThreadV2 {
                    reference: activity_message_reference(&entry.channel_id, &entry.latest_ts)?,
                    thread_timestamp: activity_message_timestamp(
                        "thread_v2 thread timestamp",
                        &entry.thread_ts,
                    )?,
                    unread_message_count: entry.unread_msg_count,
                }
            }
            SlackActivityKnownWireContent::AtUser { message } => Self::AtUser {
                reference: activity_message_reference(&message.channel, &message.ts)?,
                author_user_id: message.author_user_id,
                broadcast: message.is_broadcast,
                thread_timestamp: message
                    .thread_ts
                    .as_deref()
                    .map(|timestamp| {
                        activity_message_timestamp("at_user thread timestamp", timestamp)
                    })
                    .transpose()?,
            },
            SlackActivityKnownWireContent::Dm { bundle_info } => {
                let message = bundle_info.payload.dm_entry.latest_message;
                Self::Dm {
                    reference: activity_message_reference(&message.channel, &message.ts)?,
                }
            }
            SlackActivityKnownWireContent::BotDmBundle { bundle_info } => Self::BotDmBundle {
                reference: activity_message_reference(
                    &bundle_info.payload.message.channel,
                    &bundle_info.payload.message.ts,
                )?,
                unread_count: bundle_info.unread_count,
            },
        })
    }
}

fn activity_message_reference(
    channel_id: &str,
    timestamp: &str,
) -> Result<SlackMessageReference, String> {
    SlackMessageReference::parse(channel_id, timestamp).map_err(|error| {
        format!("Slack {ACTIVITY_FEED} returned an invalid message target: {error}")
    })
}

fn activity_message_timestamp(
    field: &str,
    timestamp: &str,
) -> Result<SlackMessageTimestamp, String> {
    SlackMessageTimestamp::parse(timestamp)
        .map_err(|error| format!("Slack {ACTIVITY_FEED} returned invalid {field}: {error}"))
}

#[derive(Debug, Deserialize)]
struct SlackActivityReactionMessage {
    channel: String,
    ts: String,
}

#[derive(Debug, Deserialize)]
struct SlackActivityReaction {
    name: String,
    user: String,
}

#[derive(Debug, Deserialize)]
struct SlackActivityThreadBundleInfo {
    payload: SlackActivityThreadPayload,
}

#[derive(Debug, Deserialize)]
struct SlackActivityThreadPayload {
    thread_entry: SlackActivityThreadEntry,
}

#[derive(Debug, Deserialize)]
struct SlackActivityThreadEntry {
    channel_id: String,
    latest_ts: String,
    thread_ts: String,
    unread_msg_count: u32,
}

#[derive(Debug, Deserialize)]
struct SlackActivityMentionMessage {
    author_user_id: String,
    channel: String,
    is_broadcast: bool,
    ts: String,
    thread_ts: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SlackActivityDmBundleInfo {
    payload: SlackActivityDmPayload,
}

#[derive(Debug, Deserialize)]
struct SlackActivityDmPayload {
    dm_entry: SlackActivityDmEntry,
}

#[derive(Debug, Deserialize)]
struct SlackActivityDmEntry {
    latest_message: SlackActivityDmMessage,
}

#[derive(Debug, Deserialize)]
struct SlackActivityDmMessage {
    channel: String,
    ts: String,
}

#[derive(Debug, Deserialize)]
struct SlackActivityBotDmBundleInfo {
    payload: SlackActivityBotDmPayload,
    unread_count: u32,
}

#[derive(Debug, Deserialize)]
struct SlackActivityBotDmPayload {
    message: SlackActivityBotDmMessage,
}

#[derive(Debug, Deserialize)]
struct SlackActivityBotDmMessage {
    channel: String,
    ts: String,
}
