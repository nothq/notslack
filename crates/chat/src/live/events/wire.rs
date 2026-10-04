mod impact;
mod notification;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    live::api::SlackRealtimeSocketUrl,
    model::{SlackMessageTimestamp, SlackRealtimeBatch},
};

use notification::{SlackWireNotificationAction, SlackWireNotificationMessage};

pub(super) enum SlackRealtimeWireAction {
    Hello,
    Reconnect(SlackRealtimeSocketUrl),
    Pong,
    Goodbye,
    Impact(SlackRealtimeBatch),
    MalformedPresence,
    Ignore,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackRealtimeWireEvent {
    #[serde(rename = "hello")]
    Hello,
    #[serde(rename = "reconnect_url")]
    ReconnectUrl { url: String },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "goodbye")]
    Goodbye,
    #[serde(rename = "presence_change")]
    PresenceChange(SlackWirePresenceChange),
    #[serde(rename = "desktop_notification")]
    DesktopNotification(Box<SlackWireDesktopNotification>),
    #[serde(rename = "message")]
    Message {
        channel: String,
        #[serde(default)]
        subtype: Option<String>,
        #[serde(default)]
        ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        thread_ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        message: Option<SlackWireMessage>,
        #[serde(default)]
        previous_message: Option<SlackWireMessage>,
    },
    #[serde(rename = "reaction_added")]
    ReactionAdded { item: SlackWireReactionItem },
    #[serde(rename = "reaction_removed")]
    ReactionRemoved { item: SlackWireReactionItem },
    #[serde(rename = "channel_marked")]
    ChannelMarked { channel: String },
    #[serde(rename = "im_marked")]
    ImMarked { channel: String },
    #[serde(rename = "mpim_marked")]
    MpimMarked { channel: String },
    #[serde(rename = "group_marked")]
    GroupMarked { channel: String },
    #[serde(rename = "thread_marked")]
    ThreadMarked {
        #[serde(default)]
        channel: Option<String>,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        thread_ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        subscription: Option<SlackWireThreadReference>,
    },
    #[serde(rename = "thread_subscribed")]
    ThreadSubscribed {
        #[serde(default)]
        channel: Option<String>,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        thread_ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        subscription: Option<SlackWireThreadReference>,
    },
    #[serde(rename = "thread_unsubscribed")]
    ThreadUnsubscribed {
        #[serde(default)]
        channel: Option<String>,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        thread_ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        subscription: Option<SlackWireThreadReference>,
    },
    #[serde(rename = "update_thread_state")]
    UpdateThreadState {
        #[serde(default)]
        channel_ids: Vec<String>,
    },
    #[serde(rename = "update_global_thread_state")]
    UpdateGlobalThreadState,
    #[serde(rename = "desync")]
    Desync {
        #[serde(default)]
        channel_ids: Vec<String>,
    },
    #[serde(rename = "resync")]
    Resync {
        #[serde(default)]
        channel_ids: Vec<String>,
    },
    #[serde(rename = "client.counts")]
    ClientCounts {
        #[serde(default)]
        channel_ids: Vec<String>,
    },
    #[serde(rename = "channel_created")]
    ChannelCreated { channel: SlackWireChannel },
    #[serde(rename = "channel_joined")]
    ChannelJoined { channel: SlackWireChannel },
    #[serde(rename = "channel_left")]
    ChannelLeft { channel: SlackWireChannel },
    #[serde(rename = "channel_archive")]
    ChannelArchive { channel: SlackWireChannel },
    #[serde(rename = "channel_unarchive")]
    ChannelUnarchive { channel: SlackWireChannel },
    #[serde(rename = "channel_rename")]
    ChannelRename { channel: SlackWireChannel },
    #[serde(rename = "channel_deleted")]
    ChannelDeleted { channel: SlackWireChannel },
    #[serde(rename = "channel_updated")]
    ChannelUpdated { channel: SlackWireChannel },
    #[serde(rename = "member_joined_channel")]
    MemberJoinedChannel { channel: String },
    #[serde(rename = "member_left_channel")]
    MemberLeftChannel { channel: String },
    #[serde(rename = "team_join")]
    TeamJoin,
    #[serde(rename = "user_change")]
    UserChange,
    #[serde(rename = "subteam_created")]
    SubteamCreated,
    #[serde(rename = "subteam_updated")]
    SubteamUpdated,
    #[serde(rename = "subteam_self_added")]
    SubteamSelfAdded,
    #[serde(rename = "subteam_self_removed")]
    SubteamSelfRemoved,
    #[serde(rename = "saved_item_added")]
    SavedItemAdded {
        #[serde(default)]
        item: Option<SlackWireSavedItem>,
    },
    #[serde(rename = "saved_item_deleted")]
    SavedItemDeleted {
        #[serde(default)]
        item: Option<SlackWireSavedItem>,
    },
    #[serde(rename = "saved_item_updated")]
    SavedItemUpdated {
        #[serde(default)]
        item: Option<SlackWireSavedItem>,
    },
    #[serde(rename = "file_created")]
    FileCreated {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_shared")]
    FileShared {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_unshared")]
    FileUnshared {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_deleted")]
    FileDeleted {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_change")]
    FileChange {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_public")]
    FilePublic {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(rename = "file_private")]
    FilePrivate {
        #[serde(default)]
        channel_id: Option<String>,
    },
    #[serde(other)]
    Ignored,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackWirePresenceChange {
    Batch {
        #[serde(default)]
        presence: Value,
        users: Vec<String>,
    },
    MalformedBatch {
        #[serde(rename = "users")]
        _users: Value,
    },
    Single {
        #[serde(default)]
        presence: Value,
        user: String,
    },
    Malformed {},
}

#[derive(Deserialize)]
struct SlackWireDesktopNotification {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, alias = "notificationId")]
    notification_id: Option<String>,
    #[serde(default, alias = "teamId")]
    team_id: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default, alias = "channelId")]
    channel_id: Option<String>,
    #[serde(default)]
    ts: Option<SlackMessageTimestamp>,
    #[serde(default)]
    thread_ts: Option<SlackMessageTimestamp>,
    #[serde(default)]
    event_ts: Option<SlackMessageTimestamp>,
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    msg: Option<SlackWireNotificationMessage>,
    #[serde(default, rename = "avatarImage")]
    avatar_image: Option<String>,
    #[serde(default, rename = "imageUri", alias = "icon_url")]
    icon_url: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default, alias = "senderUserId")]
    sender_user_id: Option<String>,
    #[serde(default, rename = "ssbFilename", alias = "sound")]
    sound: Option<String>,
    #[serde(default)]
    silent: bool,
    #[serde(default, rename = "hasReply", alias = "has_reply")]
    has_reply: bool,
    #[serde(default, rename = "launchUri", alias = "launch_uri")]
    launch_uri: Option<String>,
    #[serde(default)]
    actions: Vec<SlackWireNotificationAction>,
}

#[derive(Deserialize)]
struct SlackWireMessage {
    #[serde(default)]
    ts: Option<SlackMessageTimestamp>,
    #[serde(default)]
    thread_ts: Option<SlackMessageTimestamp>,
}

#[derive(Deserialize)]
struct SlackWireReactionItem {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    ts: Option<SlackMessageTimestamp>,
}

#[derive(Deserialize)]
struct SlackWireThreadReference {
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    thread_ts: Option<SlackMessageTimestamp>,
    #[serde(default)]
    ts: Option<SlackMessageTimestamp>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackWireChannel {
    Id(String),
    Record { id: String },
}

impl SlackWireChannel {
    fn into_id(self) -> String {
        match self {
            Self::Id(id) | Self::Record { id } => id,
        }
    }
}

#[derive(Deserialize)]
struct SlackWireSavedItem {
    #[serde(default)]
    channel_id: Option<String>,
}

pub(super) fn parse_slack_realtime_event(body: &str) -> Result<SlackRealtimeWireAction, String> {
    let event = serde_json::from_str::<SlackRealtimeWireEvent>(body)
        .map_err(|error| format!("Slack realtime event is invalid: {error}"))?;
    event.into_action()
}
