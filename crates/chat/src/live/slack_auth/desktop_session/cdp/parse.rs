use serde_json::Value;
#[cfg(test)]
use url::{form_urlencoded, Url};

pub(super) fn required_local_config_field(value: &str, field: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!(
            "Slack Desktop localConfig_v2 team record missing {field}"
        ));
    }
    Ok(value.to_string())
}

#[cfg(test)]
pub(super) fn team_domain_from_api_url(url: &str) -> Result<String, String> {
    let parsed = Url::parse(url)
        .map_err(|error| format!("Slack sidebar request URL was invalid: {error}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "Slack sidebar request URL did not include a host".to_string())?;
    let Some(domain) = host.strip_suffix(".slack.com") else {
        return Err("Slack sidebar request URL was not a Slack workspace host".to_string());
    };
    if domain.is_empty() || domain == "app" {
        return Err("Slack sidebar request URL did not include a workspace domain".to_string());
    }
    Ok(domain.to_string())
}

#[cfg(test)]
pub(super) fn slack_token_from_post_data(post_data: &str) -> Option<String> {
    urlencoded_token(post_data).or_else(|| multipart_token(post_data))
}

#[cfg(test)]
fn urlencoded_token(post_data: &str) -> Option<String> {
    form_urlencoded::parse(post_data.as_bytes())
        .find(|(key, value)| key == "token" && value.starts_with("xoxc-"))
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
fn multipart_token(post_data: &str) -> Option<String> {
    let after_token_header = post_data.split_once("name=\"token\"")?.1;
    let (separator, offset) = if let Some(index) = after_token_header.find("\r\n\r\n") {
        ("\r\n--", index + 4)
    } else {
        ("\n--", after_token_header.find("\n\n")? + 2)
    };
    let token_body = &after_token_header[offset..];
    let token_end = token_body.find(separator).unwrap_or(token_body.len());
    let token = token_body[..token_end].trim();
    token.starts_with("xoxc-").then(|| token.to_string())
}

pub(super) fn cookie_header_pair(cookie: &Value) -> Option<String> {
    let name = cookie.get("name")?.as_str()?.trim();
    let value = cookie.get("value")?.as_str()?;
    if name.is_empty() {
        return None;
    }
    Some(format!("{name}={value}"))
}
