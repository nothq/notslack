use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use reqwest::{blocking::Client, header::HeaderValue};
use serde_json::Value;

use crate::live::{SlackWebBuildTimestamp, SlackWebSessionCredentials};

mod http;
mod huddle;
mod media;
pub(crate) mod messages;
mod observed;
mod reaction_catalog;
mod realtime;
mod search;
mod self_settings;

pub(in crate::live) use realtime::{SlackRealtimeSocketAuth, SlackRealtimeSocketUrl};

pub(crate) use http::load_remote_image;
use http::{
    build_http_client, build_media_http_client, slack_api_payload_from_response,
    slack_retry_after_seconds,
};
pub(crate) use search::{
    SlackQuickMessageRequest, SlackQuickSearchApiSnapshot, SlackQuickSearchMessageReference,
};

const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com/api";
pub(crate) const SLACK_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const SLACK_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const SLACK_MEDIA_READ_TIMEOUT: Duration = Duration::from_secs(30);

type SlackBytesResponse = (Vec<u8>, Option<String>);
type SlackApiCallCounts = Arc<Mutex<HashMap<String, usize>>>;

const SLACK_RATE_LIMIT_RETRY_ATTEMPTS: usize = 1;
const SLACK_RATE_LIMIT_MAX_RETRY_AFTER_SECS: u64 = 30;
const MAX_SLACK_REMOTE_IMAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct SlackApiClient {
    http: Client,
    base_url: String,
    auth: SlackApiAuth,
    call_counts: SlackApiCallCounts,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::live) enum SlackApiRequestError {
    AuthenticationRejected(String),
    Runtime(String),
}

impl SlackApiRequestError {
    fn into_message(self) -> String {
        match self {
            Self::AuthenticationRejected(message) | Self::Runtime(message) => message,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SlackMediaRequestMethod {
    Get,
    Head,
}

pub(crate) enum SlackObservedApiPost {
    Accepted(Value),
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

pub(crate) enum SlackObservedUpload {
    Transferred,
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
    Cancelled,
}

#[derive(Clone)]
enum SlackApiAuth {
    Desktop {
        xoxc_token: String,
        cookie_header: HeaderValue,
        web_build_timestamp: SlackWebBuildTimestamp,
        media_http: reqwest::Client,
    },
    Public,
}

impl SlackApiClient {
    pub(crate) fn desktop(web_session: &SlackWebSessionCredentials) -> Result<Self, String> {
        let mut cookie_header = HeaderValue::from_bytes(web_session.cookie_header().as_bytes())
            .map_err(|_| {
                "Slack Desktop cookie header is not a valid HTTP header value".to_string()
            })?;
        cookie_header.set_sensitive(true);
        Ok(Self {
            http: build_http_client()?,
            base_url: DEFAULT_SLACK_API_BASE_URL.to_string(),
            auth: SlackApiAuth::Desktop {
                xoxc_token: web_session.xoxc_token().to_string(),
                cookie_header,
                web_build_timestamp: web_session.web_build_timestamp().clone(),
                media_http: build_media_http_client()?,
            },
            call_counts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn public() -> Result<Self, String> {
        Ok(Self {
            http: build_http_client()?,
            base_url: DEFAULT_SLACK_API_BASE_URL.to_string(),
            auth: SlackApiAuth::Public,
            call_counts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn post(&self, method: &str, params: &[(&str, String)]) -> Result<Value, String> {
        self.post_with_error_provenance(method, params)
            .map_err(SlackApiRequestError::into_message)
    }

    pub(in crate::live) fn post_with_error_provenance(
        &self,
        method: &str,
        params: &[(&str, String)],
    ) -> Result<Value, SlackApiRequestError> {
        let url = format!("{}/{}", self.base_url, method);
        let mut retries_remaining = SLACK_RATE_LIMIT_RETRY_ATTEMPTS;

        loop {
            self.record_call(method)
                .map_err(SlackApiRequestError::Runtime)?;
            let mut form = params.to_vec();
            let mut request = self.http.post(&url);
            match &self.auth {
                SlackApiAuth::Desktop {
                    xoxc_token,
                    cookie_header,
                    web_build_timestamp,
                    ..
                } => {
                    request = request.header(reqwest::header::COOKIE, cookie_header.clone());
                    form.insert(
                        0,
                        ("_x_version_ts", web_build_timestamp.as_str().to_string()),
                    );
                    form.insert(0, ("token", xoxc_token.clone()));
                }
                SlackApiAuth::Public => {
                    return Err(SlackApiRequestError::Runtime(format!(
                        "Slack API request {method} requires authenticated Slack credentials"
                    )));
                }
            }
            let response = request.form(&form).send().map_err(|error| {
                SlackApiRequestError::Runtime(format!("Slack API request {method} failed: {error}"))
            })?;
            let status = response.status();
            let retry_after = slack_retry_after_seconds(response.headers());
            let body = response.text().map_err(|error| {
                SlackApiRequestError::Runtime(format!(
                    "failed to read Slack API response for {method}: {error}"
                ))
            })?;

            if status.as_u16() == 429
                && retries_remaining > 0
                && retry_after
                    .is_some_and(|seconds| seconds <= SLACK_RATE_LIMIT_MAX_RETRY_AFTER_SECS)
            {
                retries_remaining -= 1;
                thread::sleep(Duration::from_secs(retry_after.unwrap_or_default()));
                continue;
            }

            return slack_api_payload_from_response(method, status.as_u16(), retry_after, &body);
        }
    }

    pub(crate) fn call_counts_snapshot(&self) -> Result<HashMap<String, usize>, String> {
        self.call_counts
            .lock()
            .map_err(|_| "slack api call counts mutex poisoned".to_string())
            .map(|counts| counts.clone())
    }

    fn record_call(&self, method: &str) -> Result<(), String> {
        if std::env::var_os("NOTSLACK_SLACK_API_CALL_PROFILE").is_none() {
            return Ok(());
        }
        let count = {
            let mut call_counts = self
                .call_counts
                .lock()
                .map_err(|_| "slack api call counts mutex poisoned".to_string())?;
            let count = call_counts.entry(method.to_string()).or_default();
            *count += 1;
            *count
        };
        eprintln!("[notslack-slack-api-call-profile] method={method} count={count}");
        Ok(())
    }
}
