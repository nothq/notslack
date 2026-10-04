mod decode;
mod query;

use crate::model::{
    SlackConversationFilesPage, SlackConversationFilesPagination, SlackConversationFilesRequest,
    SlackFileItem, SlackFilesRequest, SlackFilesSnapshot,
};
use uuid::Uuid;

use super::SlackInternalSidebarClient;
use decode::{decode_files_page, decode_files_response};
use query::{
    conversation_files_query, conversation_files_sort, files_query, files_sort,
    validate_conversation_files_request, validate_files_request,
};

const SEARCH_MODULES_FILES: &str = "search.modules.files";
const FILES_PAGE_SIZE: u32 = 50;

#[derive(Clone, Copy)]
pub(crate) enum SlackConversationFileSource {
    Files,
    Media,
}

impl SlackInternalSidebarClient {
    pub(crate) fn load_files(
        &self,
        request: &SlackFilesRequest,
    ) -> Result<SlackFilesSnapshot, String> {
        validate_files_request(request)?;
        let query = files_query(request);
        let (sort, sort_direction, search_only_my_channels) = files_sort(request);
        let mut params = vec![
            ("module", "files".to_string()),
            ("query", query.clone()),
            ("page", request.page.to_string()),
            ("client_req_id", Uuid::new_v4().to_string()),
            (
                "browse_session_id",
                request.browser_session_id.as_str().to_string(),
            ),
            ("extracts", "1".to_string()),
            ("highlight", "1".to_string()),
            ("max_extract_len", "200".to_string()),
            ("extra_message_data", "1".to_string()),
            ("no_user_profile", "1".to_string()),
            ("count", FILES_PAGE_SIZE.to_string()),
            ("file_title_only", "false".to_string()),
            ("query_rewrite_disabled", "false".to_string()),
            ("include_files_shares", "1".to_string()),
            ("search_context", "desktop_files_browser".to_string()),
            ("max_filter_suggestions", "10".to_string()),
            ("sort", sort.to_string()),
            ("search_exclude_bots", "false".to_string()),
            (
                "search_only_my_channels",
                search_only_my_channels.to_string(),
            ),
            ("_x_reason", "fetch-current-browser".to_string()),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        if let Some(sort_direction) = sort_direction {
            params.push(("sort_dir", sort_direction.to_string()));
        }
        let body = self.post_internal_method(SEARCH_MODULES_FILES, params)?;
        decode_files_response(request, &query, &body)
    }

    pub(crate) fn load_conversation_file_page(
        &self,
        request: &SlackConversationFilesRequest,
        source: SlackConversationFileSource,
        page: u32,
    ) -> Result<SlackConversationFilesPage<SlackFileItem>, String> {
        validate_conversation_files_request(request, page)?;
        let query = conversation_files_query(request, source);
        let (sort, sort_direction) = conversation_files_sort(request.sort);
        let rich_search = !request.search_query.trim().is_empty();
        let body = self.post_internal_method(
            SEARCH_MODULES_FILES,
            vec![
                ("module", "files".to_string()),
                ("query", query.clone()),
                ("page", page.to_string()),
                ("client_req_id", Uuid::new_v4().to_string()),
                (
                    "browse_session_id",
                    request.browser_session_id.as_str().to_string(),
                ),
                ("extracts", u8::from(rich_search).to_string()),
                ("highlight", u8::from(rich_search).to_string()),
                ("max_extract_len", "200".to_string()),
                ("extra_message_data", u8::from(rich_search).to_string()),
                ("no_user_profile", "1".to_string()),
                ("count", FILES_PAGE_SIZE.to_string()),
                ("file_title_only", "false".to_string()),
                ("query_rewrite_disabled", "false".to_string()),
                ("include_files_shares", "1".to_string()),
                ("search_context", "files_channel_tab".to_string()),
                ("max_filter_suggestions", "10".to_string()),
                ("sort", sort.to_string()),
                ("sort_dir", sort_direction.to_string()),
                ("search_exclude_bots", "false".to_string()),
                ("search_only_my_channels", "false".to_string()),
                ("_x_reason", "fetch-channel-files".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        let (pagination, items) = decode_files_page(page, &query, &body)?;
        Ok(SlackConversationFilesPage {
            pagination: SlackConversationFilesPagination {
                total_count: pagination.total_count,
                page_count: pagination.page_count,
                page: pagination.page,
            },
            items,
        })
    }
}
