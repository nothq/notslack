mod alias;
mod transport;

use std::collections::{BTreeMap, HashSet};

use crate::model::{SlackCustomEmoji, SlackCustomEmojiAsset};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, COOKIE};
use serde::{Deserialize, Serialize};

use super::parse_reaction_name;
use crate::live::api::{http::slack_authenticated_https_url, SlackApiAuth, SlackApiClient};
use transport::{flannel_emoji_list_url, read_bounded_flannel_response};

const SLACK_FLANNEL_ENDPOINT_METHOD: &str = "api.getFlannelHttpUrl";
const SLACK_FLANNEL_EMOJI_LIST_METHOD: &str = "emojis/list";
const SLACK_REACTION_CATALOG_PAGE_SIZE: u16 = 100;
const MAX_SLACK_REACTION_CATALOG_PAGES: usize = 100;
const MAX_SLACK_REACTION_CATALOG_RESULTS: usize =
    MAX_SLACK_REACTION_CATALOG_PAGES * SLACK_REACTION_CATALOG_PAGE_SIZE as usize;

#[derive(Deserialize)]
struct SlackFlannelEndpointResponseWire {
    url: String,
}

#[derive(Serialize)]
struct SlackFlannelEmojiListRequestWire<'a> {
    token: &'a str,
    count: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    marker: Option<&'a str>,
}

#[derive(Deserialize)]
struct SlackFlannelEmojiListResponseWire {
    ok: bool,
    #[serde(default)]
    results: Option<SlackFlannelEmojiResultsWire>,
    #[serde(default)]
    next_marker: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

struct SlackFlannelEmojiListPage {
    results: SlackFlannelEmojiResultsWire,
    next_marker: Option<String>,
}

struct SlackFlannelEndpoint<'a> {
    url: reqwest::Url,
    token: &'a str,
    cookie_header: &'a HeaderValue,
}

#[derive(Deserialize)]
struct SlackFlannelEmojiWire {
    name: String,
    value: SlackFlannelEmojiValueWire,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    is_alias: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackFlannelEmojiValueWire {
    String(String),
    Platform(BTreeMap<String, String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SlackFlannelEmojiResultsWire {
    List(Vec<SlackFlannelEmojiWire>),
    Map(BTreeMap<String, SlackFlannelEmojiWire>),
}

struct ParsedSlackFlannelEmoji {
    name: String,
    asset: ParsedSlackFlannelEmojiAsset,
}

enum ParsedSlackFlannelEmojiAsset {
    Platform,
    ImageUrl(String),
    Alias(ParsedSlackFlannelEmojiAlias),
}

struct ParsedSlackFlannelEmojiAlias {
    target: String,
    resolved_image_url: Option<String>,
}

pub(super) fn load_custom_emoji(api: &SlackApiClient) -> Result<Vec<SlackCustomEmoji>, String> {
    let endpoint = flannel_endpoint(api)?;
    let catalog = load_flannel_pages(api, endpoint.url, endpoint.token, endpoint.cookie_header)?;
    alias::validate_alias_graph(&catalog)?;
    Ok(catalog.into_iter().filter_map(into_custom_emoji).collect())
}

fn load_flannel_pages(
    api: &SlackApiClient,
    endpoint: reqwest::Url,
    token: &str,
    cookie_header: &HeaderValue,
) -> Result<Vec<ParsedSlackFlannelEmoji>, String> {
    let mut marker = None;
    let mut seen_markers = HashSet::new();
    let mut seen_names = HashSet::new();
    let mut catalog = Vec::new();
    let mut page_count = 0;
    let mut result_count: usize = 0;

    loop {
        page_count += 1;
        validate_page_count(page_count)?;
        let request = SlackFlannelEmojiListRequestWire {
            token,
            count: SLACK_REACTION_CATALOG_PAGE_SIZE,
            marker: marker.as_deref(),
        };
        let page = post_flannel_emoji_page(api, endpoint.clone(), cookie_header, &request)?;
        let results = page.results.into_vec()?;
        result_count = validate_result_count(result_count, results.len())?;
        append_unique_results(&mut catalog, &mut seen_names, results)?;
        marker = next_page_marker(page.next_marker, &mut seen_markers)?;
        if marker.is_none() {
            break;
        }
    }
    Ok(catalog)
}

fn flannel_endpoint(api: &SlackApiClient) -> Result<SlackFlannelEndpoint<'_>, String> {
    let endpoint_payload = api.post(
        SLACK_FLANNEL_ENDPOINT_METHOD,
        &[("include_external_workspaces", "true".to_string())],
    )?;
    let endpoint = serde_json::from_value::<SlackFlannelEndpointResponseWire>(endpoint_payload)
        .map_err(|error| {
            format!("failed to decode Slack {SLACK_FLANNEL_ENDPOINT_METHOD} response: {error}")
        })?;
    let endpoint = flannel_emoji_list_url(&endpoint.url)?;
    let (token, cookie_header) = desktop_auth(api)?;
    Ok(SlackFlannelEndpoint {
        url: endpoint,
        token,
        cookie_header,
    })
}

fn validate_page_count(page_count: usize) -> Result<(), String> {
    if page_count > MAX_SLACK_REACTION_CATALOG_PAGES {
        return Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} exceeded the {MAX_SLACK_REACTION_CATALOG_PAGES}-page safety limit"
        ));
    }
    Ok(())
}

