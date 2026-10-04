use std::collections::HashSet;

use crate::model::{
    SlackConversationFilesPage, SlackConversationFilesPagination, SlackConversationFilesRequest,
    SlackConversationFilesSnapshot, SlackConversationFilesSort, SlackConversationLinkItem,
};
use serde::Deserialize;

use super::SlackLiveWorkspaceLoader;
use crate::live::internal_sidebar::SlackConversationFileSource;

const CONVERSATIONS_SEARCH_LINKS: &str = "conversations.searchLinks";

impl SlackLiveWorkspaceLoader {
    pub fn load_conversation_files(
        &self,
        request: SlackConversationFilesRequest,
    ) -> Result<SlackConversationFilesSnapshot, String> {
        validate_request(&request, &self.team_id)?;
        let files = request
            .files_page
            .map(|page| {
                self.sidebar_api.load_conversation_file_page(
                    &request,
                    SlackConversationFileSource::Files,
                    page,
                )
            })
            .transpose()?;
        let media = request
            .media_page
            .map(|page| {
                self.sidebar_api.load_conversation_file_page(
                    &request,
                    SlackConversationFileSource::Media,
                    page,
                )
            })
            .transpose()?;
        let links = request
            .links_page
            .map(|page| self.load_conversation_links_page(&request, page))
            .transpose()?;
        Ok(SlackConversationFilesSnapshot {
            team_id: request.team_id,
            conversation_id: request.conversation_id,
            search_query: request.search_query,
            sort: request.sort,
            files,
            media,
            links,
        })
    }

    fn load_conversation_links_page(
        &self,
        request: &SlackConversationFilesRequest,
        page: u32,
    ) -> Result<SlackConversationFilesPage<SlackConversationLinkItem>, String> {
        let (sort, sort_direction) = links_sort(request.sort);
        let payload = self.api.post(
            CONVERSATIONS_SEARCH_LINKS,
            &[
                ("channel_id", request.conversation_id.clone()),
                ("page", page.to_string()),
                ("query", request.search_query.trim().to_string()),
                ("sort", sort.to_string()),
                ("sort_dir", sort_direction.to_string()),
                ("_x_reason", "fetch-channel-links".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        let response =
            serde_json::from_value::<SlackConversationLinksResponse>(payload).map_err(|error| {
                format!("failed to decode Slack {CONVERSATIONS_SEARCH_LINKS} response: {error}")
            })?;
        let pagination = SlackConversationFilesPagination {
            total_count: response.pagination.total_count.unwrap_or(0),
            page_count: response.pagination.page_count.unwrap_or(1),
            page: response.pagination.page.unwrap_or(1),
        };
        if pagination.page != page {
            return Err(format!(
                "Slack {CONVERSATIONS_SEARCH_LINKS} returned page {} for requested page {page}",
                pagination.page
            ));
        }
        if pagination.page_count == 0 || pagination.page > pagination.page_count {
            return Err(format!(
                "Slack {CONVERSATIONS_SEARCH_LINKS} returned invalid pagination page {}/{}",
                pagination.page, pagination.page_count
            ));
        }
        let mut seen = HashSet::with_capacity(response.items.len());
        let items = response
            .items
            .into_iter()
            .map(SlackConversationLinkWire::into_item)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|item| seen.insert((item.timestamp.clone(), item.url.clone())))
            .collect();
        Ok(SlackConversationFilesPage { pagination, items })
    }
}

fn validate_request(
    request: &SlackConversationFilesRequest,
    runtime_team_id: &str,
) -> Result<(), String> {
    if request.team_id != runtime_team_id {
        return Err(format!(
            "Slack conversation Files request targeted team {} from runtime team {runtime_team_id}",
            request.team_id
        ));
    }
    if request.conversation_id.trim().is_empty() {
        return Err("Slack conversation Files request requires a conversation id".to_string());
    }
    if [request.files_page, request.media_page, request.links_page]
        .into_iter()
        .all(|page| page.is_none())
    {
        return Err("Slack conversation Files request requires at least one source".to_string());
    }
    if [request.files_page, request.media_page, request.links_page]
        .into_iter()
        .flatten()
        .any(|page| page == 0)
    {
        return Err("Slack conversation Files pages must be positive".to_string());
    }
    Ok(())
}

fn links_sort(sort: SlackConversationFilesSort) -> (&'static str, &'static str) {
    match sort {
        SlackConversationFilesSort::Newest => ("timestamp", "desc"),
        SlackConversationFilesSort::Oldest => ("timestamp", "asc"),
        SlackConversationFilesSort::Relevant => ("score", "desc"),
    }
}

#[derive(Deserialize)]
struct SlackConversationLinksResponse {
    items: Vec<SlackConversationLinkWire>,
    pagination: SlackConversationLinksPaginationWire,
}

#[derive(Deserialize)]
struct SlackConversationLinksPaginationWire {
    page: Option<u32>,
    total_count: Option<u32>,
    page_count: Option<u32>,
    #[serde(rename = "next_cursor")]
    _next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct SlackConversationLinkWire {
    url: String,
    title: Option<String>,
    icon_url: Option<String>,
    timestamp: SlackConversationLinkTimestampWire,
}

impl SlackConversationLinkWire {
    fn into_item(self) -> Result<SlackConversationLinkItem, String> {
        let url = self.url.trim().to_string();
        if url.is_empty() {
            return Err(format!(
                "Slack {CONVERSATIONS_SEARCH_LINKS} returned an empty link URL"
            ));
        }
        let timestamp = self.timestamp.into_string();
        if timestamp.trim().is_empty() {
            return Err(format!(
                "Slack {CONVERSATIONS_SEARCH_LINKS} returned an empty link timestamp"
            ));
        }
        let title = self
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| url.clone());
        let icon_url = self.icon_url.filter(|url| !url.trim().is_empty());
        Ok(SlackConversationLinkItem {
            url,
            title,
            icon_url,
            timestamp,
        })
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackConversationLinkTimestampWire {
    String(String),
    Integer(u64),
}

impl SlackConversationLinkTimestampWire {
    fn into_string(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Integer(value) => value.to_string(),
        }
    }
}
