use super::range::SlackMediaRange;
use crate::live::runtime::media_proxy::SlackMediaContentProfile;
use axum::{
    body::Body,
    http::{
        header::{ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::Response,
};

const SLACK_MEDIA_MAX_BYTES: u64 = 1024 * 1024 * 1024;

pub(super) struct SlackMediaResponseHead {
    pub(super) status: StatusCode,
    pub(super) body_limit: u64,
    pub(super) expected_body_length: Option<u64>,
    headers: HeaderMap,
}

struct SlackSatisfiedContentRange {
    start: u64,
    end: u64,
    total: u64,
}

struct SlackMediaBodyLengths {
    body_limit: u64,
    expected: Option<u64>,
}

pub(super) fn validate_slack_media_response(
    response: &reqwest::Response,
    content_profile: SlackMediaContentProfile,
    requested_range: Option<&SlackMediaRange>,
) -> Result<SlackMediaResponseHead, String> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| format!("Slack media returned an invalid HTTP status: {error}"))?;
    if status == StatusCode::RANGE_NOT_SATISFIABLE {
        return unsatisfied_range_response(response, requested_range);
    }
    validate_response_status(status, requested_range, response.headers())?;
    let content_type = response_content_type(response.headers())?;
    validate_slack_media_content_type(content_profile, content_type)?;
    let content_length = parse_optional_content_length(response.headers())?;
    let body_lengths =
        response_body_lengths(status, requested_range, response.headers(), content_length)?;
    Ok(slack_media_response_head(
        status,
        response.headers(),
        true,
        body_lengths.body_limit,
        body_lengths.expected,
    ))
}

fn unsatisfied_range_response(
    response: &reqwest::Response,
    requested_range: Option<&SlackMediaRange>,
) -> Result<SlackMediaResponseHead, String> {
    let requested_range = requested_range
        .ok_or_else(|| "Slack media returned an unsolicited unsatisfied range".to_string())?;
    let content_range = required_header(response.headers(), CONTENT_RANGE)?;
    let total = parse_unsatisfied_content_range(content_range)?;
    validate_slack_media_total_size(total)?;
    requested_range.validate_unsatisfied(total)?;
    Ok(slack_media_response_head(
        StatusCode::RANGE_NOT_SATISFIABLE,
        response.headers(),
        false,
        0,
        None,
    ))
}

fn validate_response_status(
    status: StatusCode,
    requested_range: Option<&SlackMediaRange>,
    headers: &reqwest::header::HeaderMap,
) -> Result<(), String> {
    match (status, requested_range) {
        (StatusCode::OK, None) => {
            if headers.contains_key(CONTENT_RANGE) {
                Err("Slack media returned Content-Range with a full response".to_string())
            } else {
                Ok(())
            }
        }
        (StatusCode::OK, Some(_)) => {
            Err("Slack media ignored the requested byte range".to_string())
        }
        (StatusCode::PARTIAL_CONTENT, Some(_)) => Ok(()),
        (StatusCode::PARTIAL_CONTENT, None) => {
            Err("Slack media returned an unsolicited partial response".to_string())
        }
        _ => Err(format!("Slack media returned HTTP {}", status.as_u16())),
    }
}

fn response_content_type(headers: &reqwest::header::HeaderMap) -> Result<&str, String> {
    required_header(headers, CONTENT_TYPE)?
        .split(';')
        .next()
        .map(str::trim)
        .filter(|content_type| !content_type.is_empty())
        .ok_or_else(|| "Slack media returned an empty content type".to_string())
}

fn response_body_lengths(
    status: StatusCode,
    requested_range: Option<&SlackMediaRange>,
    headers: &reqwest::header::HeaderMap,
    content_length: Option<u64>,
) -> Result<SlackMediaBodyLengths, String> {
    if status != StatusCode::PARTIAL_CONTENT {
        return match content_length {
            Some(content_length) => {
                validate_slack_media_total_size(content_length)?;
                Ok(SlackMediaBodyLengths {
                    body_limit: content_length,
                    expected: Some(content_length),
                })
            }
            None => Ok(SlackMediaBodyLengths {
                body_limit: SLACK_MEDIA_MAX_BYTES,
                expected: None,
            }),
        };
    }
    let requested_range =
        requested_range.expect("partial Slack media response must have a requested range");
    let content_range = parse_satisfied_content_range(required_header(headers, CONTENT_RANGE)?)?;
    validate_slack_media_total_size(content_range.total)?;
    requested_range.validate_satisfied(
        content_range.start,
        content_range.end,
        content_range.total,
    )?;
    let expected_length = content_range
        .end
        .checked_sub(content_range.start)
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| "Slack media returned an invalid byte range".to_string())?;
    if content_length.is_some_and(|content_length| content_length != expected_length) {
        return Err(format!(
            "Slack media Content-Length {} does not match Content-Range length {expected_length}",
            content_length.expect("mismatched content length must be present")
        ));
    }
    Ok(SlackMediaBodyLengths {
        body_limit: expected_length,
        expected: Some(expected_length),
    })
}