fn validate_result_count(current: usize, page: usize) -> Result<usize, String> {
    let result_count = current.checked_add(page).ok_or_else(|| {
        format!("Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} result count overflowed")
    })?;
    if result_count > MAX_SLACK_REACTION_CATALOG_RESULTS {
        return Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} exceeded the {MAX_SLACK_REACTION_CATALOG_RESULTS}-result safety limit"
        ));
    }
    Ok(result_count)
}

fn append_unique_results(
    catalog: &mut Vec<ParsedSlackFlannelEmoji>,
    seen_names: &mut HashSet<String>,
    results: Vec<SlackFlannelEmojiWire>,
) -> Result<(), String> {
    for emoji in results {
        let emoji = emoji.into_catalog_entry()?;
        if !seen_names.insert(emoji.name.clone()) {
            return Err(format!(
                "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} returned duplicate emoji {}",
                emoji.name
            ));
        }
        catalog.push(emoji);
    }
    Ok(())
}

fn next_page_marker(
    next_marker: Option<String>,
    seen_markers: &mut HashSet<String>,
) -> Result<Option<String>, String> {
    let Some(next_marker) = next_marker.filter(|marker| !marker.is_empty()) else {
        return Ok(None);
    };
    if !seen_markers.insert(next_marker.clone()) {
        return Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} repeated pagination marker"
        ));
    }
    Ok(Some(next_marker))
}

fn into_custom_emoji(emoji: ParsedSlackFlannelEmoji) -> Option<SlackCustomEmoji> {
    let asset = match emoji.asset {
        ParsedSlackFlannelEmojiAsset::Platform => return None,
        ParsedSlackFlannelEmojiAsset::ImageUrl(url) => SlackCustomEmojiAsset::ImageUrl(url),
        ParsedSlackFlannelEmojiAsset::Alias(alias) => SlackCustomEmojiAsset::Alias(alias.target),
    };
    Some(SlackCustomEmoji {
        name: emoji.name,
        asset,
    })
}

fn desktop_auth(api: &SlackApiClient) -> Result<(&str, &HeaderValue), String> {
    match &api.auth {
        SlackApiAuth::Desktop {
            xoxc_token,
            cookie_header,
            ..
        } => Ok((xoxc_token, cookie_header)),
        SlackApiAuth::Public => {
            Err("Slack Flannel request requires authenticated Slack credentials".to_string())
        }
    }
}

