use std::collections::HashMap;

use crate::model::{SlackConversationKind, SlackProfile};
use chrono::Utc;
use chrono_tz::Tz;
use serde_json::Value;

use crate::live::conversation::normalize_conversation_label;

#[derive(Clone, Debug)]
pub(super) struct SlackTimezone {
    pub(super) id: Option<String>,
    pub(super) value: Tz,
}

#[derive(Clone, Copy)]
pub(in crate::live) enum SlackAvatarPurpose {
    Sidebar,
    Compact,
    Message,
    Profile,
}

impl SlackTimezone {
    pub(super) fn from_native_runtime() -> Result<Self, String> {
        let timezone_id = iana_time_zone::get_timezone()
            .map_err(|error| format!("The desktop runtime timezone lookup failed: {error}"))?;
        let timezone = timezone_id.parse::<Tz>().map_err(|error| {
            format!("The desktop runtime returned invalid timezone {timezone_id:?}: {error}")
        })?;
        Ok(Self {
            id: Some(timezone.to_string()),
            value: timezone,
        })
    }

    #[cfg(test)]
    pub(super) fn utc() -> Self {
        Self {
            id: None,
            value: chrono_tz::UTC,
        }
    }
}

pub(super) fn channel_topic(channel: &Value, channel_kind: SlackConversationKind) -> String {
    if channel_kind.is_channel() {
        string_at(channel, &["topic", "value"]).unwrap_or_default()
    } else {
        String::new()
    }
}

pub(super) fn channel_member_count(
    channel: &Value,
    channel_kind: SlackConversationKind,
) -> Option<u32> {
    channel_kind
        .is_channel()
        .then(|| value_as_u32(channel.get("num_members")))
        .flatten()
}

pub(super) fn slack_conversation_kind(channel: &Value) -> SlackConversationKind {
    if channel.get("is_im").and_then(Value::as_bool) == Some(true) {
        SlackConversationKind::DirectMessage
    } else if channel.get("is_mpim").and_then(Value::as_bool) == Some(true) {
        SlackConversationKind::GroupMessage
    } else if is_private_channel(channel) {
        SlackConversationKind::PrivateChannel
    } else if channel.get("is_channel").and_then(Value::as_bool) == Some(true) {
        SlackConversationKind::Channel
    } else {
        SlackConversationKind::Unknown
    }
}

pub(super) fn is_private_channel(channel: &Value) -> bool {
    channel.get("is_private").and_then(Value::as_bool) == Some(true)
        || channel.get("is_group").and_then(Value::as_bool) == Some(true)
}

pub(super) fn slack_conversation_label(
    channel: &Value,
    users: &HashMap<String, Value>,
) -> Option<String> {
    let kind = slack_conversation_kind(channel);
    let label = match kind {
        SlackConversationKind::DirectMessage => channel
            .get("user")
            .and_then(Value::as_str)
            .and_then(|user_id| users.get(user_id))
            .and_then(slack_user_display_name)
            .or_else(|| string_at(channel, &["user"])),
        _ => string_at(channel, &["name"]).or_else(|| string_at(channel, &["id"])),
    }?;
    Some(normalize_conversation_label(&label, kind))
}

pub(super) fn slack_user_display_name(user: &Value) -> Option<String> {
    string_at(user, &["profile", "real_name"])
        .or_else(|| string_at(user, &["real_name"]))
        .or_else(|| string_at(user, &["profile", "display_name"]))
}

pub(in crate::live) struct SlackAvatarImageUrls<'a> {
    pub(in crate::live) image_24: Option<&'a str>,
    pub(in crate::live) image_32: Option<&'a str>,
    pub(in crate::live) image_48: Option<&'a str>,
    pub(in crate::live) image_72: Option<&'a str>,
    pub(in crate::live) image_192: Option<&'a str>,
    pub(in crate::live) image_512: Option<&'a str>,
}

pub(in crate::live) fn select_slack_avatar_image_url(
    purpose: SlackAvatarPurpose,
    images: SlackAvatarImageUrls<'_>,
) -> Option<String> {
    let SlackAvatarImageUrls {
        image_24,
        image_32,
        image_48,
        image_72,
        image_192,
        image_512,
    } = images;
    let image_urls = match purpose {
        SlackAvatarPurpose::Sidebar => {
            [image_32, image_48, image_72, image_192, image_512, image_24]
        }
        SlackAvatarPurpose::Compact => {
            [image_48, image_72, image_192, image_512, image_32, image_24]
        }
        SlackAvatarPurpose::Message => {
            [image_72, image_192, image_512, image_48, image_32, image_24]
        }
        SlackAvatarPurpose::Profile => {
            [image_512, image_192, image_72, image_48, image_32, image_24]
        }
    };
    image_urls
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|url| !url.is_empty())
        .map(str::to_string)
}

