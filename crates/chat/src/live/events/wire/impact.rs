use crate::{
    live::api::SlackRealtimeSocketUrl,
    model::{
        SlackMessageTimestamp, SlackRealtimeBatch, SlackRealtimePresenceChange,
        SlackRealtimeThreadTarget, SlackUserPresence, SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES,
        SLACK_REALTIME_PRESENCE_USER_LIMIT,
    },
};

use super::notification::{notification_impact, SlackNotificationImpactInput};
use super::{
    SlackRealtimeWireAction, SlackRealtimeWireEvent, SlackWirePresenceChange,
    SlackWireReactionItem, SlackWireSavedItem, SlackWireThreadReference,
};

impl SlackRealtimeWireEvent {
    pub(super) fn into_action(self) -> Result<SlackRealtimeWireAction, String> {
        match self {
            Self::Hello => Ok(SlackRealtimeWireAction::Hello),
            Self::ReconnectUrl { url } => SlackRealtimeSocketUrl::parse_reconnect(&url)
                .map(SlackRealtimeWireAction::Reconnect),
            Self::Pong => Ok(SlackRealtimeWireAction::Pong),
            Self::Goodbye => Ok(SlackRealtimeWireAction::Goodbye),
            Self::PresenceChange(change) => Ok(match presence_impact(change) {
                Ok(batch) => SlackRealtimeWireAction::Impact(batch),
                Err(_) => SlackRealtimeWireAction::MalformedPresence,
            }),
            event => event.into_notification_action(),
        }
    }

    fn into_notification_action(self) -> Result<SlackRealtimeWireAction, String> {
        match self {
            Self::DesktopNotification(notification) => {
                let super::SlackWireDesktopNotification {
                    id,
                    notification_id,
                    team_id,
                    channel,
                    channel_id,
                    ts,
                    thread_ts,
                    event_ts,
                    title,
                    subtitle,
                    content,
                    msg,
                    avatar_image,
                    icon_url,
                    user,
                    sender_user_id,
                    sound,
                    silent,
                    has_reply,
                    launch_uri,
                    actions,
                } = *notification;
                notification_impact(SlackNotificationImpactInput {
                    id: id
                        .filter(|id| !id.is_empty())
                        .or_else(|| notification_id.filter(|id| !id.is_empty())),
                    team_id,
                    channel_id: channel.or(channel_id),
                    message_timestamp: ts,
                    thread_timestamp: thread_ts,
                    event_timestamp: event_ts,
                    title,
                    subtitle,
                    content,
                    message: msg,
                    icon_url: icon_url.or(avatar_image),
                    sender_user_id: sender_user_id.or(user),
                    sound,
                    silent,
                    has_reply,
                    launch_uri,
                    actions,
                })
                .map(SlackRealtimeWireAction::Impact)
            }
            event => event.into_message_action(),
        }
    }

    fn into_message_action(self) -> Result<SlackRealtimeWireAction, String> {
        let action = match self {
            Self::Message {
                channel,
                subtype,
                ts,
                thread_ts,
                message,
                previous_message,
            } => SlackRealtimeWireAction::Impact(message_impact(
                channel,
                subtype.as_deref(),
                thread_ts
                    .or_else(|| {
                        message
                            .as_ref()
                            .and_then(|message| message.thread_ts.clone())
                    })
                    .or_else(|| {
                        previous_message
                            .as_ref()
                            .and_then(|message| message.thread_ts.clone())
                    }),
                ts.or_else(|| message.and_then(|message| message.ts))
                    .or_else(|| previous_message.and_then(|message| message.ts)),
            )),
            Self::ReactionAdded { item } | Self::ReactionRemoved { item } => {
                SlackRealtimeWireAction::Impact(reaction_impact(item))
            }
            Self::ChannelMarked { channel }
            | Self::ImMarked { channel }
            | Self::MpimMarked { channel }
            | Self::GroupMarked { channel } => {
                SlackRealtimeWireAction::Impact(mark_impact(channel))
            }
            event => return event.into_thread_action(),
        };
        Ok(action)
    }