fn post_flannel_emoji_page(
    api: &SlackApiClient,
    endpoint: reqwest::Url,
    cookie_header: &HeaderValue,
    request: &SlackFlannelEmojiListRequestWire<'_>,
) -> Result<SlackFlannelEmojiListPage, String> {
    api.record_call("flannel.emojis.list")?;
    let mut response = api
        .http
        .post(endpoint)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(COOKIE, cookie_header.clone())
        .json(request)
        .send()
        .map_err(|error| format!("Slack Flannel emojis/list request failed: {error}"))?;
    if !slack_authenticated_https_url(response.url()) {
        return Err(
            "Slack Flannel emojis/list redirected outside trusted Slack HTTPS hosts".to_string(),
        );
    }
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Slack Flannel emojis/list returned HTTP {}",
            status.as_u16()
        ));
    }
    let body = read_bounded_flannel_response(&mut response)?;
    let response = serde_json::from_slice::<SlackFlannelEmojiListResponseWire>(&body)
        .map_err(|error| format!("failed to decode Slack Flannel emojis/list response: {error}"))?;
    if !response.ok {
        let error = response.error.as_deref().unwrap_or("unknown_error");
        return Err(format!("Slack Flannel emojis/list failed: {error}"));
    }
    let results = response
        .results
        .ok_or_else(|| "Slack Flannel emojis/list returned ok=true without results".to_string())?;
    Ok(SlackFlannelEmojiListPage {
        results,
        next_marker: response.next_marker,
    })
}

impl SlackFlannelEmojiWire {
    fn into_catalog_entry(self) -> Result<ParsedSlackFlannelEmoji, String> {
        let Self {
            name,
            value,
            alias,
            is_alias,
        } = self;
        let name = parse_reaction_name(SLACK_FLANNEL_EMOJI_LIST_METHOD, name)?;
        if is_alias {
            return into_alias_entry(name, value, alias);
        }
        if alias.is_some() {
            return Err(format!(
                "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} emoji {name} returned an alias target without is_alias"
            ));
        }
        let value = match value {
            SlackFlannelEmojiValueWire::String(value) => value,
            SlackFlannelEmojiValueWire::Platform(platforms) => {
                drop(platforms);
                return Ok(ParsedSlackFlannelEmoji {
                    name,
                    asset: ParsedSlackFlannelEmojiAsset::Platform,
                });
            }
        };
        let image_url = parse_trusted_image_url(&name, value)?;
        Ok(ParsedSlackFlannelEmoji {
            name,
            asset: ParsedSlackFlannelEmojiAsset::ImageUrl(image_url),
        })
    }
}

fn into_alias_entry(
    name: String,
    value: SlackFlannelEmojiValueWire,
    alias: Option<String>,
) -> Result<ParsedSlackFlannelEmoji, String> {
    let alias = alias.ok_or_else(|| {
        format!("Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {name} omitted its target")
    })?;
    let alias = parse_reaction_name(SLACK_FLANNEL_EMOJI_LIST_METHOD, alias)?;
    let SlackFlannelEmojiValueWire::String(value) = value else {
        return Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {name} returned a platform asset"
        ));
    };
    let resolved_image_url = match value.strip_prefix("alias:") {
        Some(value_target) => {
            let value_target =
                parse_reaction_name(SLACK_FLANNEL_EMOJI_LIST_METHOD, value_target.to_string())?;
            if value_target != alias {
                return Err(format!(
                    "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {name} returned a mismatched target"
                ));
            }
            None
        }
        None => Some(parse_trusted_image_url(&name, value)?),
    };
    Ok(ParsedSlackFlannelEmoji {
        name,
        asset: ParsedSlackFlannelEmojiAsset::Alias(ParsedSlackFlannelEmojiAlias {
            target: alias,
            resolved_image_url,
        }),
    })
}

fn parse_trusted_image_url(name: &str, value: String) -> Result<String, String> {
    let image_url = reqwest::Url::parse(&value).map_err(|error| {
        format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} emoji {name} returned an invalid image URL: {error}"
        )
    })?;
    if !slack_authenticated_https_url(&image_url) {
        return Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} emoji {name} returned an untrusted image URL"
        ));
    }
    Ok(value)
}

impl SlackFlannelEmojiResultsWire {
    fn into_vec(self) -> Result<Vec<SlackFlannelEmojiWire>, String> {
        match self {
            Self::List(results) => Ok(results),
            Self::Map(results) => results
                .into_iter()
                .map(|(key, emoji)| {
                    if key != emoji.name {
                        return Err(format!(
                            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} returned an emoji map key that did not match its name"
                        ));
                    }
                    Ok(emoji)
                })
                .collect(),
        }
    }
}