pub(super) fn slack_user_avatar_image_url(
    user: &Value,
    purpose: SlackAvatarPurpose,
) -> Option<String> {
    let profile = user.get("profile")?;
    select_slack_avatar_image_url(
        purpose,
        SlackAvatarImageUrls {
            image_24: profile.get("image_24").and_then(Value::as_str),
            image_32: profile.get("image_32").and_then(Value::as_str),
            image_48: profile.get("image_48").and_then(Value::as_str),
            image_72: profile.get("image_72").and_then(Value::as_str),
            image_192: profile.get("image_192").and_then(Value::as_str),
            image_512: profile.get("image_512").and_then(Value::as_str),
        },
    )
}

pub(super) fn slack_workspace_logo_url(team_info: &Value) -> Option<String> {
    [
        ["team", "icon", "image_230"].as_slice(),
        ["team", "icon", "image_132"].as_slice(),
        ["team", "icon", "image_102"].as_slice(),
        ["team", "icon", "image_88"].as_slice(),
        ["team", "icon", "image_68"].as_slice(),
        ["team", "icon", "image_44"].as_slice(),
        ["team", "icon", "image_34"].as_slice(),
    ]
    .into_iter()
    .find_map(|path| string_at(team_info, path))
}

pub(super) fn slack_profile_from_user(user_id: &str, user: &Value) -> SlackProfile {
    let display_name = slack_user_display_name(user).unwrap_or_else(|| user_id.to_string());
    SlackProfile {
        user_id: user_id.to_string(),
        real_name: string_at(user, &["real_name"]).unwrap_or_else(|| display_name.clone()),
        title: string_at(user, &["profile", "title"]),
        status_text: string_at(user, &["profile", "status_text"]),
        email: string_at(user, &["profile", "email"]),
        phone: string_at(user, &["profile", "phone"]),
        timezone_label: string_at(user, &["tz_label"]).or_else(|| string_at(user, &["tz"])),
        avatar_label: Some(initials(&display_name)),
        avatar_image_url: slack_user_avatar_image_url(user, SlackAvatarPurpose::Profile),
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        display_name,
    }
}

pub(super) fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn value_as_u32(value: Option<&Value>) -> Option<u32> {
    value.and_then(|value| {
        value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .or_else(|| value.as_str()?.parse::<u32>().ok())
    })
}

pub(super) fn has_slack_unread_messages(channel: &Value) -> bool {
    slack_unread_count(channel) > 0
        || channel.get("has_unreads").and_then(Value::as_bool) == Some(true)
        || channel.get("is_unread").and_then(Value::as_bool) == Some(true)
        || slack_latest_after_last_read(channel)
}

fn slack_latest_after_last_read(channel: &Value) -> bool {
    let Some(latest) = slack_timestamp_value(channel.get("latest")) else {
        return false;
    };
    let last_read = slack_timestamp_value(channel.get("last_read")).unwrap_or_default();
    latest > last_read
}

fn slack_timestamp_value(value: Option<&Value>) -> Option<u128> {
    let value = value?;
    let value = value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_u64().map(|value| value.to_string()))?;
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u128>().ok()
}

pub(super) fn slack_unread_count(channel: &Value) -> u32 {
    value_as_u32(channel.get("unread_count"))
        .or_else(|| value_as_u32(channel.get("unread_count_display")))
        .unwrap_or(0)
}

pub(super) fn slack_unread_display_count(channel: &Value) -> Option<u32> {
    value_as_u32(channel.get("unread_count_display")).filter(|count| *count > 0)
}

pub(super) fn normalize_whitespace(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn format_slack_timestamp_in_timezone(value: &str, timezone: Tz) -> String {
    let Some(seconds) = value
        .split('.')
        .next()
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return String::new();
    };
    let Some(timestamp) = chrono::DateTime::<Utc>::from_timestamp(seconds, 0) else {
        return String::new();
    };
    timestamp
        .with_timezone(&timezone)
        .format("%-I:%M %p")
        .to_string()
}

pub(super) fn initials(value: &str) -> String {
    let initials = value
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}

#[cfg(test)]
mod tests {
    use crate::model::SlackConversationKind;
    use serde_json::json;

    use super::slack_conversation_kind;

    #[test]
    fn slack_channel_with_is_private_is_private_channel() {
        let channel = json!({
            "id": "C_PRIVATE",
            "is_channel": true,
            "is_private": true
        });

        assert_eq!(
            slack_conversation_kind(&channel),
            SlackConversationKind::PrivateChannel
        );
    }
}
