use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackChannelText {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackChannelMembership {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_member: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_general: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_org_mandatory: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackChannelCreator {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_deactivated: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackChannelDetailsSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    pub name: String,
    pub text: SlackChannelText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<SlackChannelCreator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_seconds: Option<i64>,
    pub membership: SlackChannelMembership,
}
