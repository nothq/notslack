use std::{future::Future, pin::Pin};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::{SlackMessageTimestamp, SlackUserPresence};

pub const SLACK_REALTIME_PRESENCE_USER_LIMIT: usize = 65_536;
pub const SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES: usize = 128;
pub const SLACK_REALTIME_PRESENCE_PAYLOAD_MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackRealtimeConnectionState {
    Connecting,
    Connected,
    Reconnecting,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackRealtimeThreadTarget {
    pub conversation_id: String,
    pub thread_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackRealtimePresenceChange {
    pub user_id: String,
    pub presence: SlackUserPresence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackRealtimePresenceSnapshot {
    pub team_id: String,
    pub revision: u64,
    pub entries: Vec<SlackRealtimePresenceChange>,
}

impl SlackRealtimePresenceSnapshot {
    pub fn empty(team_id: String) -> Self {
        Self {
            team_id,
            revision: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackRealtimeNotificationAction {
    pub id: String,
    pub label: String,
    pub action_type: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackNotificationSound {
    None,
    AnimalStick,
    B2,
    BeenTree,
    Boop,
    CallsAlertV2,
    CallsConfirmationV2,
    CallsIncomingRingV2,
    CallsOutgoingRingV2,
    CallsPopV2,
    CallsTheyJoinedCallV2,
    CallsTheyLeftCallV2,
    CallsYouJoinedCallV2,
    CallsYouLeftCallV2,
    CompleteQuestRequirement,
    ConfirmDelivery,
    Flitterbug,
    HereYouGoLighter,
    HiFlowersHit,
    Hummus,
    ItemPickup,
    KnockBrush,
    SaveAndCheckout,
    ShortBoop,
}

impl SlackNotificationSound {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, String> {
        let value = value.as_ref().trim();
        match value {
            "none" => Ok(Self::None),
            "animal_stick.mp3" => Ok(Self::AnimalStick),
            "b2.mp3" => Ok(Self::B2),
            "been_tree.mp3" => Ok(Self::BeenTree),
            "boop.mp3" => Ok(Self::Boop),
            "calls_alert_v2.mp3" => Ok(Self::CallsAlertV2),
            "calls_confirmation_v2.mp3" => Ok(Self::CallsConfirmationV2),
            "calls_incoming_ring_v2.mp3" => Ok(Self::CallsIncomingRingV2),
            "calls_outgoing_ring_v2.mp3" => Ok(Self::CallsOutgoingRingV2),
            "calls_pop_v2.mp3" => Ok(Self::CallsPopV2),
            "calls_they_joined_call_v2.mp3" => Ok(Self::CallsTheyJoinedCallV2),
            "calls_they_left_call_v2.mp3" => Ok(Self::CallsTheyLeftCallV2),
            "calls_you_joined_call_v2.mp3" => Ok(Self::CallsYouJoinedCallV2),
            "calls_you_left_call_v2.mp3" => Ok(Self::CallsYouLeftCallV2),
            "complete_quest_requirement.mp3" => Ok(Self::CompleteQuestRequirement),
            "confirm_delivery.mp3" => Ok(Self::ConfirmDelivery),
            "flitterbug.mp3" => Ok(Self::Flitterbug),
            "here_you_go_lighter.mp3" => Ok(Self::HereYouGoLighter),
            "hi_flowers_hit.mp3" => Ok(Self::HiFlowersHit),
            "hummus.mp3" => Ok(Self::Hummus),
            "item_pickup.mp3" => Ok(Self::ItemPickup),
            "knock_brush.mp3" => Ok(Self::KnockBrush),
            "save_and_checkout.mp3" => Ok(Self::SaveAndCheckout),
            "short_boop.mp3" => Ok(Self::ShortBoop),
            "" => Err("Slack notification sound name is empty".to_string()),
            _ => Err(format!("Slack notification sound {value:?} is unsupported")),
        }
    }

    pub const fn slack_filename(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AnimalStick => "animal_stick.mp3",
            Self::B2 => "b2.mp3",
            Self::BeenTree => "been_tree.mp3",
            Self::Boop => "boop.mp3",
            Self::CallsAlertV2 => "calls_alert_v2.mp3",
            Self::CallsConfirmationV2 => "calls_confirmation_v2.mp3",
            Self::CallsIncomingRingV2 => "calls_incoming_ring_v2.mp3",
            Self::CallsOutgoingRingV2 => "calls_outgoing_ring_v2.mp3",
            Self::CallsPopV2 => "calls_pop_v2.mp3",
            Self::CallsTheyJoinedCallV2 => "calls_they_joined_call_v2.mp3",
            Self::CallsTheyLeftCallV2 => "calls_they_left_call_v2.mp3",
            Self::CallsYouJoinedCallV2 => "calls_you_joined_call_v2.mp3",
            Self::CallsYouLeftCallV2 => "calls_you_left_call_v2.mp3",
            Self::CompleteQuestRequirement => "complete_quest_requirement.mp3",
            Self::ConfirmDelivery => "confirm_delivery.mp3",
            Self::Flitterbug => "flitterbug.mp3",
            Self::HereYouGoLighter => "here_you_go_lighter.mp3",
            Self::HiFlowersHit => "hi_flowers_hit.mp3",
            Self::Hummus => "hummus.mp3",
            Self::ItemPickup => "item_pickup.mp3",
            Self::KnockBrush => "knock_brush.mp3",
            Self::SaveAndCheckout => "save_and_checkout.mp3",
            Self::ShortBoop => "short_boop.mp3",
        }
    }
}

impl Serialize for SlackNotificationSound {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.slack_filename())
    }
}

impl<'de> Deserialize<'de> for SlackNotificationSound {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackRealtimeUpstreamNotificationId(String);

impl SlackRealtimeUpstreamNotificationId {
    pub fn parse(value: String) -> Result<Self, String> {
        if value.is_empty() {
            return Err("upstream Slack notification ID must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackRealtimeUpstreamNotificationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SlackRealtimeNotificationSource {
    UpstreamNotificationId {
        id: SlackRealtimeUpstreamNotificationId,
    },
    EventTimestamp {
        timestamp: SlackMessageTimestamp,
    },
}

impl SlackRealtimeNotificationSource {
    pub fn upstream_notification_id(value: String) -> Result<Self, String> {
        Ok(Self::UpstreamNotificationId {
            id: SlackRealtimeUpstreamNotificationId::parse(value)?,
        })
    }

    pub fn event_timestamp(timestamp: SlackMessageTimestamp) -> Self {
        Self::EventTimestamp { timestamp }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackRealtimeNotification {
    pub source: SlackRealtimeNotificationSource,
    pub team_id: Option<String>,
    pub channel_id: String,
    pub message_timestamp: Option<SlackMessageTimestamp>,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub title: String,
    pub subtitle: Option<String>,
    pub body: String,
    pub icon_url: Option<String>,
    pub sender_user_id: Option<String>,
    pub sound: Option<SlackNotificationSound>,
    pub silent: bool,
    pub has_reply: bool,
    pub launch_uri: Option<String>,
    pub actions: Vec<SlackRealtimeNotificationAction>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackRealtimeBatch {
    pub conversation_ids: Vec<String>,
    pub threads: Vec<SlackRealtimeThreadTarget>,
    pub presence_changes: Vec<SlackRealtimePresenceChange>,
    pub presence_revision: Option<u64>,
    pub presence_snapshot: Option<SlackRealtimePresenceSnapshot>,
    pub notifications: Vec<SlackRealtimeNotification>,
    pub sidebar_changed: bool,
    pub activity_changed: bool,
    pub later_changed: bool,
    pub files_changed: bool,
    pub all_threads_changed: bool,
    pub full_resync: bool,
    pub connection_state: Option<SlackRealtimeConnectionState>,
}

impl SlackRealtimeBatch {
    pub fn resync() -> Self {
        Self {
            sidebar_changed: true,
            activity_changed: true,
            later_changed: true,
            files_changed: true,
            all_threads_changed: true,
            full_resync: true,
            ..Self::default()
        }
    }

    pub fn closed() -> Self {
        Self {
            connection_state: Some(SlackRealtimeConnectionState::Closed),
            ..Self::default()
        }
    }

    pub fn resync_with_presence(snapshot: SlackRealtimePresenceSnapshot) -> Self {
        Self {
            presence_revision: Some(snapshot.revision),
            presence_snapshot: Some(snapshot),
            ..Self::resync()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackRealtimeRecvError {
    Lagged,
    Closed,
}

pub type SlackRealtimeReceiveFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SlackRealtimeBatch, SlackRealtimeRecvError>> + Send + 'a>>;

pub trait SlackRealtimeSubscription: Send + 'static {
    fn recv(&mut self) -> SlackRealtimeReceiveFuture<'_>;
}
