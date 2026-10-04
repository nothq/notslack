use std::io::Read;

use crate::live::api::http::slack_authenticated_https_url;

use super::SLACK_FLANNEL_EMOJI_LIST_METHOD;

const MAX_SLACK_FLANNEL_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(super) fn flannel_emoji_list_url(base_url: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|error| format!("Slack Flannel endpoint URL is invalid: {error}"))?;
    if !slack_authenticated_https_url(&url) {
        return Err("Slack Flannel endpoint must use a trusted Slack HTTPS host".to_string());
    }
    let path = format!(
        "{}/{}",
        url.path().trim_end_matches('/'),
        SLACK_FLANNEL_EMOJI_LIST_METHOD
    );
    url.set_path(&path);
    if !url.path().ends_with("/emojis/list") {
        return Err("Slack Flannel emoji endpoint path construction failed".to_string());
    }
    if url.query_pairs().any(|(key, _)| key == "_x_app_name") {
        return Err(
            "Slack Flannel endpoint unexpectedly supplied an _x_app_name query parameter"
                .to_string(),
        );
    }
    url.query_pairs_mut().append_pair("_x_app_name", "client");
    Ok(url)
}

pub(super) fn read_bounded_flannel_response(
    response: &mut reqwest::blocking::Response,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SLACK_FLANNEL_RESPONSE_BYTES as u64)
    {
        return Err(format!(
            "Slack Flannel emojis/list response exceeds {MAX_SLACK_FLANNEL_RESPONSE_BYTES} bytes"
        ));
    }
    let mut body = Vec::new();
    response
        .take(MAX_SLACK_FLANNEL_RESPONSE_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| format!("failed to read Slack Flannel emojis/list response: {error}"))?;
    if body.len() > MAX_SLACK_FLANNEL_RESPONSE_BYTES {
        return Err(format!(
            "Slack Flannel emojis/list response exceeds {MAX_SLACK_FLANNEL_RESPONSE_BYTES} bytes"
        ));
    }
    Ok(body)
}
