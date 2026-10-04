use std::str::FromStr;

use serde::{Deserialize, Serialize};

fn parse_nonempty_id(value: String, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("Chat {label} ID must not be empty"));
    }
    Ok(value.to_string())
}

macro_rules! chat_route_id {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = String;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                parse_nonempty_id(value, $label).map(Self)
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from(value.to_string())
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

chat_route_id!(ChatTeamId, "team");
chat_route_id!(ChatChannelId, "channel");
chat_route_id!(ChatTabId, "tab");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatLaunchRoute {
    team_id: ChatTeamId,
    channel_id: ChatChannelId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tab_id: Option<ChatTabId>,
}

impl ChatLaunchRoute {
    pub const fn new(
        team_id: ChatTeamId,
        channel_id: ChatChannelId,
        tab_id: Option<ChatTabId>,
    ) -> Self {
        Self {
            team_id,
            channel_id,
            tab_id,
        }
    }

    pub fn try_from_parts(
        team_id: String,
        channel_id: String,
        tab_id: Option<String>,
    ) -> Result<Self, String> {
        Ok(Self::new(
            ChatTeamId::try_from(team_id)?,
            ChatChannelId::try_from(channel_id)?,
            tab_id.map(ChatTabId::try_from).transpose()?,
        ))
    }

    pub const fn team_id(&self) -> &ChatTeamId {
        &self.team_id
    }

    pub const fn channel_id(&self) -> &ChatChannelId {
        &self.channel_id
    }

    pub const fn tab_id(&self) -> Option<&ChatTabId> {
        self.tab_id.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedSlackChannel {
    pub team_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub web_url: String,
    pub desktop_url: String,
}
