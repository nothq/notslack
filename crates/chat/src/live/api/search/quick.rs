mod messages;
mod results;

pub(crate) use messages::SlackQuickMessageRequest;

use std::thread;

use serde::Deserialize;
use uuid::Uuid;

use crate::model::{
    SlackConversationKind, SlackMessageTimestamp, SlackQuickSearchConversation,
    SlackQuickSearchHighlight, SlackQuickSearchPerson,
};

use super::{
    require_nonempty_for, require_slack_timestamp_for, wire::strip_search_highlight_markers,
};
use crate::live::api::SlackApiClient;
use results::{SlackQuickSearchDmWire, SlackQuickSearchPersonWire};

const SLACK_SEARCH_CHANNELS_METHOD: &str = "search.modules.channels";
pub(super) const SLACK_SEARCH_PEOPLE_METHOD: &str = "search.modules.people";
pub(super) const SLACK_SEARCH_DMS_METHOD: &str = "search.modules.dms";
const SLACK_SEARCH_FIRST_PAGE: u32 = 1;
const SLACK_QUICK_SEARCH_AGGREGATE_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SlackQuickSearchModule {
    Channels,
    People,
    Dms,
}

impl SlackQuickSearchModule {
    fn as_str(self) -> &'static str {
        match self {
            Self::Channels => "channels",
            Self::People => "people",
            Self::Dms => "dms",
        }
    }

    fn method(self) -> &'static str {
        match self {
            Self::Channels => SLACK_SEARCH_CHANNELS_METHOD,
            Self::People => SLACK_SEARCH_PEOPLE_METHOD,
            Self::Dms => SLACK_SEARCH_DMS_METHOD,
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::Channels => "fetch-channels-results",
            Self::People => "fetch-people-results",
            Self::Dms => "fetch-direct-message-results",
        }
    }
}

#[derive(Deserialize)]
struct SlackQuickSearchResponseWire<T> {
    ok: bool,
    module: SlackQuickSearchModule,
    query: String,
    items: Vec<T>,
    pagination: SlackQuickSearchPaginationWire,
}

type SlackQuickSearchModuleResults = (
    Result<SlackQuickSearchResponseWire<SlackQuickSearchChannelWire>, String>,
    Result<SlackQuickSearchResponseWire<SlackQuickSearchPersonWire>, String>,
    Result<SlackQuickSearchResponseWire<SlackQuickSearchDmWire>, String>,
);

pub(crate) struct SlackQuickSearchApiSnapshot {
    pub(crate) query: String,
    pub(crate) channels: Vec<SlackQuickSearchConversation>,
    pub(crate) people: Vec<SlackQuickSearchPerson>,
    pub(crate) direct_messages: Vec<SlackQuickSearchConversation>,
}

#[derive(Clone)]
pub(crate) struct SlackQuickSearchMessageReference {
    pub(crate) id: String,
    pub(crate) conversation_id: String,
    pub(crate) user_id: Option<String>,
    pub(crate) bot_id: Option<String>,
    pub(crate) timestamp: SlackMessageTimestamp,
    pub(crate) thread_timestamp: Option<SlackMessageTimestamp>,
    pub(crate) excerpt: String,
    pub(crate) highlights: Vec<SlackQuickSearchHighlight>,
}

impl<T> SlackQuickSearchResponseWire<T> {
    fn validate(
        &self,
        method: &str,
        expected_module: SlackQuickSearchModule,
        expected_query: &str,
    ) -> Result<(), String> {
        if !self.ok {
            return Err(format!("Slack {method} returned ok=false"));
        }
        if self.module != expected_module {
            return Err(format!(
                "Slack {method} returned unexpected module {}",
                self.module.as_str()
            ));
        }
        require_nonempty_for(method, "query", &self.query)?;
        if self.query != expected_query {
            return Err(format!("Slack {method} returned a mismatched query"));
        }
        self.pagination.validate(method, self.items.len())
    }
}

#[derive(Deserialize)]
struct SlackQuickSearchPaginationWire {
    first: u32,
    last: u32,
    page: u32,
    page_count: u32,
    per_page: u32,
    total_count: u32,
}

