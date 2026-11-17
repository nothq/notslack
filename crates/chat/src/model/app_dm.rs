use serde::{Deserialize, Serialize};

use super::SlackRichTextBody;

/// The app (bot) on the other side of a direct message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackDmApp {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_image_url: Option<String>,
}

/// `app_home` metadata returned by `conversations.info` with `return_app_home`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAppHome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default)]
    pub home_tab_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_view_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages_tab_enabled: Option<bool>,
}

/// Subset of `apps.profile.get`'s `app_profile`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAppProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_tab_enabled: Option<bool>,
}

/// App home metadata and profile for an app DM. Either half may be missing
/// when its request failed; the matching error explains why.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAppDmDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_home: Option<SlackAppHome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<SlackAppProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SlackAppDmDetails {
    pub fn home_tab_enabled(&self) -> bool {
        self.app_home
            .as_ref()
            .map(|home| home.home_tab_enabled)
            .or_else(|| self.profile.as_ref()?.home_tab_enabled)
            .unwrap_or(false)
    }

    pub fn home_view_id(&self) -> Option<&str> {
        self.app_home
            .as_ref()?
            .home_view_id
            .as_deref()
            .filter(|id| !id.is_empty())
    }
}

/// A published App Home view (`views.get`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackAppHomeView {
    pub view_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<SlackRichTextBody>,
}
