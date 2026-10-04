use std::{
    collections::{BTreeMap, HashMap, HashSet},
    thread,
};

use crate::model::SlackMessageTimestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SlackApiClient;

pub(crate) const SLACK_MESSAGES_LIST_METHOD: &str = "messages.list";
const MESSAGES_LIST_CHANNELS_PER_REQUEST: usize = 10;
const MESSAGES_LIST_CONCURRENT_REQUESTS: usize = 2;

#[derive(Clone, Copy)]
pub(crate) enum SlackMessagesListPurpose {
    Search,
    Activity,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackMessageReference {
    conversation_id: String,
    timestamp: SlackMessageTimestamp,
}

#[derive(Default)]
pub(crate) struct SlackMessagesListHydration {
    messages: HashMap<SlackMessageReference, Value>,
    remaining_consumers: HashMap<SlackMessageReference, usize>,
    returned_unrequested_message: bool,
}

#[derive(Clone, Serialize)]
struct SlackMessagesListRequestGroup {
    channel: String,
    timestamps: Vec<String>,
}

#[derive(Deserialize)]
struct SlackMessagesListResponseWire {
    messages_data: HashMap<String, SlackMessagesListChannelWire>,
}

#[derive(Deserialize)]
struct SlackMessagesListChannelWire {
    messages: Vec<Value>,
    #[serde(default)]
    unchanged_messages: Vec<String>,
}

impl SlackMessageReference {
    pub(crate) fn parse(conversation_id: &str, timestamp: &str) -> Result<Self, String> {
        if conversation_id.trim().is_empty() {
            return Err("Slack messages.list message reference requires a channel id".to_string());
        }
        Ok(Self {
            conversation_id: conversation_id.to_string(),
            timestamp: SlackMessageTimestamp::parse(timestamp).map_err(|error| {
                format!("Slack messages.list message reference has an invalid timestamp: {error}")
            })?,
        })
    }

    pub(crate) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub(crate) fn timestamp(&self) -> &SlackMessageTimestamp {
        &self.timestamp
    }
}

impl SlackMessagesListHydration {
    fn from_payloads(
        payloads: Vec<Value>,
        remaining_consumers: HashMap<SlackMessageReference, usize>,
    ) -> Result<Self, String> {
        let mut messages = HashMap::new();
        let mut returned_references = HashSet::new();
        let mut returned_unrequested_message = false;
        for payload in payloads {
            let payload = serde_json::from_value::<SlackMessagesListResponseWire>(payload)
                .map_err(|error| {
                    format!("failed to decode Slack {SLACK_MESSAGES_LIST_METHOD} response: {error}")
                })?;
            for (conversation_id, channel) in payload.messages_data {
                if conversation_id.trim().is_empty() {
                    return Err(format!(
                        "Slack {SLACK_MESSAGES_LIST_METHOD} response returned empty messages_data channel id"
                    ));
                }
                if !channel.unchanged_messages.is_empty() {
                    return Err(format!(
                        "Slack {SLACK_MESSAGES_LIST_METHOD} returned unchanged messages without cached latest updates"
                    ));
                }
                for message in channel.messages {
                    let timestamp = message.get("ts").and_then(Value::as_str).ok_or_else(|| {
                        format!(
                            "Slack {SLACK_MESSAGES_LIST_METHOD} response omitted messages_data.messages.ts"
                        )
                    })?;
                    let reference = SlackMessageReference::parse(&conversation_id, timestamp)?;
                    if !returned_references.insert(reference.clone()) {
                        return Err(format!(
                            "Slack {SLACK_MESSAGES_LIST_METHOD} returned a duplicate message"
                        ));
                    }
                    if remaining_consumers.contains_key(&reference) {
                        messages.insert(reference, message);
                    } else {
                        returned_unrequested_message = true;
                    }
                }
            }
        }
        Ok(Self {
            messages,
            remaining_consumers,
            returned_unrequested_message,
        })
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Value> {
        self.messages.values()
    }

    pub(crate) fn take(&mut self, reference: &SlackMessageReference) -> Result<Value, String> {
        let remaining_consumers = self
            .remaining_consumers
            .get(reference)
            .copied()
            .ok_or_else(|| {
                format!(
                    "Slack {SLACK_MESSAGES_LIST_METHOD} omitted requested message {}:{}",
                    reference.conversation_id(),
                    reference.timestamp().as_str()
                )
            })?;
        if remaining_consumers == 1 {
            self.remaining_consumers.remove(reference);
            return self.messages.remove(reference).ok_or_else(|| {
                format!(
                    "Slack {SLACK_MESSAGES_LIST_METHOD} omitted requested message {}:{}",
                    reference.conversation_id(),
                    reference.timestamp().as_str()
                )
            });
        }
        let message = self.messages.get(reference).cloned().ok_or_else(|| {
            format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} omitted requested message {}:{}",
                reference.conversation_id(),
                reference.timestamp().as_str()
            )
        })?;
        self.remaining_consumers
            .insert(reference.clone(), remaining_consumers - 1);
        Ok(message)
    }