    fn into_thread_action(self) -> Result<SlackRealtimeWireAction, String> {
        let action = match self {
            Self::ThreadMarked {
                channel,
                channel_id,
                thread_ts,
                ts,
                subscription,
            }
            | Self::ThreadSubscribed {
                channel,
                channel_id,
                thread_ts,
                ts,
                subscription,
            }
            | Self::ThreadUnsubscribed {
                channel,
                channel_id,
                thread_ts,
                ts,
                subscription,
            } => SlackRealtimeWireAction::Impact(thread_impact(
                channel,
                channel_id,
                thread_ts,
                ts,
                subscription,
            )),
            Self::UpdateThreadState { channel_ids } => {
                SlackRealtimeWireAction::Impact(thread_state_impact(channel_ids))
            }
            Self::UpdateGlobalThreadState => SlackRealtimeWireAction::Impact(SlackRealtimeBatch {
                all_threads_changed: true,
                ..SlackRealtimeBatch::default()
            }),
            event => return event.into_sync_action(),
        };
        Ok(action)
    }

    fn into_sync_action(self) -> Result<SlackRealtimeWireAction, String> {
        let action = match self {
            Self::Desync { channel_ids } | Self::Resync { channel_ids } => {
                SlackRealtimeWireAction::Impact(resync_impact(channel_ids))
            }
            Self::ClientCounts { channel_ids } => {
                let mut batch = conversations_impact(channel_ids);
                batch.sidebar_changed = true;
                SlackRealtimeWireAction::Impact(batch)
            }
            Self::ChannelCreated { channel }
            | Self::ChannelJoined { channel }
            | Self::ChannelLeft { channel }
            | Self::ChannelArchive { channel }
            | Self::ChannelUnarchive { channel }
            | Self::ChannelRename { channel }
            | Self::ChannelDeleted { channel }
            | Self::ChannelUpdated { channel } => {
                SlackRealtimeWireAction::Impact(sidebar_impact(Some(channel.into_id())))
            }
            Self::MemberJoinedChannel { channel } | Self::MemberLeftChannel { channel } => {
                SlackRealtimeWireAction::Impact(sidebar_impact(Some(channel)))
            }
            Self::TeamJoin
            | Self::UserChange
            | Self::SubteamCreated
            | Self::SubteamUpdated
            | Self::SubteamSelfAdded
            | Self::SubteamSelfRemoved => SlackRealtimeWireAction::Impact(sidebar_impact(None)),
            event => return Ok(event.into_asset_action()),
        };
        Ok(action)
    }

    fn into_asset_action(self) -> SlackRealtimeWireAction {
        match self {
            Self::SavedItemAdded { item }
            | Self::SavedItemDeleted { item }
            | Self::SavedItemUpdated { item } => {
                SlackRealtimeWireAction::Impact(later_impact(item))
            }
            Self::FileCreated { channel_id }
            | Self::FileShared { channel_id }
            | Self::FileUnshared { channel_id }
            | Self::FileDeleted { channel_id }
            | Self::FileChange { channel_id }
            | Self::FilePublic { channel_id }
            | Self::FilePrivate { channel_id } => {
                SlackRealtimeWireAction::Impact(files_impact(channel_id))
            }
            Self::Ignored => SlackRealtimeWireAction::Ignore,
            _ => SlackRealtimeWireAction::Ignore,
        }
    }
}

fn presence_impact(change: SlackWirePresenceChange) -> Result<SlackRealtimeBatch, String> {
    let (presence, users) = match change {
        SlackWirePresenceChange::Single { presence, user } => (presence, vec![user]),
        SlackWirePresenceChange::Batch { presence, users } => (presence, users),
        SlackWirePresenceChange::MalformedBatch { .. } | SlackWirePresenceChange::Malformed {} => {
            return Err("presence user fields are malformed".to_string());
        }
    };
    let presence = match presence.as_str() {
        Some("active") => SlackUserPresence::Active,
        Some("away") => SlackUserPresence::Away,
        Some(_) => return Err("unsupported presence value".to_string()),
        None => return Err("presence is not a string".to_string()),
    };
    if users.len() > SLACK_REALTIME_PRESENCE_USER_LIMIT {
        return Err(format!(
            "users exceeds the {SLACK_REALTIME_PRESENCE_USER_LIMIT}-user bound"
        ));
    }
    let mut seen = std::collections::HashSet::with_capacity(users.len());
    let mut presence_changes = Vec::with_capacity(users.len());
    for user in &users {
        let user_id = user.trim();
        if user_id.is_empty() {
            return Err("users contains an empty id".to_string());
        }
        if user_id.len() > SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES {
            return Err(format!(
                "presence user id exceeds {SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES} bytes"
            ));
        }
        if seen.insert(user_id) {
            presence_changes.push(SlackRealtimePresenceChange {
                user_id: user_id.to_string(),
                presence,
            });
        }
    }
    Ok(SlackRealtimeBatch {
        presence_changes,
        ..SlackRealtimeBatch::default()
    })
}

