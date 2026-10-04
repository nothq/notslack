use std::{io::Read, time::Duration};

use base64::prelude::{Engine as _, BASE64_STANDARD};
use reqwest::{blocking::Client, redirect::Policy};
use serde_json::Value;

use super::{
    SlackApiClient, SlackApiRequestError, SlackBytesResponse, SLACK_HTTP_CONNECT_TIMEOUT,
    SLACK_HTTP_REQUEST_TIMEOUT, SLACK_MEDIA_READ_TIMEOUT,
};

pub(super) fn slack_url_requires_auth(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    slack_authenticated_host(url.host_str())
}

pub(super) fn slack_authenticated_https_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && slack_authenticated_host(url.host_str())
}

fn slack_authenticated_host(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return false;
    };
    host == "slack.com"
        || host.ends_with(".slack.com")
        || host.ends_with(".slack-edge.com")
        || host.ends_with(".slack-files.com")
        || host.ends_with(".slack-imgs.com")
}

pub(crate) fn load_remote_image(url: &str, timeout: Duration) -> Result<(String, String), String> {
    let api = SlackApiClient::public()?;
    let (bytes, content_type) = api.get_remote_image_bytes(url, timeout)?;
    let mimetype = content_type
        .filter(|value| value.starts_with("image/"))
        .or_else(|| slack_remote_image_mimetype_from_url(url))
        .ok_or_else(|| format!("failed to determine remote image mimetype for {url}"))?;
    Ok((BASE64_STANDARD.encode(bytes), mimetype))
}

pub(super) fn slack_api_payload_from_response(
    method: &str,
    status: u16,
    retry_after: Option<u64>,
    body: &str,
) -> Result<Value, SlackApiRequestError> {
    if status == 429 {
        return Err(SlackApiRequestError::Runtime(slack_rate_limit_message(
            method,
            retry_after,
        )));
    }
    let payload = serde_json::from_str::<Value>(body).map_err(|error| {
        SlackApiRequestError::Runtime(format!(
            "failed to decode Slack API response for {method}: {error}"
        ))
    })?;
    if !(200..300).contains(&status) {
        return Err(SlackApiRequestError::Runtime(format!(
            "Slack API {method} returned HTTP {status}"
        )));
    }
    if payload.get("ok").and_then(Value::as_bool) != Some(true) {
        let error = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        let message = format!("Slack API {method} failed: {error}");
        if method == "auth.test" && slack_authentication_rejection(error) {
            return Err(SlackApiRequestError::AuthenticationRejected(message));
        }
        return Err(SlackApiRequestError::Runtime(message));
    }
    Ok(payload)
}

fn slack_authentication_rejection(error: &str) -> bool {
    matches!(
        error,
        "not_authed"
            | "invalid_auth"
            | "token_revoked"
            | "token_expired"
            | "expired_token"
            | "account_inactive"
    )
}

pub(super) fn slack_retry_after_seconds(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

pub(super) fn slack_rate_limit_message(method: &str, retry_after: Option<u64>) -> String {
    match retry_after {
        Some(seconds) => {
            format!("Slack API {method} rate limited (HTTP 429; retry after {seconds}s)")
        }
        None => format!("Slack API {method} rate limited (HTTP 429)"),
    }
}

pub(super) fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("notslack/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(SLACK_HTTP_CONNECT_TIMEOUT)
        .timeout(SLACK_HTTP_REQUEST_TIMEOUT)
        .redirect(slack_redirect_policy())
        .build()
        .map_err(|error| format!("failed to build Slack API client: {error}"))
}

pub(super) fn build_media_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("notslack/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(SLACK_HTTP_CONNECT_TIMEOUT)
        .read_timeout(SLACK_MEDIA_READ_TIMEOUT)
        .no_gzip()
        .no_zstd()
        .redirect(slack_redirect_policy())
        .build()
        .map_err(|error| format!("failed to build Slack media client: {error}"))
}

fn slack_redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many HTTP redirects");
        }
        let authenticated_request = attempt
            .previous()
            .first()
            .is_some_and(slack_authenticated_https_url);
        if !authenticated_request || slack_authenticated_https_url(attempt.url()) {
            attempt.follow()
        } else {
            attempt.error("authenticated Slack request redirected outside trusted HTTPS hosts")
        }
    })
}

