use std::collections::{HashMap, HashSet};

use crate::model::{
    SlackConversationKind, SlackConversationOpenReceipt, SlackConversationOpenRequest,
    SlackDestinationCandidate, SlackDestinationDirectorySnapshot, SlackDestinationTarget,
    SlackUserPresence,
};
use serde::Deserialize;
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::{
    select_slack_avatar_image_url,
    util::{slack_conversation_kind, slack_conversation_label, string_at},
    SlackAvatarImageUrls, SlackAvatarPurpose,
};

const SLACKBOT_USER_IDS: [&str; 2] = ["USLACK", "USLACKBOT"];

impl SlackLiveWorkspaceLoader {
    pub fn load_destination_directory(&self) -> Result<SlackDestinationDirectorySnapshot, String> {
        let (conversations_result, users_result, self_user_id_result) =
            std::thread::scope(|scope| {
                let conversations = scope.spawn(|| self.load_conversations());
                let users = scope.spawn(|| self.ensure_user_directory_cache());
                let self_user_id = scope.spawn(|| self.load_self_user_id());
                (
                    conversations
                        .join()
                        .map_err(|_| "Slack conversations directory thread panicked".to_string()),
                    users
                        .join()
                        .map_err(|_| "Slack users.list directory thread panicked".to_string()),
                    self_user_id
                        .join()
                        .map_err(|_| "Slack auth.test directory thread panicked".to_string()),
                )
            });
        let conversations = conversations_result??;
        users_result??;
        let self_user_id = self_user_id_result??;
        let users = self
            .user_cache
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?
            .clone();
        destination_directory_from_cache(&self.team_id, &self_user_id, &conversations, &users)
    }

    pub fn open_conversation(
        &self,
        request: SlackConversationOpenRequest,
    ) -> Result<SlackConversationOpenReceipt, String> {
        if request.team_id() != self.team_id {
            return Err(format!(
                "Slack conversations.open request targeted team {} from runtime team {}",
                request.team_id(),
                self.team_id
            ));
        }
        let response = self.api.post(
            "conversations.open",
            &[
                ("users", request.user_ids().join(",")),
                ("return_im", "true".to_string()),
            ],
        )?;
        let response =
            serde_json::from_value::<SlackConversationOpenResponse>(response).map_err(|error| {
                format!("failed to decode Slack conversations.open response: {error}")
            })?;
        SlackConversationOpenReceipt::new(
            request.team_id().to_string(),
            response.channel.id,
            request.user_ids().to_vec(),
        )
    }
}

fn destination_directory_from_cache(
    team_id: &str,
    self_user_id: &str,
    conversations: &Value,
    users: &HashMap<String, Value>,
) -> Result<SlackDestinationDirectorySnapshot, String> {
    let channels = conversations
        .get("channels")
        .and_then(Value::as_array)
        .ok_or_else(|| "Slack cached conversations missing channels array".to_string())?;
    let directory_users = users
        .iter()
        .map(|(user_id, user)| {
            serde_json::from_value::<SlackDirectoryUser>(user.clone())
                .map(|user| (user_id.clone(), user))
                .map_err(|error| {
                    format!("failed to decode cached Slack directory user {user_id}: {error}")
                })
        })
        .collect::<Result<HashMap<_, _>, _>>()?;

    let mut candidates = conversation_candidates(channels, &directory_users, users, self_user_id)?;
    candidates.extend(person_candidates(directory_users));
    candidates.sort_by(|left, right| {
        destination_sort_rank(&left.target)
            .cmp(&destination_sort_rank(&right.target))
            .then_with(|| {
                left.label
                    .to_ascii_lowercase()
                    .cmp(&right.label.to_ascii_lowercase())
            })
            .then_with(|| left.target.stable_id().cmp(right.target.stable_id()))
    });
    Ok(SlackDestinationDirectorySnapshot {
        team_id: team_id.to_string(),
        self_user_id: self_user_id.to_string(),
        candidates,
    })
}

