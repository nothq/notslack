use reqwest::header::HeaderValue;
use serde::Deserialize;
use url::Url;

use super::{SlackApiAuth, SlackApiClient};
use crate::live::SlackWebBuildTimestamp;

const SLACK_PRIMARY_WEBSOCKET_HOST: &str = "wss-primary.slack.com";
const SLACK_FALLBACK_WEBSOCKET_HOST: &str = "wss-backup.slack.com";

#[derive(Deserialize)]
struct SlackWebSocketUrlResponse {
    primary_websocket_url: String,
    fallback_websocket_url: String,
    #[serde(rename = "ttl_seconds")]
    _ttl_seconds: u64,
    routing_context: String,
}

#[derive(Clone)]
pub(in crate::live) struct SlackRealtimeSocketUrl(Url);

pub(in crate::live) struct SlackRealtimeSocketAuth(HeaderValue);

impl SlackRealtimeSocketAuth {
    pub(in crate::live) fn cookie_header(&self) -> &HeaderValue {
        &self.0
    }
}

impl SlackRealtimeSocketUrl {
    pub(in crate::live) fn parse_reconnect(raw_url: &str) -> Result<Self, String> {
        let url = Url::parse(raw_url)
            .map_err(|error| format!("Slack realtime reconnect URL is invalid: {error}"))?;
        validate_slack_websocket_url(&url, None)?;
        if url.query().is_none() {
            return Err("Slack realtime reconnect URL is missing its session query".to_string());
        }
        Ok(Self(url))
    }

    pub(in crate::live) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(in crate::live) struct SlackRealtimeSocketUrls {
    pub(in crate::live) primary: SlackRealtimeSocketUrl,
    pub(in crate::live) fallback: SlackRealtimeSocketUrl,
}

impl SlackApiClient {
    pub(in crate::live) fn realtime_socket_auth(&self) -> Result<SlackRealtimeSocketAuth, String> {
        match &self.auth {
            SlackApiAuth::Desktop { cookie_header, .. } => {
                Ok(SlackRealtimeSocketAuth(cookie_header.clone()))
            }
            SlackApiAuth::Public => Err(
                "Slack realtime events require authenticated Slack desktop credentials".to_string(),
            ),
        }
    }

    pub(in crate::live) fn realtime_socket_urls(&self) -> Result<SlackRealtimeSocketUrls, String> {
        let payload = self.post("client.getWebSocketURL", &[])?;
        let response =
            serde_json::from_value::<SlackWebSocketUrlResponse>(payload).map_err(|error| {
                format!("Slack client.getWebSocketURL response is invalid: {error}")
            })?;
        let (xoxc_token, web_build_timestamp) = match &self.auth {
            SlackApiAuth::Desktop {
                xoxc_token,
                web_build_timestamp,
                ..
            } => (xoxc_token.as_str(), web_build_timestamp),
            SlackApiAuth::Public => {
                return Err(
                    "Slack realtime events require authenticated Slack desktop credentials"
                        .to_string(),
                );
            }
        };
        let start_args = desktop_start_args(web_build_timestamp);
        Ok(SlackRealtimeSocketUrls {
            primary: desktop_socket_url(
                &response.primary_websocket_url,
                SLACK_PRIMARY_WEBSOCKET_HOST,
                xoxc_token,
                &response.routing_context,
                &start_args,
            )?,
            fallback: desktop_socket_url(
                &response.fallback_websocket_url,
                SLACK_FALLBACK_WEBSOCKET_HOST,
                xoxc_token,
                &response.routing_context,
                &start_args,
            )?,
        })
    }
}

fn desktop_socket_url(
    raw_url: &str,
    expected_host: &str,
    xoxc_token: &str,
    routing_context: &str,
    start_args: &str,
) -> Result<SlackRealtimeSocketUrl, String> {
    let mut url = Url::parse(raw_url)
        .map_err(|error| format!("Slack realtime endpoint URL is invalid: {error}"))?;
    validate_slack_websocket_url(&url, Some(expected_host))?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Slack realtime endpoint URL contains unexpected URL state".to_string());
    }
    url.query_pairs_mut()
        .append_pair("token", xoxc_token)
        .append_pair("sync_desync", "1")
        .append_pair("slack_client", "desktop")
        .append_pair("start_args", start_args)
        .append_pair("flannel", "3")
        .append_pair("lazy_channels", "1")
        .append_pair("gateway_server", routing_context)
        .append_pair("batch_presence_aware", "1")
        .finish();
    Ok(SlackRealtimeSocketUrl(url))
}

fn desktop_start_args(web_build_timestamp: &SlackWebBuildTimestamp) -> String {
    format!(
        "?agent=client&org_wide_aware=true&agent_version={}&eac_cache_ts=true&cache_ts=0&name_tagging=true&only_self_subteams=true&connect_only=true&ms_latest=true",
        web_build_timestamp.as_str(),
    )
}

fn validate_slack_websocket_url(url: &Url, expected_host: Option<&str>) -> Result<(), String> {
    if url.scheme() != "wss"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("Slack realtime endpoint must be an authenticated WSS URL".to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "Slack realtime endpoint is missing its host".to_string())?;
    let known_host = matches!(
        host,
        SLACK_PRIMARY_WEBSOCKET_HOST | SLACK_FALLBACK_WEBSOCKET_HOST
    );
    if !known_host || expected_host.is_some_and(|expected| host != expected) {
        return Err("Slack realtime endpoint uses an unexpected host".to_string());
    }
    Ok(())
}