pub(super) fn get_bytes_with_request(
    request: reqwest::blocking::RequestBuilder,
    url: &str,
    max_bytes: usize,
) -> Result<SlackBytesResponse, String> {
    let mut response = request
        .send()
        .map_err(|error| format!("Slack file request failed for {url}: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Slack file request returned HTTP {} for {url}",
            status.as_u16()
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let bytes = read_response_bytes(&mut response, url, max_bytes)?;
    Ok((bytes, content_type))
}

fn read_response_bytes(
    response: &mut reqwest::blocking::Response,
    url: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|content_length| content_length > max_bytes as u64)
    {
        return Err(format!(
            "Slack file response exceeds {max_bytes} bytes for {url}"
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read Slack file body for {url}: {error}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "Slack file response exceeds {max_bytes} bytes for {url}"
        ));
    }
    Ok(bytes)
}

fn slack_remote_image_mimetype_from_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg".to_string())
    } else if lower.ends_with(".png") {
        Some("image/png".to_string())
    } else if lower.ends_with(".gif") {
        Some("image/gif".to_string())
    } else if lower.ends_with(".webp") {
        Some("image/webp".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::slack_url_requires_auth;
    use crate::live::api::{slack_api_payload_from_response, SlackApiRequestError};
    use serde_json::json;

    #[gpui::test]
    fn slack_url_auth_is_limited_to_slack_hosts() {
        assert!(slack_url_requires_auth(
            "https://files.slack.com/files-pri/T123-F456/image.png"
        ));
        assert!(slack_url_requires_auth(
            "https://ca.slack-edge.com/T123-U456/avatar.png"
        ));
        assert!(!slack_url_requires_auth("https://example.com/preview.png"));
    }

    #[gpui::test]
    fn slack_auth_rejection_payload_is_typed_at_the_response_boundary() {
        for rejection in [
            "not_authed",
            "invalid_auth",
            "token_revoked",
            "token_expired",
            "expired_token",
            "account_inactive",
        ] {
            let error = slack_api_payload_from_response(
                "auth.test",
                200,
                None,
                &json!({ "ok": false, "error": rejection }).to_string(),
            )
            .expect_err("expected Slack API payload error");

            assert_eq!(
                error,
                SlackApiRequestError::AuthenticationRejected(format!(
                    "Slack API auth.test failed: {rejection}"
                ))
            );
        }
    }

    #[gpui::test]
    fn auth_test_noncredential_failures_remain_runtime_errors() {
        let cases = [
            (
                200,
                None,
                json!({ "ok": false, "error": "missing_scope" }).to_string(),
                "missing_scope",
            ),
            (
                401,
                None,
                json!({ "ok": false, "error": "invalid_auth" }).to_string(),
                "HTTP 401",
            ),
            (200, None, "not-json".to_string(), "failed to decode"),
        ];

        for (status, retry_after, body, expected) in cases {
            let error = slack_api_payload_from_response("auth.test", status, retry_after, &body)
                .expect_err("noncredential auth.test failure should fail");
            let SlackApiRequestError::Runtime(message) = error else {
                panic!("noncredential auth.test failure must remain a runtime error");
            };
            assert!(message.contains(expected), "unexpected error: {message}");
        }
    }

    #[gpui::test]
    fn slack_api_payload_reports_retry_after_on_rate_limit() {
        let error = slack_api_payload_from_response("conversations.info", 429, Some(42), "")
            .expect_err("expected Slack API rate limit error");
        let SlackApiRequestError::Runtime(message) = error else {
            panic!("Slack API rate limiting must remain a runtime error");
        };

        assert!(message.contains("HTTP 429"));
        assert!(message.contains("retry after 42s"));
    }
}