impl SlackQuickSearchPaginationWire {
    fn validate(&self, method: &str, item_count: usize) -> Result<(), String> {
        if self.page != SLACK_SEARCH_FIRST_PAGE {
            return Err(format!("Slack {method} returned an unexpected page"));
        }
        if item_count > self.per_page as usize || item_count > self.total_count as usize {
            return Err(format!(
                "Slack {method} returned more items than pagination permits"
            ));
        }
        if self.total_count == 0 {
            if item_count != 0 || self.page_count != 0 || self.first != 0 || self.last != 0 {
                return Err(format!(
                    "Slack {method} returned inconsistent empty pagination"
                ));
            }
        } else {
            if self.per_page == 0 || item_count == 0 || self.first != 1 {
                return Err(format!("Slack {method} returned invalid pagination"));
            }
            let expected_last = u32::try_from(item_count)
                .map_err(|_| format!("Slack {method} returned too many pagination items"))?;
            let expected_page_count = self.total_count.div_ceil(self.per_page);
            if self.last != expected_last || self.page_count != expected_page_count {
                return Err(format!(
                    "Slack {method} returned inconsistent pagination bounds"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct SlackQuickSearchChannelWire {
    id: String,
    iid: String,
    context_team_id: String,
    is_member: bool,
    member_count: u32,
    name: String,
}

impl SlackQuickSearchChannelWire {
    fn into_channel(self) -> Result<SlackQuickSearchConversation, String> {
        require_nonempty_for(SLACK_SEARCH_CHANNELS_METHOD, "item.id", &self.id)?;
        require_nonempty_for(SLACK_SEARCH_CHANNELS_METHOD, "item.iid", &self.iid)?;
        require_nonempty_for(
            SLACK_SEARCH_CHANNELS_METHOD,
            "item.context_team_id",
            &self.context_team_id,
        )?;
        let kind = match self.id.as_bytes().first() {
            Some(b'C') => SlackConversationKind::Channel,
            Some(b'G') => SlackConversationKind::PrivateChannel,
            _ => {
                return Err(format!(
                    "Slack {SLACK_SEARCH_CHANNELS_METHOD} returned an invalid channel id"
                ));
            }
        };
        let label =
            strip_search_highlight_markers(SLACK_SEARCH_CHANNELS_METHOD, "item.name", self.name)?;
        require_nonempty_for(SLACK_SEARCH_CHANNELS_METHOD, "item.name", &label)?;
        Ok(SlackQuickSearchConversation {
            id: self.id,
            team_id: self.context_team_id,
            kind,
            label,
            is_member: self.is_member,
            user_id: None,
            member_user_ids: Vec::new(),
            member_count: Some(self.member_count),
        })
    }
}
impl SlackApiClient {
    pub(crate) fn search_quick_switch(
        &self,
        query: &str,
    ) -> Result<SlackQuickSearchApiSnapshot, String> {
        let query = query.trim();
        if query.is_empty() {
            return Err("Slack quick search requires a query".to_string());
        }
        let (channels, people, direct_messages) = self.search_quick_modules(query, Uuid::new_v4());
        let mut channels = channels?
            .items
            .into_iter()
            .map(SlackQuickSearchChannelWire::into_channel)
            .collect::<Result<Vec<_>, _>>()?;
        let mut people = people?
            .items
            .into_iter()
            .map(SlackQuickSearchPersonWire::into_person)
            .collect::<Result<Vec<_>, _>>()?;
        let mut direct_messages = direct_messages?
            .items
            .into_iter()
            .map(SlackQuickSearchDmWire::into_direct_message)
            .collect::<Result<Vec<_>, _>>()?;
        truncate_quick_search_modules(&mut channels, &mut people, &mut direct_messages);
        Ok(SlackQuickSearchApiSnapshot {
            query: query.to_string(),
            channels,
            people,
            direct_messages,
        })
    }

    fn search_quick_modules(
        &self,
        query: &str,
        search_session_id: Uuid,
    ) -> SlackQuickSearchModuleResults {
        thread::scope(|scope| {
            let channels = scope.spawn(|| {
                self.search_quick_module::<SlackQuickSearchChannelWire>(
                    query,
                    search_session_id,
                    SlackQuickSearchModule::Channels,
                )
            });
            let people = scope.spawn(|| {
                self.search_quick_module::<SlackQuickSearchPersonWire>(
                    query,
                    search_session_id,
                    SlackQuickSearchModule::People,
                )
            });
            let direct_messages = scope.spawn(|| {
                self.search_quick_module::<SlackQuickSearchDmWire>(
                    query,
                    search_session_id,
                    SlackQuickSearchModule::Dms,
                )
            });
            (
                join_search_module(channels, SLACK_SEARCH_CHANNELS_METHOD),
                join_search_module(people, SLACK_SEARCH_PEOPLE_METHOD),
                join_search_module(direct_messages, SLACK_SEARCH_DMS_METHOD),
            )
        })
    }

    fn search_quick_module<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        search_session_id: Uuid,
        module: SlackQuickSearchModule,
    ) -> Result<SlackQuickSearchResponseWire<T>, String> {
        let method = module.method();
        let reason = module.reason();
        let parameters = vec![
            ("module", module.as_str().to_string()),
            ("query", query.to_string()),
            ("page", SLACK_SEARCH_FIRST_PAGE.to_string()),
            ("client_req_id", Uuid::new_v4().to_string()),
            ("search_session_id", search_session_id.to_string()),
            ("highlight", "true".to_string()),
            ("_x_reason", reason.to_string()),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        let payload = self.post(method, &parameters)?;
        let response = serde_json::from_value::<SlackQuickSearchResponseWire<T>>(payload)
            .map_err(|error| format!("failed to decode Slack {method} response: {error}"))?;
        response.validate(method, module, query)?;
        Ok(response)
    }
}

fn truncate_quick_search_modules(
    channels: &mut Vec<SlackQuickSearchConversation>,
    people: &mut Vec<SlackQuickSearchPerson>,
    direct_messages: &mut Vec<SlackQuickSearchConversation>,
) {
    channels.truncate(SLACK_QUICK_SEARCH_AGGREGATE_LIMIT);
    people.truncate(SLACK_QUICK_SEARCH_AGGREGATE_LIMIT.saturating_sub(channels.len()));
    direct_messages.truncate(
        SLACK_QUICK_SEARCH_AGGREGATE_LIMIT
            .saturating_sub(channels.len())
            .saturating_sub(people.len()),
    );
}

fn join_search_module<T>(
    handle: thread::ScopedJoinHandle<'_, Result<T, String>>,
    method: &str,
) -> Result<T, String> {
    handle
        .join()
        .map_err(|_| format!("Slack {method} request worker panicked"))?
}