pub(super) fn response_with_head(head: SlackMediaResponseHead, body: Body) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = head.status;
    *response.headers_mut() = head.headers;
    response
}

pub(super) fn empty_response(status: StatusCode) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = status;
    response
}

fn slack_media_response_head(
    status: StatusCode,
    upstream: &reqwest::header::HeaderMap,
    include_entity_headers: bool,
    body_limit: u64,
    expected_body_length: Option<u64>,
) -> SlackMediaResponseHead {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    if matches!(
        status,
        StatusCode::PARTIAL_CONTENT | StatusCode::RANGE_NOT_SATISFIABLE
    ) {
        if let Some(value) = upstream.get(CONTENT_RANGE) {
            headers.insert(CONTENT_RANGE, value.clone());
        }
    }
    if include_entity_headers {
        if let Some(value) = upstream.get(CONTENT_LENGTH) {
            headers.insert(CONTENT_LENGTH, value.clone());
        }
        if let Some(value) = upstream.get(CONTENT_TYPE) {
            headers.insert(CONTENT_TYPE, value.clone());
        }
    }
    SlackMediaResponseHead {
        status,
        body_limit,
        expected_body_length,
        headers,
    }
}

fn validate_slack_media_content_type(
    content_profile: SlackMediaContentProfile,
    content_type: &str,
) -> Result<(), String> {
    if is_generic_binary_content_type(content_type)
        || content_profile.accepts_response_content_type(content_type)
    {
        return Ok(());
    }
    Err(format!(
        "Slack media response type {content_type} does not match {content_profile:?}"
    ))
}

fn is_generic_binary_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "application/octet-stream" | "binary/octet-stream"
    )
}

fn parse_optional_content_length(
    headers: &reqwest::header::HeaderMap,
) -> Result<Option<u64>, String> {
    headers
        .get(CONTENT_LENGTH)
        .map(|value| {
            value
                .to_str()
                .map_err(|error| {
                    format!("Slack media returned an invalid Content-Length: {error}")
                })?
                .parse::<u64>()
                .map_err(|error| format!("Slack media returned an invalid Content-Length: {error}"))
        })
        .transpose()
}

fn required_header(
    headers: &reqwest::header::HeaderMap,
    name: reqwest::header::HeaderName,
) -> Result<&str, String> {
    headers
        .get(&name)
        .ok_or_else(|| format!("Slack media response omitted {name}"))?
        .to_str()
        .map_err(|error| format!("Slack media returned an invalid {name}: {error}"))
}

fn parse_satisfied_content_range(value: &str) -> Result<SlackSatisfiedContentRange, String> {
    let value = value
        .strip_prefix("bytes ")
        .ok_or_else(|| "Slack media returned a non-byte Content-Range".to_string())?;
    let (range, total) = value
        .split_once('/')
        .ok_or_else(|| "Slack media returned an invalid Content-Range".to_string())?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| "Slack media returned an invalid Content-Range".to_string())?;
    let start = start
        .parse::<u64>()
        .map_err(|error| format!("Slack media returned an invalid range start: {error}"))?;
    let end = end
        .parse::<u64>()
        .map_err(|error| format!("Slack media returned an invalid range end: {error}"))?;
    let total = parse_content_range_total(total)?;
    if start > end || end >= total {
        return Err("Slack media returned an out-of-bounds Content-Range".to_string());
    }
    Ok(SlackSatisfiedContentRange { start, end, total })
}

fn parse_unsatisfied_content_range(value: &str) -> Result<u64, String> {
    let total = value
        .strip_prefix("bytes */")
        .ok_or_else(|| "Slack media returned an invalid unsatisfied Content-Range".to_string())?;
    parse_content_range_total(total)
}

fn parse_content_range_total(total: &str) -> Result<u64, String> {
    if total == "*" {
        return Err("Slack media response omitted its total size".to_string());
    }
    total
        .parse::<u64>()
        .map_err(|error| format!("Slack media returned an invalid total size: {error}"))
}

fn validate_slack_media_total_size(total: u64) -> Result<(), String> {
    if total == 0 {
        return Err("Slack media response is empty".to_string());
    }
    if total > SLACK_MEDIA_MAX_BYTES {
        return Err(format!(
            "Slack media exceeds the {SLACK_MEDIA_MAX_BYTES} byte playback limit"
        ));
    }
    Ok(())
}
