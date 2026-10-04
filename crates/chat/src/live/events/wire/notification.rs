use serde::Deserialize;
use url::Url;

use crate::model::{
    SlackMessageTimestamp, SlackNotificationSound, SlackRealtimeBatch, SlackRealtimeNotification,
    SlackRealtimeNotificationAction, SlackRealtimeNotificationSource, SlackRealtimeThreadTarget,
};

use super::impact::conversations_impact;

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum SlackWireNotificationMessage {
    Text(String),
    Record {
        #[serde(default)]
        ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        thread_ts: Option<SlackMessageTimestamp>,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        user: Option<String>,
    },
}

struct SlackWireNotificationMessageParts {
    timestamp: Option<SlackMessageTimestamp>,
    thread_timestamp: Option<SlackMessageTimestamp>,
    body: Option<String>,
    sender_user_id: Option<String>,
}

impl SlackWireNotificationMessage {
    fn into_parts(self) -> SlackWireNotificationMessageParts {
        match self {
            Self::Text(text) => match SlackMessageTimestamp::parse(&text) {
                Ok(timestamp) => SlackWireNotificationMessageParts {
                    timestamp: Some(timestamp),
                    thread_timestamp: None,
                    body: None,
                    sender_user_id: None,
                },
                Err(_) => SlackWireNotificationMessageParts {
                    timestamp: None,
                    thread_timestamp: None,
                    body: Some(text),
                    sender_user_id: None,
                },
            },
            Self::Record {
                ts,
                thread_ts,
                text,
                content,
                user,
            } => SlackWireNotificationMessageParts {
                timestamp: ts,
                thread_timestamp: thread_ts,
                body: text.or(content),
                sender_user_id: user,
            },
        }
    }
}

#[derive(Deserialize)]
pub(super) struct SlackWireNotificationAction {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    action_id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    action_type: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

impl SlackWireNotificationAction {
    fn into_model(self) -> Option<SlackRealtimeNotificationAction> {
        Some(SlackRealtimeNotificationAction {
            id: self.id.or(self.action_id)?,
            label: self.label.or(self.title)?,
            action_type: self.action_type.or(self.kind),
            value: self.value,
        })
    }
}

pub(super) struct SlackNotificationImpactInput {
    pub(super) id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) channel_id: Option<String>,
    pub(super) message_timestamp: Option<SlackMessageTimestamp>,
    pub(super) thread_timestamp: Option<SlackMessageTimestamp>,
    pub(super) event_timestamp: Option<SlackMessageTimestamp>,
    pub(super) title: String,
    pub(super) subtitle: Option<String>,
    pub(super) content: Option<String>,
    pub(super) message: Option<SlackWireNotificationMessage>,
    pub(super) icon_url: Option<String>,
    pub(super) sender_user_id: Option<String>,
    pub(super) sound: Option<String>,
    pub(super) silent: bool,
    pub(super) has_reply: bool,
    pub(super) launch_uri: Option<String>,
    pub(super) actions: Vec<SlackWireNotificationAction>,
}

struct SlackNotificationImpact {
    notification: SlackRealtimeNotification,
    channel_id: String,
    thread_timestamp: Option<SlackMessageTimestamp>,
}

impl SlackNotificationImpactInput {
    fn into_impact(self) -> Result<SlackNotificationImpact, String> {
        let channel_id = self
            .channel_id
            .filter(|channel_id| !channel_id.is_empty())
            .ok_or_else(|| "Slack desktop notification is missing its channel".to_string())?;
        let message = self.message.map(SlackWireNotificationMessage::into_parts);
        let message_timestamp = self.message_timestamp.or_else(|| {
            message
                .as_ref()
                .and_then(|message| message.timestamp.clone())
        });
        let thread_timestamp = self.thread_timestamp.or_else(|| {
            message
                .as_ref()
                .and_then(|message| message.thread_timestamp.clone())
        });
        let body = message
            .as_ref()
            .and_then(|message| message.body.clone())
            .or(self.content)
            .ok_or_else(|| "Slack desktop notification is missing its body".to_string())?;
        if self.title.is_empty() {
            return Err("Slack desktop notification is missing its title".to_string());
        }
        let sender_user_id = self.sender_user_id.or_else(|| {
            message
                .as_ref()
                .and_then(|message| message.sender_user_id.clone())
        });
        let team_id = self
            .team_id
            .filter(|team_id| !team_id.is_empty())
            .or_else(|| slack_notification_team_id(self.launch_uri.as_deref()));
        let notification = SlackRealtimeNotification {
            source: notification_source(self.id, self.event_timestamp)?,
            team_id,
            channel_id: channel_id.clone(),
            message_timestamp,
            thread_timestamp: thread_timestamp.clone(),
            title: self.title,
            subtitle: self.subtitle,
            body,
            icon_url: self.icon_url,
            sender_user_id,
            sound: self.sound.map(SlackNotificationSound::parse).transpose()?,
            silent: self.silent,
            has_reply: self.has_reply,
            launch_uri: self.launch_uri,
            actions: self
                .actions
                .into_iter()
                .filter_map(SlackWireNotificationAction::into_model)
                .collect(),
        };
        Ok(SlackNotificationImpact {
            notification,
            channel_id,
            thread_timestamp,
        })
    }
}

pub(super) fn notification_impact(
    input: SlackNotificationImpactInput,
) -> Result<SlackRealtimeBatch, String> {
    let impact = input.into_impact()?;
    let mut batch = conversations_impact(vec![impact.channel_id.clone()]);
    batch.sidebar_changed = true;
    batch.activity_changed = true;
    if let Some(thread_timestamp) = impact.thread_timestamp {
        batch.all_threads_changed = true;
        batch.threads.push(SlackRealtimeThreadTarget {
            conversation_id: impact.channel_id,
            thread_timestamp,
        });
    }
    batch.notifications.push(impact.notification);
    Ok(batch)
}

fn notification_source(
    id: Option<String>,
    event_timestamp: Option<SlackMessageTimestamp>,
) -> Result<SlackRealtimeNotificationSource, String> {
    match id.filter(|id| !id.is_empty()) {
        Some(id) => SlackRealtimeNotificationSource::upstream_notification_id(id),
        None => Ok(SlackRealtimeNotificationSource::event_timestamp(
            event_timestamp.ok_or_else(|| {
                "Slack desktop notification is missing both its upstream notification ID and event timestamp"
                    .to_string()
            })?,
        )),
    }
}

fn slack_notification_team_id(launch_uri: Option<&str>) -> Option<String> {
    let launch_uri = Url::parse(launch_uri?).ok()?;
    launch_uri
        .query_pairs()
        .find_map(|(key, value)| (key == "team" && !value.is_empty()).then(|| value.into_owned()))
}
