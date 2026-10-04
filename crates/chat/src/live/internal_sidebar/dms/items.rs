use std::collections::{BTreeSet, HashMap};

use crate::model::{
    SlackConversationKind, SlackDmInboxItem, SlackDmParticipant, SlackMessageTimestamp,
};
use serde_json::Value;

use super::types::{
    slack_timestamp_sort_key as timestamp_sort_key, timestamp_after, SlackClientCountsResponse,
    SlackClientDm, SlackClientDmsResponse, SlackDmCount, SlackDmMessage, SlackDmUser,
};
use crate::live::payload::SlackAvatarPurpose;
use crate::live::{
    cache::SlackUserPayloadCache, internal_sidebar::SlackFallbackUser, payload::slack_message_body,
};

const SLACKBOT_USER_ID: &str = "USLACK";

pub(super) struct SlackDmInboxItemsInput<'a> {
    pub(super) ims: Vec<SlackClientDm>,
    pub(super) mpims: Vec<SlackClientDm>,
    pub(super) self_user_id: &'a str,
    pub(super) users: &'a [SlackDmUser],
    pub(super) fallback_users: &'a HashMap<String, SlackFallbackUser>,
    pub(super) counts: &'a SlackClientCountsResponse,
}

pub(super) struct SlackDmInboxItems {
    pub(super) slackbot_conversation_id: Option<String>,
    pub(super) items: Vec<SlackDmInboxItem>,
}

pub(super) fn dm_inbox_items(
    input: SlackDmInboxItemsInput<'_>,
) -> Result<SlackDmInboxItems, String> {
    let SlackDmInboxItemsInput {
        ims,
        mpims,
        self_user_id,
        users,
        fallback_users,
        counts,
    } = input;
    let slackbot_conversation_id = ims
        .iter()
        .find(|dm| dm.channel.user.as_deref() == Some(SLACKBOT_USER_ID))
        .map(|dm| dm.id.clone());
    let users = users
        .iter()
        .map(|user| (user.id.as_str(), user))
        .collect::<HashMap<_, _>>();
    let fallback_users = fallback_users
        .iter()
        .map(|(user_id, user)| (user_id.as_str(), user))
        .collect::<HashMap<_, _>>();
    let counts = counts
        .conversations()
        .map(|count| (count.id.as_str(), count))
        .collect::<HashMap<_, _>>();
    let mut items = ims
        .into_iter()
        .chain(mpims)
        .filter(|dm| dm.channel.user.as_deref() != Some(SLACKBOT_USER_ID))
        .map(|dm| dm_inbox_item(dm, self_user_id, &users, &fallback_users, &counts))
        .collect::<Result<Vec<_>, String>>()?;
    items.sort_by(|left, right| {
        timestamp_sort_key(&right.latest_timestamp)
            .cmp(&timestamp_sort_key(&left.latest_timestamp))
            .then_with(|| left.conversation_id.cmp(&right.conversation_id))
    });
    Ok(SlackDmInboxItems {
        slackbot_conversation_id,
        items,
    })
}

pub(super) fn dm_inbox_user_ids(
    dms: &SlackClientDmsResponse,
    self_user_id: &str,
) -> BTreeSet<String> {
    let mut user_ids = BTreeSet::new();
    for dm in dms.ims.iter().chain(dms.mpims.iter()) {
        if dm.channel.user.as_deref() == Some(SLACKBOT_USER_ID) {
            continue;
        }
        if let Some(user_id) = dm.message.user.as_ref() {
            user_ids.insert(user_id.clone());
        }
        if dm.channel.is_mpim {
            user_ids.extend(
                dm.channel
                    .members
                    .iter()
                    .filter(|user_id| user_id.as_str() != self_user_id)
                    .cloned(),
            );
        } else if let Some(user_id) = dm.channel.user.as_ref() {
            user_ids.insert(user_id.clone());
        }
    }
    user_ids
}

pub(super) fn cached_dm_users(
    user_cache: &SlackUserPayloadCache,
    user_ids: &BTreeSet<String>,
) -> Result<Vec<SlackDmUser>, String> {
    let cached_users = user_cache.lock()?;
    user_ids
        .iter()
        .filter_map(|user_id| {
            cached_users.get(user_id).map(|payload| {
                serde_json::from_value::<SlackDmUser>(payload.clone()).map_err(|error| {
                    format!("failed to decode cached Slack user {user_id}: {error}")
                })
            })
        })
        .collect()
}

