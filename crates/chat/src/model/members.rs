use serde::{Deserialize, Serialize};

use crate::model::SlackUserPresence;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlackConversationMembersCursor(String);

impl SlackConversationMembersCursor {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err("Slack conversation-members cursor must not be empty".to_string());
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationMember {
    pub user_id: String,
    pub team_id: String,
    pub real_name: String,
    pub display_name: Option<String>,
    pub title: Option<String>,
    pub avatar_image_url: Option<String>,
    pub presence: Option<SlackUserPresence>,
    pub is_self: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConnectedOrganization {
    pub team_id: String,
    pub name: String,
    pub icon_image_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationMembersSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    pub members: Vec<SlackConversationMember>,
    pub external_organization_count: usize,
    pub connected_organizations: Vec<SlackConnectedOrganization>,
    pub next_cursor: Option<SlackConversationMembersCursor>,
}