    pub(crate) fn ensure_empty(self) -> Result<(), String> {
        if self.returned_unrequested_message {
            return Err(format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} returned unrequested messages"
            ));
        }
        if !self.remaining_consumers.is_empty() {
            return Err(format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} hydration left unconsumed requested message references"
            ));
        }
        if !self.messages.is_empty() {
            return Err(format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} hydration retained fully consumed messages"
            ));
        }
        Ok(())
    }
}

impl SlackApiClient {
    pub(crate) fn load_messages_list(
        &self,
        references: Vec<SlackMessageReference>,
        purpose: SlackMessagesListPurpose,
    ) -> Result<SlackMessagesListHydration, String> {
        let mut timestamps_by_channel = BTreeMap::<String, Vec<String>>::new();
        let mut remaining_consumers = HashMap::<SlackMessageReference, usize>::new();
        for reference in references {
            *remaining_consumers.entry(reference.clone()).or_default() += 1;
            let timestamps = timestamps_by_channel
                .entry(reference.conversation_id)
                .or_default();
            let timestamp = reference.timestamp.as_str();
            if !timestamps.iter().any(|existing| existing == timestamp) {
                timestamps.push(timestamp.to_string());
            }
        }
        let groups = timestamps_by_channel
            .into_iter()
            .map(|(channel, timestamps)| SlackMessagesListRequestGroup {
                channel,
                timestamps,
            })
            .collect::<Vec<_>>();
        if groups.is_empty() {
            return Ok(SlackMessagesListHydration::default());
        }

        let request_groups = groups
            .chunks(MESSAGES_LIST_CHANNELS_PER_REQUEST)
            .map(<[_]>::to_vec)
            .collect::<Vec<_>>();
        let payloads = thread::scope(|scope| {
            let mut payloads = Vec::with_capacity(request_groups.len());
            for concurrent_requests in request_groups.chunks(MESSAGES_LIST_CONCURRENT_REQUESTS) {
                let requests = concurrent_requests
                    .iter()
                    .cloned()
                    .map(|groups| {
                        let api = self.clone();
                        scope.spawn(move || api.load_messages_list_chunk(&groups))
                    })
                    .collect::<Vec<_>>();
                for request in requests {
                    payloads.push(
                        request
                            .join()
                            .map_err(|_| purpose.worker_panic_diagnostic().to_string())??,
                    );
                }
            }
            Ok::<_, String>(payloads)
        })?;
        SlackMessagesListHydration::from_payloads(payloads, remaining_consumers)
    }

    fn load_messages_list_chunk(
        &self,
        groups: &[SlackMessagesListRequestGroup],
    ) -> Result<Value, String> {
        let message_ids = serde_json::to_string(groups).map_err(|error| {
            format!("failed to encode Slack {SLACK_MESSAGES_LIST_METHOD} message ids: {error}")
        })?;
        self.post(
            SLACK_MESSAGES_LIST_METHOD,
            &[
                ("message_ids", message_ids),
                ("org_wide_aware", "true".to_string()),
                ("cached_latest_updates", "{}".to_string()),
                ("_x_reason", "messages-ufm".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )
    }
}

impl SlackMessagesListPurpose {
    fn worker_panic_diagnostic(self) -> &'static str {
        match self {
            Self::Search => "Slack messages.list search hydration request thread panicked",
            Self::Activity => "Slack messages.list Activity hydration request thread panicked",
        }
    }
}