fn conversation_candidates(
    channels: &[Value],
    users: &HashMap<String, SlackDirectoryUser>,
    raw_users: &HashMap<String, Value>,
    self_user_id: &str,
) -> Result<Vec<SlackDestinationCandidate>, String> {
    let mut candidates = Vec::new();
    let mut conversation_ids = HashSet::new();
    for channel in channels {
        if let Some(candidate) = conversation_candidate(
            channel,
            users,
            raw_users,
            self_user_id,
            &mut conversation_ids,
        )? {
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

fn conversation_candidate(
    channel: &Value,
    users: &HashMap<String, SlackDirectoryUser>,
    raw_users: &HashMap<String, Value>,
    self_user_id: &str,
    conversation_ids: &mut HashSet<String>,
) -> Result<Option<SlackDestinationCandidate>, String> {
    if channel.get("is_archived").and_then(Value::as_bool) == Some(true) {
        return Ok(None);
    }
    let kind = slack_conversation_kind(channel);
    if kind == SlackConversationKind::Unknown {
        return Ok(None);
    }
    let conversation_id = string_at(channel, &["id"])
        .ok_or_else(|| "Slack cached conversation is missing id".to_string())?;
    if !conversation_ids.insert(conversation_id.clone()) {
        return Ok(None);
    }
    let mut participant_user_ids = conversation_participant_user_ids(channel, kind, self_user_id);
    participant_user_ids.sort();
    participant_user_ids.dedup();
    let mut participant_labels = participant_user_ids
        .iter()
        .map(|user_id| {
            users
                .get(user_id)
                .map(SlackDirectoryUser::label)
                .unwrap_or_else(|| user_id.clone())
        })
        .collect::<Vec<_>>();
    participant_labels.sort_by_cached_key(|label| label.to_ascii_lowercase());
    let label = conversation_candidate_label(
        channel,
        raw_users,
        kind,
        &conversation_id,
        &participant_labels,
    );
    let direct_message_user = (kind == SlackConversationKind::DirectMessage)
        .then(|| participant_user_ids.first())
        .flatten()
        .and_then(|user_id| users.get(user_id));
    Ok(Some(SlackDestinationCandidate {
        target: SlackDestinationTarget::Conversation {
            conversation_id,
            kind,
        },
        label,
        real_name: direct_message_user.and_then(SlackDirectoryUser::real_name),
        email: direct_message_user.and_then(SlackDirectoryUser::email),
        avatar_image_url: direct_message_user
            .and_then(|user| user.avatar_image_url(SlackAvatarPurpose::Compact)),
        presence: direct_message_user.and_then(SlackDirectoryUser::presence),
        participant_user_ids,
        participant_labels,
    }))
}

fn conversation_candidate_label(
    channel: &Value,
    users: &HashMap<String, Value>,
    kind: SlackConversationKind,
    conversation_id: &str,
    participant_labels: &[String],
) -> String {
    if kind == SlackConversationKind::GroupMessage && !participant_labels.is_empty() {
        participant_labels.join(", ")
    } else {
        slack_conversation_label(channel, users).unwrap_or_else(|| conversation_id.to_string())
    }
}

fn person_candidates(
    directory_users: HashMap<String, SlackDirectoryUser>,
) -> Vec<SlackDestinationCandidate> {
    directory_users
        .into_iter()
        .filter(|(user_id, user)| {
            !SLACKBOT_USER_IDS.contains(&user_id.as_str())
                && !user.deleted
                && !user.is_bot
                && !user.is_app_user
        })
        .map(|(user_id, user)| SlackDestinationCandidate {
            target: SlackDestinationTarget::Person {
                user_id: user_id.clone(),
            },
            label: user.label(),
            real_name: user.real_name(),
            email: user.email(),
            avatar_image_url: user.avatar_image_url(SlackAvatarPurpose::Compact),
            presence: user.presence(),
            participant_user_ids: vec![user_id],
            participant_labels: Vec::new(),
        })
        .collect()
}

fn conversation_participant_user_ids(
    channel: &Value,
    kind: SlackConversationKind,
    self_user_id: &str,
) -> Vec<String> {
    match kind {
        SlackConversationKind::DirectMessage => string_at(channel, &["user"])
            .into_iter()
            .filter(|user_id| user_id != self_user_id)
            .collect(),
        SlackConversationKind::GroupMessage => channel
            .get("members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|user_id| *user_id != self_user_id)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn destination_sort_rank(target: &SlackDestinationTarget) -> u8 {
    match target {
        SlackDestinationTarget::Conversation { kind, .. } if kind.is_channel() => 0,
        SlackDestinationTarget::Conversation { .. } => 1,
        SlackDestinationTarget::Person { .. } => 2,
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct SlackDirectoryUser {
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    presence: Option<String>,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    is_bot: bool,
    #[serde(default)]
    is_app_user: bool,
    #[serde(default)]
    profile: SlackDirectoryUserProfile,
}

impl SlackDirectoryUser {
    fn label(&self) -> String {
        [
            self.profile.display_name.as_deref(),
            self.profile.real_name.as_deref(),
            self.real_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .unwrap_or("Unknown person")
        .to_string()
    }

    fn real_name(&self) -> Option<String> {
        self.profile
            .real_name
            .as_deref()
            .or(self.real_name.as_deref())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    }

    fn email(&self) -> Option<String> {
        self.profile
            .email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(str::to_string)
    }

    fn avatar_image_url(&self, purpose: SlackAvatarPurpose) -> Option<String> {
        select_slack_avatar_image_url(
            purpose,
            SlackAvatarImageUrls {
                image_24: self.profile.image_24.as_deref(),
                image_32: self.profile.image_32.as_deref(),
                image_48: self.profile.image_48.as_deref(),
                image_72: self.profile.image_72.as_deref(),
                image_192: self.profile.image_192.as_deref(),
                image_512: self.profile.image_512.as_deref(),
            },
        )
    }

    fn presence(&self) -> Option<SlackUserPresence> {
        match self.presence.as_deref() {
            Some("active") => Some(SlackUserPresence::Active),
            Some("away") => Some(SlackUserPresence::Away),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct SlackDirectoryUserProfile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    image_24: Option<String>,
    #[serde(default)]
    image_32: Option<String>,
    #[serde(default)]
    image_48: Option<String>,
    #[serde(default)]
    image_72: Option<String>,
    #[serde(default)]
    image_192: Option<String>,
    #[serde(default)]
    image_512: Option<String>,
}

#[derive(Deserialize)]
struct SlackConversationOpenResponse {
    channel: SlackConversationOpenChannel,
}

#[derive(Deserialize)]
struct SlackConversationOpenChannel {
    id: String,
}
