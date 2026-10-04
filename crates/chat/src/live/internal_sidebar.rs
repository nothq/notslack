mod activity;
mod boot;
mod dms;
mod files;
mod later;
mod sections;
pub(crate) mod threads;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use reqwest::{blocking::Client, header::HeaderValue};
use serde_json::Value;

use crate::live::api::{SLACK_HTTP_CONNECT_TIMEOUT, SLACK_HTTP_REQUEST_TIMEOUT};
use crate::live::cache::SlackUserPayloadCache;
use crate::live::payload::sidebar_dom::SlackSidebarSnapshot;
use crate::live::SlackWebBuildTimestamp;

pub(crate) use self::activity::SlackActivitySnapshotHydrationInput;
pub(crate) use self::files::SlackConversationFileSource;
use self::{
    boot::{SlackBootClient, SlackBootState},
    sections::{SlackChannelSectionsClient, SlackChannelSectionsResponse},
};

const USERS_INFO: &str = "users.info";
const TEAM_INFO: &str = "team.info";

type SlackFallbackUserFetchLock = Arc<Mutex<()>>;
type SlackFallbackUserFetchLocks = Mutex<HashMap<String, SlackFallbackUserFetchLock>>;
type SlackExternalTeamLabelCache = Arc<Mutex<HashMap<String, String>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackWebSessionCredentials {
    xoxc_token: String,
    cookie_header: String,
    web_build_timestamp: SlackWebBuildTimestamp,
    draft_count: Option<u32>,
}

#[derive(Clone)]
pub struct SlackInternalSidebarClient {
    http: Client,
    team_id: String,
    team_domain: SlackTeamDomain,
    credentials: SlackWebSessionCredentials,
    dm_inbox_cache: Arc<dms::SlackDmInboxCache>,
    fallback_user_cache: Arc<SlackFallbackUserCache>,
    external_team_label_cache: SlackExternalTeamLabelCache,
}

#[derive(Clone)]
struct SlackFallbackUser {
    label: String,
    compact_avatar_image_url: Option<String>,
    message_avatar_image_url: Option<String>,
    raw_payload: Option<Value>,
}

#[derive(Default)]
struct SlackFallbackUserCache {
    users: Mutex<HashMap<String, SlackFallbackUser>>,
    fetch_locks: SlackFallbackUserFetchLocks,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackTeamDomain(String);

impl SlackWebSessionCredentials {
    pub fn new(
        xoxc_token: impl Into<String>,
        cookie_header: impl Into<String>,
        web_build_timestamp: SlackWebBuildTimestamp,
    ) -> Result<Self, String> {
        let xoxc_token = non_empty_secret(xoxc_token.into(), "Slack xoxc token")?;
        if !xoxc_token.starts_with("xoxc-") {
            return Err("Slack xoxc token must start with xoxc-".to_string());
        }
        let cookie_header = non_empty_secret(cookie_header.into(), "Slack cookie header")?;
        HeaderValue::from_bytes(cookie_header.as_bytes()).map_err(|_| {
            "Slack Desktop cookie header is not a valid HTTP header value".to_string()
        })?;
        Ok(Self {
            xoxc_token,
            cookie_header,
            web_build_timestamp,
            draft_count: None,
        })
    }

    pub fn with_draft_count(mut self, draft_count: u32) -> Self {
        self.draft_count = Some(draft_count);
        self
    }

    pub(crate) fn xoxc_token(&self) -> &str {
        &self.xoxc_token
    }

    pub(crate) fn cookie_header(&self) -> &str {
        &self.cookie_header
    }

    pub(crate) fn web_build_timestamp(&self) -> &SlackWebBuildTimestamp {
        &self.web_build_timestamp
    }