pub(super) fn dm_inbox_item(
    dm: SlackClientDm,
    self_user_id: &str,
    users: &HashMap<&str, &SlackDmUser>,
    fallback_users: &HashMap<&str, &SlackFallbackUser>,
    counts: &HashMap<&str, &SlackDmCount>,
) -> Result<SlackDmInboxItem, String> {
    let kind = dm_kind(&dm);
    let participants = dm_participants(&dm, kind, self_user_id, users, fallback_users);
    let label = dm_label(&dm, kind, &participants);
    let count = counts.get(dm.id.as_str()).copied();
    let latest_sender_user_id = dm.message.user.clone();
    let latest_sender_label =
        latest_sender_label(latest_sender_user_id.as_deref(), users, fallback_users);
    let latest_timestamp = SlackMessageTimestamp::parse(&dm.latest)
        .map_err(|error| format!("Slack client.dms latest for {}: {error}", dm.id))?;
    Ok(SlackDmInboxItem {
        conversation_id: dm.id,
        kind,
        label,
        participants,
        latest_sender_user_id,
        latest_sender_label,
        latest_message_text: dm_preview_text(&dm.message)?,
        latest_timestamp,
        unread_state_latest_timestamp: count
            .map(SlackDmCount::latest_message_timestamp)
            .transpose()?
            .flatten(),
        unread: count.map_or_else(
            || timestamp_after(&dm.latest, dm.channel.last_read.as_deref()),
            SlackDmCount::unread,
        ),
        mention_count: count.and_then(SlackDmCount::display_count),
    })
}

fn dm_kind(dm: &SlackClientDm) -> SlackConversationKind {
    if dm.channel.is_mpim {
        SlackConversationKind::GroupMessage
    } else {
        SlackConversationKind::DirectMessage
    }
}

fn dm_participants(
    dm: &SlackClientDm,
    kind: SlackConversationKind,
    self_user_id: &str,
    users: &HashMap<&str, &SlackDmUser>,
    fallback_users: &HashMap<&str, &SlackFallbackUser>,
) -> Vec<SlackDmParticipant> {
    let participant_user_ids = match kind {
        SlackConversationKind::DirectMessage => dm.channel.user.iter().cloned().collect(),
        SlackConversationKind::GroupMessage => dm
            .channel
            .members
            .iter()
            .filter(|user_id| user_id.as_str() != self_user_id)
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    let mut participants = participant_user_ids
        .into_iter()
        .map(|user_id| dm_participant(user_id, users, fallback_users))
        .collect::<Vec<_>>();
    if kind == SlackConversationKind::GroupMessage {
        participants.sort_by_cached_key(|participant| {
            (
                participant.label.to_ascii_lowercase(),
                participant.user_id.clone(),
            )
        });
    }
    participants
}

fn dm_label(
    dm: &SlackClientDm,
    kind: SlackConversationKind,
    participants: &[SlackDmParticipant],
) -> String {
    match kind {
        SlackConversationKind::DirectMessage => participants
            .first()
            .map(|participant| participant.label.clone())
            .or_else(|| dm.channel.user.clone())
            .unwrap_or_else(|| dm.id.clone()),
        SlackConversationKind::GroupMessage => {
            let label = participants
                .iter()
                .map(|participant| participant.label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if label.is_empty() {
                dm.id.clone()
            } else {
                label
            }
        }
        _ => dm.id.clone(),
    }
}

fn latest_sender_label(
    user_id: Option<&str>,
    users: &HashMap<&str, &SlackDmUser>,
    fallback_users: &HashMap<&str, &SlackFallbackUser>,
) -> Option<String> {
    user_id.and_then(|user_id| {
        users
            .get(user_id)
            .map(|user| user.display_name())
            .or_else(|| fallback_users.get(user_id).map(|user| user.label.clone()))
    })
}

fn dm_participant(
    user_id: String,
    users: &HashMap<&str, &SlackDmUser>,
    fallback_users: &HashMap<&str, &SlackFallbackUser>,
) -> SlackDmParticipant {
    let user = users.get(user_id.as_str()).copied();
    let fallback_user = fallback_users.get(user_id.as_str()).copied();
    SlackDmParticipant {
        label: user.map_or_else(
            || {
                fallback_user.map_or_else(
                    || user_id.clone(),
                    |fallback_user| fallback_user.label.clone(),
                )
            },
            SlackDmUser::display_name,
        ),
        avatar_image_url: user
            .and_then(|user| user.avatar_image_url(SlackAvatarPurpose::Message))
            .or_else(|| {
                fallback_user
                    .and_then(|fallback_user| fallback_user.message_avatar_image_url.clone())
            }),
        presence: user.and_then(SlackDmUser::presence),
        user_id,
    }
}

fn dm_preview_text(message: &SlackDmMessage) -> Result<String, String> {
    let value = serde_json::to_value(message)
        .map_err(|error| format!("failed to shape Slack client.dms message: {error}"))?;
    let body = slack_message_body(&value, !message.attachments.is_empty());
    if !body.is_empty() {
        return Ok(body);
    }
    Ok(message
        .files
        .iter()
        .find_map(|file| {
            string_value(file, "title")
                .or_else(|| string_value(file, "name"))
                .map(|name| format!("Shared a file: {name}"))
        })
        .or_else(|| {
            message.attachments.iter().find_map(|attachment| {
                string_value(attachment, "title").or_else(|| string_value(attachment, "fallback"))
            })
        })
        .unwrap_or_else(|| "Shared a message".to_string()))
}

fn string_value(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