fn message_impact(
    conversation_id: String,
    subtype: Option<&str>,
    thread_timestamp: Option<SlackMessageTimestamp>,
    message_timestamp: Option<SlackMessageTimestamp>,
) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(vec![conversation_id.clone()]);
    batch.sidebar_changed = true;
    batch.activity_changed = true;
    if subtype == Some("file_share") {
        batch.files_changed = true;
    }
    if subtype == Some("message_replied") || thread_timestamp.is_some() {
        batch.all_threads_changed = true;
    }
    if let Some(thread_timestamp) = thread_timestamp.or(message_timestamp) {
        if subtype == Some("message_replied") || batch.all_threads_changed {
            batch.threads.push(SlackRealtimeThreadTarget {
                conversation_id,
                thread_timestamp,
            });
        }
    }
    batch
}

fn reaction_impact(item: SlackWireReactionItem) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(item.channel.clone().into_iter().collect());
    batch.activity_changed = true;
    batch.all_threads_changed = true;
    if let (Some(conversation_id), Some(thread_timestamp)) = (item.channel, item.ts) {
        batch.threads.push(SlackRealtimeThreadTarget {
            conversation_id,
            thread_timestamp,
        });
    }
    batch
}

fn mark_impact(conversation_id: String) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(vec![conversation_id]);
    batch.sidebar_changed = true;
    batch
}

fn thread_impact(
    channel: Option<String>,
    channel_id: Option<String>,
    thread_ts: Option<SlackMessageTimestamp>,
    ts: Option<SlackMessageTimestamp>,
    subscription: Option<SlackWireThreadReference>,
) -> SlackRealtimeBatch {
    let subscription_channel = subscription.as_ref().and_then(|subscription| {
        subscription
            .channel
            .clone()
            .or_else(|| subscription.channel_id.clone())
    });
    let subscription_timestamp =
        subscription.and_then(|subscription| subscription.thread_ts.or(subscription.ts));
    let conversation_id = channel.or(channel_id).or(subscription_channel);
    let thread_timestamp = thread_ts.or(ts).or(subscription_timestamp);
    let mut batch = conversations_impact(conversation_id.clone().into_iter().collect());
    batch.sidebar_changed = true;
    batch.activity_changed = true;
    batch.all_threads_changed = true;
    if let (Some(conversation_id), Some(thread_timestamp)) = (conversation_id, thread_timestamp) {
        batch.threads.push(SlackRealtimeThreadTarget {
            conversation_id,
            thread_timestamp,
        });
    }
    batch
}

fn thread_state_impact(channel_ids: Vec<String>) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(channel_ids);
    batch.sidebar_changed = true;
    batch.all_threads_changed = true;
    batch
}

fn resync_impact(channel_ids: Vec<String>) -> SlackRealtimeBatch {
    if channel_ids.is_empty() {
        return SlackRealtimeBatch::resync();
    }
    let mut batch = conversations_impact(channel_ids);
    batch.sidebar_changed = true;
    batch.activity_changed = true;
    batch.later_changed = true;
    batch.files_changed = true;
    batch.all_threads_changed = true;
    batch
}

fn sidebar_impact(conversation_id: Option<String>) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(conversation_id.into_iter().collect());
    batch.sidebar_changed = true;
    batch
}

fn later_impact(item: Option<SlackWireSavedItem>) -> SlackRealtimeBatch {
    let mut batch =
        conversations_impact(item.and_then(|item| item.channel_id).into_iter().collect());
    batch.later_changed = true;
    batch
}

fn files_impact(conversation_id: Option<String>) -> SlackRealtimeBatch {
    let mut batch = conversations_impact(conversation_id.into_iter().collect());
    batch.files_changed = true;
    batch
}

pub(super) fn conversations_impact(conversation_ids: Vec<String>) -> SlackRealtimeBatch {
    SlackRealtimeBatch {
        conversation_ids,
        ..SlackRealtimeBatch::default()
    }
}
