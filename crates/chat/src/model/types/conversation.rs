use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackConversationKind {
    Channel,
    PrivateChannel,
    DirectMessage,
    GroupMessage,
    #[default]
    Unknown,
}

impl SlackConversationKind {
    pub fn is_channel(self) -> bool {
        matches!(self, Self::Channel | Self::PrivateChannel)
    }

    pub fn composer_placeholder(self, label: &str) -> String {
        if self.is_channel() {
            format!("Message #{label}")
        } else {
            format!("Message to {label}")
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackUserPresence {
    Active,
    Away,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackRailBadges {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafts_sent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dms_unread_messages: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub later: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub more: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub admin_visible: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub admin_attention: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_presence: Option<SlackUserPresence>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub self_notifications_paused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationTab {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub target: SlackConversationTabTarget,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_disabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlackConversationTabTarget {
    Canvas {
        file_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shared_timestamp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permalink: Option<String>,
    },
    Folder {
        bookmark_id: String,
    },
    Files,
    Pins,
    Unsupported {
        source_type: String,
    },
}

impl SlackConversationTab {
    pub fn is_canvas(&self) -> bool {
        matches!(self.target, SlackConversationTabTarget::Canvas { .. })
    }

    pub fn is_files(&self) -> bool {
        self.target == SlackConversationTabTarget::Files
    }

    pub fn is_pins(&self) -> bool {
        self.target == SlackConversationTabTarget::Pins
    }

    pub fn merge_resolved_metadata_from(&mut self, existing: &Self) {
        let SlackConversationTabTarget::Canvas {
            file_id,
            title,
            permalink,
            ..
        } = &mut self.target
        else {
            return;
        };
        let SlackConversationTabTarget::Canvas {
            file_id: existing_file_id,
            title: existing_title,
            permalink: existing_permalink,
            ..
        } = &existing.target
        else {
            return;
        };
        if file_id != existing_file_id {
            return;
        }
        if title.is_none() {
            title.clone_from(existing_title);
        }
        if permalink.is_none() {
            permalink.clone_from(existing_permalink);
        }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}