    fn draft_count(&self) -> Option<u32> {
        self.draft_count
    }
}

impl SlackInternalSidebarClient {
    pub(crate) fn new(
        team_id: &str,
        team_domain: SlackTeamDomain,
        credentials: SlackWebSessionCredentials,
    ) -> Result<Self, String> {
        let team_id = team_id.trim();
        if team_id.is_empty() {
            return Err("Slack sidebar team id must not be empty".to_string());
        }
        Ok(Self {
            http: Client::builder()
                .user_agent(format!("notslack/{}", env!("CARGO_PKG_VERSION")))
                .connect_timeout(SLACK_HTTP_CONNECT_TIMEOUT)
                .timeout(SLACK_HTTP_REQUEST_TIMEOUT)
                .build()
                .map_err(|error| format!("failed to build Slack sidebar HTTP client: {error}"))?,
            team_id: team_id.to_string(),
            team_domain,
            credentials,
            dm_inbox_cache: Arc::new(dms::SlackDmInboxCache::default()),
            fallback_user_cache: Arc::new(SlackFallbackUserCache::default()),
            external_team_label_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn load_sidebar_snapshot(
        &self,
        user_cache: &SlackUserPayloadCache,
    ) -> Result<SlackSidebarSnapshot, String> {
        let sections = SlackChannelSectionsClient::new(self).load()?;
        let boot = SlackBootClient::new(self).load(&sections, user_cache)?;
        let mut snapshot = SlackSidebarSnapshot::from_internal_boot_state(sections, boot)?;
        snapshot.draft_count = self.credentials.draft_count();
        Ok(snapshot)
    }

    pub(crate) fn create_channel_section(
        &self,
        request: &crate::model::SlackSidebarSectionCreateRequest,
    ) -> Result<(), String> {
        SlackChannelSectionsClient::new(self).create(request)
    }

    pub(crate) fn channel_permalink(&self, conversation_id: &str) -> Result<String, String> {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("Slack channel permalink requires a conversation id".to_string());
        }
        Ok(format!(
            "https://{}.slack.com/archives/{conversation_id}",
            self.team_domain.as_str()
        ))
    }

    fn load_fallback_user(&self, user_id: &str) -> Result<SlackFallbackUser, String> {
        if let Some(user) = self
            .fallback_user_cache
            .users
            .lock()
            .map_err(|_| "Slack fallback user cache mutex poisoned".to_string())?
            .get(user_id)
            .cloned()
        {
            return Ok(user);
        }
        let fetch_lock = self
            .fallback_user_cache
            .fetch_locks
            .lock()
            .map_err(|_| "Slack fallback user fetch-lock cache mutex poisoned".to_string())?
            .entry(user_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _fetch_guard = fetch_lock
            .lock()
            .map_err(|_| format!("Slack fallback user fetch mutex poisoned for {user_id}"))?;
        if let Some(user) = self
            .fallback_user_cache
            .users
            .lock()
            .map_err(|_| "Slack fallback user cache mutex poisoned".to_string())?
            .get(user_id)
            .cloned()
        {
            return Ok(user);
        }
        let body = self.post_internal_method(
            USERS_INFO,
            vec![
                ("user", user_id.to_string()),
                ("_x_app_name", "client".to_string()),
                ("_x_sonic", "true".to_string()),
            ],
        )?;
        let user = boot::decode_user_info(user_id, &body)?;
        self.fallback_user_cache
            .users
            .lock()
            .map_err(|_| "Slack fallback user cache mutex poisoned".to_string())?
            .insert(user_id.to_string(), user.clone());
        Ok(user)
    }

    fn load_external_team_label(&self, team_id: &str) -> Result<String, String> {
        if let Some(label) = self
            .external_team_label_cache
            .lock()
            .map_err(|_| "Slack external team label cache mutex poisoned".to_string())?
            .get(team_id)
            .cloned()
        {
            return Ok(label);
        }
        let body = self.post_internal_method(
            TEAM_INFO,
            vec![
                ("team", team_id.to_string()),
                ("_x_app_name", "client".to_string()),
                ("_x_sonic", "true".to_string()),
            ],
        )?;
        let label = boot::decode_team_info(team_id, &body)?;
        self.external_team_label_cache
            .lock()
            .map_err(|_| "Slack external team label cache mutex poisoned".to_string())?
            .insert(team_id.to_string(), label.clone());
        Ok(label)
    }

    fn post_internal_method(
        &self,
        method: &str,
        params: Vec<(&str, String)>,
    ) -> Result<String, String> {
        let url = format!(
            "https://{}.slack.com/api/{method}",
            self.team_domain.as_str()
        );
        let mut form = vec![
            ("token", self.credentials.xoxc_token().to_string()),
            (
                "_x_version_ts",
                self.credentials.web_build_timestamp().as_str().to_string(),
            ),
        ];
        form.extend(params);
        let response = self
            .http
            .post(&url)
            .query(&[("slack_route", self.team_id.as_str())])
            .header(reqwest::header::COOKIE, self.credentials.cookie_header())
            .form(&form)
            .send()
            .map_err(|error| format!("Slack internal API request {method} failed: {error}"))?;
        let status = response.status();
        let body = response.text().map_err(|error| {
            format!("failed to read Slack internal API {method} response: {error}")
        })?;
        if !status.is_success() {
            return Err(format!(
                "Slack internal API {method} returned HTTP {}",
                status.as_u16()
            ));
        }
        Ok(body)
    }
}

impl SlackTeamDomain {
    pub(crate) fn parse(raw: String) -> Result<Self, String> {
        let domain = raw.trim().trim_end_matches(".slack.com");
        if domain.is_empty() {
            return Err("Slack team domain must not be empty".to_string());
        }
        if !domain
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(format!("invalid Slack team domain: {raw}"));
        }
        Ok(Self(domain.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn non_empty_secret(value: String, label: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    Ok(value)
}

impl SlackSidebarSnapshot {
    fn from_internal_boot_state(
        sections: SlackChannelSectionsResponse,
        boot: SlackBootState,
    ) -> Result<Self, String> {
        boot::snapshot_from_boot_state(sections, boot)
    }
}

#[cfg(test)]
mod tests;
