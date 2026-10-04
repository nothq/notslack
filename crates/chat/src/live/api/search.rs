mod quick;
mod wire;

pub(crate) use quick::{
    SlackQuickMessageRequest, SlackQuickSearchApiSnapshot, SlackQuickSearchMessageReference,
};

use crate::model::{
    SlackConversationKind, SlackMessageSearchRequest, SlackMessageSearchSort, SlackSearchSnapshot,
};
use uuid::Uuid;

use super::{messages::SlackMessagesListPurpose, SlackApiClient};
use wire::{SlackSearchResponseWire, SlackSearchSnapshotInput};

pub(super) const SLACK_SEARCH_MESSAGES_METHOD: &str = "search.modules.messages";
const SEARCH_PAGE_CURSOR_PREFIX: &str = "search-modules-page:";
type SlackSearchRequestParameters = Vec<(&'static str, String)>;

#[derive(Clone, Copy)]
pub(super) struct SlackSearchPageRequest {
    page: u32,
    search_session_id: Uuid,
}

impl SlackApiClient {
    pub(crate) fn search_messages(
        &self,
        request: &SlackMessageSearchRequest,
        team_id: &str,
        self_user_id: &str,
    ) -> Result<SlackSearchSnapshot, String> {
        let query = search_request_query(request)?;
        if query.is_empty() {
            return Err("Slack message search requires a query".to_string());
        }
        require_nonempty("team id", team_id)?;
        require_nonempty("self user id", self_user_id)?;
        let page_request = SlackSearchPageRequest::parse(request.cursor.as_deref())?;
        let parameters = search_request_parameters(&query, &request.options, page_request);
        let payload = self.post(SLACK_SEARCH_MESSAGES_METHOD, &parameters)?;
        let payload =
            serde_json::from_value::<SlackSearchResponseWire>(payload).map_err(|error| {
                format!("failed to decode Slack {SLACK_SEARCH_MESSAGES_METHOD} response: {error}")
            })?;
        let hydration = self.load_messages_list(
            payload.message_references()?,
            SlackMessagesListPurpose::Search,
        )?;
        let snapshot = payload.into_snapshot(
            SlackSearchSnapshotInput {
                page_request,
                team_id,
                self_user_id,
            },
            hydration,
        )?;
        profile_search_result_kinds(&snapshot);
        Ok(snapshot)
    }
}

fn search_request_parameters(
    query: &str,
    options: &crate::model::SlackMessageSearchOptions,
    page_request: SlackSearchPageRequest,
) -> SlackSearchRequestParameters {
    let sort = match options.sort {
        SlackMessageSearchSort::Relevant => "score",
        SlackMessageSearchSort::Recent => "timestamp",
    };
    vec![
        ("module", "messages".to_string()),
        ("query", query.to_string()),
        ("sort", sort.to_string()),
        ("sort_dir", "desc".to_string()),
        ("page", page_request.page.to_string()),
        ("client_req_id", Uuid::new_v4().to_string()),
        (
            "search_session_id",
            page_request.search_session_id.to_string(),
        ),
        ("highlight", "false".to_string()),
        (
            "search_exclude_bots",
            (!options.include_automations).to_string(),
        ),
        (
            "search_only_my_channels",
            options.only_my_channels.to_string(),
        ),
        ("_x_reason", "fetch-messages-results".to_string()),
        ("_x_mode", "online".to_string()),
        ("_x_sonic", "true".to_string()),
        ("_x_app_name", "client".to_string()),
    ]
}

fn search_request_query(request: &SlackMessageSearchRequest) -> Result<String, String> {
    let mut query = request.query.trim().to_string();
    append_search_refinement(&mut query, "from", request.options.from.as_deref())?;
    append_search_refinement(&mut query, "in", request.options.in_conversation.as_deref())?;
    Ok(query)
}

fn append_search_refinement(
    query: &mut String,
    modifier: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err(format!(
            "Slack message search {modifier} refinement is invalid"
        ));
    }
    query.push(' ');
    query.push_str(modifier);
    query.push(':');
    query.push_str(value);
    Ok(())
}

impl SlackSearchPageRequest {
    fn parse(cursor: Option<&str>) -> Result<Self, String> {
        let Some(cursor) = cursor else {
            return Ok(Self {
                page: 1,
                search_session_id: Uuid::new_v4(),
            });
        };
        let encoded = cursor
            .strip_prefix(SEARCH_PAGE_CURSOR_PREFIX)
            .ok_or_else(|| "invalid Slack message search page cursor".to_string())?;
        let (session_id, page) = encoded
            .split_once(':')
            .ok_or_else(|| "invalid Slack message search page cursor".to_string())?;
        let search_session_id = Uuid::parse_str(session_id)
            .map_err(|_| "invalid Slack message search page cursor".to_string())?;
        let page = page
            .parse::<u32>()
            .map_err(|_| "invalid Slack message search page cursor".to_string())?;
        if page == 0 {
            return Err("invalid Slack message search page cursor".to_string());
        }
        Ok(Self {
            page,
            search_session_id,
        })
    }

    pub(super) fn page(self) -> u32 {
        self.page
    }

    pub(super) fn next_cursor(self, next_page: u32) -> String {
        format!(
            "{SEARCH_PAGE_CURSOR_PREFIX}{}:{next_page}",
            self.search_session_id
        )
    }
}

fn require_nonempty(field: &str, value: &str) -> Result<(), String> {
    require_nonempty_for(SLACK_SEARCH_MESSAGES_METHOD, field, value)
}

pub(super) fn require_nonempty_for(method: &str, field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("Slack {method} response returned empty {field}"))
    } else {
        Ok(())
    }
}

fn require_slack_timestamp(field: &str, value: &str) -> Result<(), String> {
    require_slack_timestamp_for(SLACK_SEARCH_MESSAGES_METHOD, field, value)
}

pub(super) fn require_slack_timestamp_for(
    method: &str,
    field: &str,
    value: &str,
) -> Result<(), String> {
    require_nonempty_for(method, field, value)?;
    let Some((seconds, fractional_seconds)) = value.split_once('.') else {
        return Err(format!(
            "Slack {method} response returned malformed {field}"
        ));
    };
    if seconds.parse::<i64>().is_err()
        || fractional_seconds.is_empty()
        || fractional_seconds.len() > 9
        || !fractional_seconds.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "Slack {method} response returned malformed {field}"
        ));
    }
    Ok(())
}

fn profile_search_result_kinds(snapshot: &SlackSearchSnapshot) {
    if std::env::var_os("NOTSLACK_SLACK_API_CALL_PROFILE").is_none() {
        return;
    }
    let mut channels = 0;
    let mut private_channels = 0;
    let mut direct_messages = 0;
    let mut group_messages = 0;
    for message in &snapshot.messages {
        match message.conversation_kind {
            SlackConversationKind::Channel => channels += 1,
            SlackConversationKind::PrivateChannel => private_channels += 1,
            SlackConversationKind::DirectMessage => direct_messages += 1,
            SlackConversationKind::GroupMessage => group_messages += 1,
            SlackConversationKind::Unknown => {}
        }
    }
    eprintln!(
        "[notslack-slack-search-profile] total={} page={} channel={} private_channel={} \
         direct_message={} group_message={} has_next_cursor={}",
        snapshot.total,
        snapshot.messages.len(),
        channels,
        private_channels,
        direct_messages,
        group_messages,
        snapshot.next_cursor.is_some(),
    );
}
