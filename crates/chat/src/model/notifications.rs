use serde::{Deserialize, Serialize};

use super::{SlackMessageTimestamp, SlackRealtimeNotification};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatSurfaceEvent {
    DockBadgeChanged(SlackNotificationTeamBadge),
    NotificationWindowContextChanged(Option<SlackNotificationWindowContext>),
    AppearanceModeRequested(app_model::AppearanceMode),
    SlackSidebarRatioCommitted { ratio_basis_points: u16 },
    SlackWorkspaceSwitcherExpandedCommitted { expanded: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackNotificationWindowContext {
    pub team_id: String,
    pub active_surface: bool,
    pub visible_targets: Vec<SlackNotificationVisibleTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackNotificationVisibleTarget {
    Conversation {
        conversation_id: String,
    },
    Thread {
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
    },
    Message {
        conversation_id: String,
        message_timestamp: SlackMessageTimestamp,
    },
}

impl SlackNotificationWindowContext {
    pub fn displays(&self, notification: &SlackRealtimeNotification) -> bool {
        if !self.active_surface || notification.team_id.as_deref() != Some(self.team_id.as_str()) {
            return false;
        }
        self.visible_targets.iter().any(|target| match target {
            SlackNotificationVisibleTarget::Conversation { conversation_id } => {
                notification.thread_timestamp.is_none()
                    && notification.channel_id == *conversation_id
            }
            SlackNotificationVisibleTarget::Thread {
                conversation_id,
                thread_timestamp,
            } => {
                notification.channel_id == *conversation_id
                    && notification
                        .thread_timestamp
                        .as_ref()
                        .or(notification.message_timestamp.as_ref())
                        .is_some_and(|timestamp| {
                            timestamp.sort_key() == thread_timestamp.sort_key()
                        })
            }
            SlackNotificationVisibleTarget::Message {
                conversation_id,
                message_timestamp,
            } => {
                notification.channel_id == *conversation_id
                    && notification
                        .message_timestamp
                        .as_ref()
                        .is_some_and(|timestamp| {
                            timestamp.sort_key() == message_timestamp.sort_key()
                        })
            }
        })
    }
}

/// Where opening a notification navigates: a launch URI, or a conversation with an optional
/// message or thread inside it.
#[derive(Clone, Copy, Debug)]
pub struct SlackNotificationTarget<'a> {
    pub team_id: &'a str,
    pub conversation_id: &'a str,
    pub message_timestamp: Option<&'a str>,
    pub thread_timestamp: Option<&'a str>,
    pub launch_uri: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackNotificationTeamBadge {
    pub team_id: String,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlackChannelNotificationMode {
    Everything,
    Mentions,
    Nothing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackChannelNotificationPreference {
    pub team_id: String,
    pub conversation_id: String,
    pub desktop_mode: SlackChannelNotificationMode,
    pub mobile_mode: SlackChannelNotificationMode,
    pub muted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackChannelNotificationMutation {
    SetMode(SlackChannelNotificationMode),
    SetMuted(bool),
}
