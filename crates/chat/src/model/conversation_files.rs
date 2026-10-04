use serde::{Deserialize, Serialize};

use crate::model::{SlackFileItem, SlackFilesBrowserSessionId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackConversationFilesFilter {
    #[default]
    All,
    Files,
    Media,
    Links,
}

impl SlackConversationFilesFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Files, Self::Media, Self::Links];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Files => "Files",
            Self::Media => "Media",
            Self::Links => "Links",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackConversationFilesSort {
    #[default]
    Newest,
    Oldest,
    Relevant,
}

impl SlackConversationFilesSort {
    pub const CHRONOLOGICAL: [Self; 2] = [Self::Newest, Self::Oldest];
    pub const SEARCH: [Self; 3] = [Self::Relevant, Self::Newest, Self::Oldest];

    pub fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest",
            Self::Oldest => "Oldest",
            Self::Relevant => "Relevant",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationFilesRequest {
    pub team_id: String,
    pub conversation_id: String,
    pub browser_session_id: SlackFilesBrowserSessionId,
    pub search_query: String,
    pub sort: SlackConversationFilesSort,
    pub files_page: Option<u32>,
    pub media_page: Option<u32>,
    pub links_page: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationFilesPagination {
    pub total_count: u32,
    pub page_count: u32,
    pub page: u32,
}

impl SlackConversationFilesPagination {
    pub fn next_page(&self) -> Option<u32> {
        (self.page < self.page_count).then(|| self.page + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationFilesPage<Item> {
    pub pagination: SlackConversationFilesPagination,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationLinkItem {
    pub url: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub timestamp: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackConversationFilesSnapshot {
    pub team_id: String,
    pub conversation_id: String,
    pub search_query: String,
    pub sort: SlackConversationFilesSort,
    pub files: Option<SlackConversationFilesPage<SlackFileItem>>,
    pub media: Option<SlackConversationFilesPage<SlackFileItem>>,
    pub links: Option<SlackConversationFilesPage<SlackConversationLinkItem>>,
}
