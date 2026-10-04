use url::Url;

use crate::model::SlackMessageTimestamp;

use super::SLACK_SEARCH_INLINE_METHOD;

struct SlackQuickSearchPermalinkPath {
    conversation_id: String,
    message_path: String,
}

pub(super) fn validate_slack_quick_search_permalink(
    permalink: &str,
    conversation_id: &str,
    message_timestamp: &SlackMessageTimestamp,
    thread_timestamp: Option<&SlackMessageTimestamp>,
) -> Result<(), String> {
    let url = parse_slack_quick_search_permalink(permalink)?;
    let path = parse_slack_quick_search_permalink_path(&url)?;
    if path.conversation_id != conversation_id {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink conversation does not match item.channel_id"
        ));
    }
    let permalink_timestamp = parse_slack_quick_search_message_path(&path.message_path)?;
    if &permalink_timestamp != message_timestamp {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink timestamp does not match item.ts"
        ));
    }
    validate_slack_quick_search_permalink_query(&url, conversation_id, thread_timestamp)
}

fn parse_slack_quick_search_permalink(permalink: &str) -> Result<Url, String> {
    let url = Url::parse(permalink).map_err(|error| {
        format!("Slack {SLACK_SEARCH_INLINE_METHOD} returned an invalid item.permalink: {error}")
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.host_str().is_some_and(|host| {
            let host = host.to_ascii_lowercase();
            host == "slack.com" || host.ends_with(".slack.com")
        })
    {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} returned a non-Slack item.permalink"
        ));
    }
    Ok(url)
}

fn parse_slack_quick_search_permalink_path(
    url: &Url,
) -> Result<SlackQuickSearchPermalinkPath, String> {
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ["archives", conversation_id, message_path, ..] = segments.as_slice() else {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} returned an unsupported item.permalink path"
        ));
    };
    Ok(SlackQuickSearchPermalinkPath {
        conversation_id: conversation_id.to_string(),
        message_path: message_path.to_string(),
    })
}

fn parse_slack_quick_search_message_path(
    message_path: &str,
) -> Result<SlackMessageTimestamp, String> {
    let digits = message_path.strip_prefix('p').ok_or_else(|| {
        format!("Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink message path must start with p")
    })?;
    if digits.len() <= 6 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink has an invalid message timestamp"
        ));
    }
    let split = digits.len() - 6;
    SlackMessageTimestamp::parse(&format!("{}.{}", &digits[..split], &digits[split..]))
}

fn validate_slack_quick_search_permalink_query(
    url: &Url,
    conversation_id: &str,
    thread_timestamp: Option<&SlackMessageTimestamp>,
) -> Result<(), String> {
    let query_conversation_id = single_slack_permalink_query_value(url, "cid")?;
    if query_conversation_id
        .as_deref()
        .is_some_and(|query_conversation_id| query_conversation_id != conversation_id)
    {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink cid does not match item.channel_id"
        ));
    }
    let query_thread_timestamp = single_slack_permalink_query_value(url, "thread_ts")?
        .map(|timestamp| SlackMessageTimestamp::parse(&timestamp))
        .transpose()?;
    if query_thread_timestamp.as_ref() != thread_timestamp {
        return Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink thread_ts does not match item.thread_ts"
        ));
    }
    Ok(())
}

fn single_slack_permalink_query_value(url: &Url, key: &str) -> Result<Option<String>, String> {
    let values = url
        .query_pairs()
        .filter_map(|(query_key, value)| (query_key == key).then_some(value.into_owned()))
        .collect::<Vec<_>>();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(value.clone())),
        _ => Err(format!(
            "Slack {SLACK_SEARCH_INLINE_METHOD} item.permalink repeats {key}"
        )),
    }
}
