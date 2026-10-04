use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBookmarkFolderRequest {
    pub team_id: String,
    pub conversation_id: String,
    pub folder_bookmark_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBookmarkFolderSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    pub folder_bookmark_id: String,
    pub items: Vec<SlackBookmarkFolderItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBookmarkFolderItem {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
    pub icon_url: Option<String>,
    pub source: SlackBookmarkSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlackBookmarkSource {
    Link {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity_id: Option<String>,
    },
    File {
        file_id: String,
    },
    PinnedMessage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity_id: Option<String>,
    },
    Unsupported {
        source_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity_id: Option<String>,
    },
}
