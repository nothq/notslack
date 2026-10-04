use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

use uuid::Uuid;

use crate::{
    live::api::SlackApiClient,
    model::{SlackQuickMessageQuery, SlackQuickMessageQueryScopeRef},
};

use super::{SlackQuickSearchMessageReference, SLACK_SEARCH_FIRST_PAGE};

mod permalink;
mod response;

use response::SlackQuickSearchInlineResponseWire;

pub(super) const SLACK_SEARCH_INLINE_METHOD: &str = "search.inline";
const SLACK_QUICK_SEARCH_MESSAGE_LIMIT: usize = 3;
const SLACK_QUICK_SEARCH_EXTRACT_LENGTH: usize = 110;
const SLACK_QUICK_SEARCH_EXTRACT_MAX_BYTES: usize = SLACK_QUICK_SEARCH_EXTRACT_LENGTH * 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackQuickMessageRequest {
    query: String,
    recent_channels: Vec<String>,
    scope: SlackQuickMessageRequestScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SlackQuickMessageRequestScope {
    Global,
    Channel(String),
    User {
        user_id: String,
        conversation_id: String,
    },
}

impl SlackQuickMessageRequest {
    pub(crate) fn resolve(
        query: &SlackQuickMessageQuery,
        recent_channels: &[String],
        user_conversation_id: Option<&str>,
    ) -> Option<Self> {
        let scope = match query.scope() {
            SlackQuickMessageQueryScopeRef::Global => SlackQuickMessageRequestScope::Global,
            SlackQuickMessageQueryScopeRef::Channel(conversation_id) => {
                SlackQuickMessageRequestScope::Channel(conversation_id.to_string())
            }
            SlackQuickMessageQueryScopeRef::User(user_id) => SlackQuickMessageRequestScope::User {
                user_id: user_id.to_string(),
                conversation_id: user_conversation_id?.to_string(),
            },
        };
        if matches!(scope, SlackQuickMessageRequestScope::Global) && recent_channels.is_empty() {
            return None;
        }
        let expected_conversation_id = match &scope {
            SlackQuickMessageRequestScope::Global => None,
            SlackQuickMessageRequestScope::Channel(conversation_id)
            | SlackQuickMessageRequestScope::User {
                conversation_id, ..
            } => Some(conversation_id.as_str()),
        };
        let mut seen = HashSet::with_capacity(recent_channels.len());
        let recent_channels = recent_channels
            .iter()
            .filter(|conversation_id| {
                !conversation_id.is_empty()
                    && expected_conversation_id
                        .is_none_or(|expected| conversation_id.as_str() == expected)
            })
            .filter(|conversation_id| seen.insert(conversation_id.as_str()))
            .cloned()
            .collect();
        Some(Self {
            query: query.text().to_string(),
            recent_channels,
            scope,
        })
    }

    pub(crate) fn cache_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|true",
            self.query,
            self.channel_id().unwrap_or_default(),
            self.user_id().unwrap_or_default(),
            self.recent_channels.join(","),
            self.searches_from_me(),
        )
    }

    fn query(&self) -> &str {
        &self.query
    }

    fn recent_channels(&self) -> &[String] {
        &self.recent_channels
    }

    fn channel_id(&self) -> Option<&str> {
        match &self.scope {
            SlackQuickMessageRequestScope::Channel(conversation_id) => Some(conversation_id),
            SlackQuickMessageRequestScope::Global | SlackQuickMessageRequestScope::User { .. } => {
                None
            }
        }
    }

    fn user_id(&self) -> Option<&str> {
        match &self.scope {
            SlackQuickMessageRequestScope::User { user_id, .. } => Some(user_id),
            SlackQuickMessageRequestScope::Global | SlackQuickMessageRequestScope::Channel(_) => {
                None
            }
        }
    }

    fn expected_conversation_id(&self) -> Option<&str> {
        match &self.scope {
            SlackQuickMessageRequestScope::Global => None,
            SlackQuickMessageRequestScope::Channel(conversation_id)
            | SlackQuickMessageRequestScope::User {
                conversation_id, ..
            } => Some(conversation_id),
        }
    }

    fn searches_from_me(&self) -> bool {
        matches!(self.scope, SlackQuickMessageRequestScope::Global)
    }
}

impl SlackApiClient {
    pub(crate) fn search_quick_message_references(
        &self,
        request: &SlackQuickMessageRequest,
    ) -> Result<Vec<SlackQuickSearchMessageReference>, String> {
        self.search_quick_messages(request, Uuid::new_v4())?
            .into_references(request.expected_conversation_id())
    }

    fn search_quick_messages(
        &self,
        request: &SlackQuickMessageRequest,
        search_session_id: Uuid,
    ) -> Result<SlackQuickSearchInlineResponseWire, String> {
        let max_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock predates the Unix epoch".to_string())?
            .as_secs();
        let mut parameters = vec![
            ("search_session_id", search_session_id.to_string()),
            ("client_req_id", Uuid::new_v4().to_string()),
            ("max_ts", max_timestamp.to_string()),
            ("count", SLACK_QUICK_SEARCH_MESSAGE_LIMIT.to_string()),
            ("page", SLACK_SEARCH_FIRST_PAGE.to_string()),
            ("query", request.query().to_string()),
            ("thread_replies", String::new()),
            ("extract_len", SLACK_QUICK_SEARCH_EXTRACT_LENGTH.to_string()),
            ("recent_channels", request.recent_channels().join(",")),
            ("from_me", request.searches_from_me().to_string()),
            ("with_me", "true".to_string()),
            ("_x_reason", "quick-messages/prototype".to_string()),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        if let Some(channel_id) = request.channel_id() {
            parameters.push(("channel", channel_id.to_string()));
        }
        if let Some(user_id) = request.user_id() {
            parameters.push(("user", user_id.to_string()));
        }
        let payload = self.post(SLACK_SEARCH_INLINE_METHOD, &parameters)?;
        let response = serde_json::from_value::<SlackQuickSearchInlineResponseWire>(payload)
            .map_err(|error| {
                format!("failed to decode Slack {SLACK_SEARCH_INLINE_METHOD} response: {error}")
            })?;
        response.validate(request.query())?;
        Ok(response)
    }
}
