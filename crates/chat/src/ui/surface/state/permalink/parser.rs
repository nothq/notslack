use url::Url;

use super::SlackMessageNavigationTarget;
use crate::ui::SlackMessageTimestamp;

const SLACK_PERMALINK_TIMESTAMP_FRACTION_DIGITS: usize = 6;

pub(super) enum ParsedSlackLink {
    External,
    Message(SlackMessageNavigationTarget),
}

pub(super) fn parse_slack_link(link: &str) -> Result<ParsedSlackLink, String> {
    let Ok(url) = Url::parse(link) else {
        return Ok(ParsedSlackLink::External);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(ParsedSlackLink::External);
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return Ok(ParsedSlackLink::External);
    };
    if host != "slack.com" && !host.ends_with(".slack.com") {
        return Ok(ParsedSlackLink::External);
    }
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match segments.as_slice() {
        ["archives", conversation_id, message_path, ..] if message_path.starts_with('p') => {
            parse_slack_archives_message(&url, conversation_id, message_path)
                .map(ParsedSlackLink::Message)
        }
        ["client", team_id, conversation_id, tail @ ..] => {
            parse_slack_client_message(&url, team_id, conversation_id, tail)
        }
        _ => Ok(ParsedSlackLink::External),
    }
}

fn parse_slack_archives_message(
    url: &Url,
    conversation_id: &str,
    message_path: &str,
) -> Result<SlackMessageNavigationTarget, String> {
    let conversation_id = parse_slack_identifier("conversation", conversation_id)?;
    ensure_query_conversation_matches(url, &conversation_id)?;
    let message_timestamp = parse_slack_permalink_timestamp(message_path)?;
    let thread_timestamp = slack_query_timestamp(url, "thread_ts")?;
    Ok(SlackMessageNavigationTarget {
        team_id: None,
        conversation_id,
        message_timestamp,
        thread_timestamp,
    })
}

fn parse_slack_client_message(
    url: &Url,
    team_id: &str,
    conversation_id: &str,
    tail: &[&str],
) -> Result<ParsedSlackLink, String> {
    let team_id = parse_slack_identifier("team", team_id)?;
    let conversation_id = parse_slack_identifier("conversation", conversation_id)?;
    ensure_query_conversation_matches(url, &conversation_id)?;
    let query_thread_timestamp = slack_query_timestamp(url, "thread_ts")?;
    let path_timestamp = match tail {
        [message_path, ..] if message_path.starts_with('p') => {
            Some(parse_slack_permalink_timestamp(message_path)?)
        }
        ["thread", thread_path, ..] => Some(parse_slack_client_thread_timestamp(
            thread_path,
            &conversation_id,
        )?),
        _ => None,
    };
    let Some(message_timestamp) = path_timestamp.or_else(|| query_thread_timestamp.clone()) else {
        return Ok(ParsedSlackLink::External);
    };
    let thread_timestamp = match tail {
        ["thread", ..] => query_thread_timestamp.or_else(|| Some(message_timestamp.clone())),
        _ => query_thread_timestamp,
    };
    Ok(ParsedSlackLink::Message(SlackMessageNavigationTarget {
        team_id: Some(team_id),
        conversation_id,
        message_timestamp,
        thread_timestamp,
    }))
}

fn parse_slack_identifier(kind: &str, value: &str) -> Result<String, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(format!(
            "Slack permalink contains an invalid {kind} identifier"
        ));
    }
    Ok(value.to_string())
}

fn parse_slack_permalink_timestamp(value: &str) -> Result<SlackMessageTimestamp, String> {
    let digits = value
        .strip_prefix('p')
        .ok_or_else(|| "Slack permalink message path must start with p".to_string())?;
    parse_slack_timestamp_digits(digits)
}

fn parse_slack_timestamp_digits(digits: &str) -> Result<SlackMessageTimestamp, String> {
    if digits.len() <= SLACK_PERMALINK_TIMESTAMP_FRACTION_DIGITS
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("Slack permalink contains an invalid message timestamp".to_string());
    }
    let split = digits.len() - SLACK_PERMALINK_TIMESTAMP_FRACTION_DIGITS;
    SlackMessageTimestamp::parse(&format!("{}.{}", &digits[..split], &digits[split..]))
}

fn parse_slack_client_thread_timestamp(
    value: &str,
    conversation_id: &str,
) -> Result<SlackMessageTimestamp, String> {
    let timestamp = value
        .strip_prefix(conversation_id)
        .and_then(|value| value.strip_prefix('-'))
        .ok_or_else(|| {
            "Slack client thread path does not match its conversation identifier".to_string()
        })?;
    if timestamp.contains('.') {
        SlackMessageTimestamp::parse(timestamp)
    } else {
        parse_slack_timestamp_digits(timestamp)
    }
}

fn slack_query_timestamp(url: &Url, key: &str) -> Result<Option<SlackMessageTimestamp>, String> {
    url.query_pairs()
        .find_map(|(query_key, value)| (query_key == key).then_some(value.into_owned()))
        .map(|value| SlackMessageTimestamp::parse(&value))
        .transpose()
}

fn ensure_query_conversation_matches(url: &Url, conversation_id: &str) -> Result<(), String> {
    let query_conversation_id = url
        .query_pairs()
        .find_map(|(key, value)| (key == "cid").then_some(value.into_owned()));
    if query_conversation_id
        .as_deref()
        .is_some_and(|query_conversation_id| query_conversation_id != conversation_id)
    {
        return Err(
            "Slack permalink query conversation does not match its path conversation".to_string(),
        );
    }
    Ok(())
}
