use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::model::{SlackConversationKind, SlackUserPresence};

pub const SLACK_NEW_MESSAGE_MAX_PEOPLE: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDestinationDirectorySnapshot {
    pub team_id: String,
    pub self_user_id: String,
    pub candidates: Vec<SlackDestinationCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDestinationCandidate {
    pub target: SlackDestinationTarget,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<SlackUserPresence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participant_user_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participant_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum SlackDestinationTarget {
    Conversation {
        conversation_id: String,
        kind: SlackConversationKind,
    },
    Person {
        user_id: String,
    },
}

impl SlackDestinationTarget {
    pub fn stable_id(&self) -> &str {
        match self {
            Self::Conversation {
                conversation_id, ..
            } => conversation_id,
            Self::Person { user_id } => user_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationOpenRequest {
    team_id: String,
    user_ids: Vec<String>,
}

impl SlackConversationOpenRequest {
    pub fn new(team_id: String, user_ids: Vec<String>) -> Result<Self, String> {
        parse_slack_id(&team_id, &['T'], "team")?;
        if !(1..=SLACK_NEW_MESSAGE_MAX_PEOPLE).contains(&user_ids.len()) {
            return Err(format!(
                "Slack conversations.open requires between 1 and {SLACK_NEW_MESSAGE_MAX_PEOPLE} people"
            ));
        }
        let mut unique_user_ids = HashSet::with_capacity(user_ids.len());
        for user_id in &user_ids {
            parse_slack_id(user_id, &['U', 'W'], "user")?;
            if !unique_user_ids.insert(user_id.as_str()) {
                return Err(format!(
                    "Slack conversations.open user list contains duplicate user id {user_id}"
                ));
            }
        }
        Ok(Self { team_id, user_ids })
    }

    pub fn team_id(&self) -> &str {
        &self.team_id
    }

    pub fn user_ids(&self) -> &[String] {
        &self.user_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationOpenReceipt {
    pub team_id: String,
    pub conversation_id: String,
    pub user_ids: Vec<String>,
}

impl SlackConversationOpenReceipt {
    pub fn new(
        team_id: String,
        conversation_id: String,
        user_ids: Vec<String>,
    ) -> Result<Self, String> {
        let request = SlackConversationOpenRequest::new(team_id, user_ids)?;
        parse_slack_id(&conversation_id, &['D', 'G'], "conversation")?;
        let expected_prefix = if request.user_ids.len() == 1 {
            'D'
        } else {
            'G'
        };
        if !conversation_id.starts_with(expected_prefix) {
            return Err(format!(
                "Slack conversations.open returned conversation {conversation_id} for {} selected people",
                request.user_ids.len()
            ));
        }
        Ok(Self {
            team_id: request.team_id,
            conversation_id,
            user_ids: request.user_ids,
        })
    }
}

pub(crate) fn parse_slack_id(
    value: &str,
    allowed_prefixes: &[char],
    kind: &str,
) -> Result<(), String> {
    let mut characters = value.chars();
    let prefix = characters
        .next()
        .ok_or_else(|| format!("Slack {kind} id must not be empty"))?;
    if !allowed_prefixes.contains(&prefix)
        || characters.clone().next().is_none()
        || !characters.all(|character| character.is_ascii_alphanumeric())
    {
        return Err(format!("invalid Slack {kind} id {value:?}"));
    }
    Ok(())
}
