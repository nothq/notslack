use emojis::get_by_shortcode;
use gpui::SharedString;
use time::{Month, OffsetDateTime, UtcOffset, Weekday};

use super::{SlackActivityRow, SlackActivityRowKind, SlackActivityWorkspaceContext};
use crate::model::{
    SlackActivityActor, SlackActivityContent, SlackActivityItem, SlackConversationKind,
    SlackMessage, SlackMessageTimestamp,
};
use crate::ui::surface::{prepare_slack_message_body_from_message, SlackMessageBody};

struct SlackActivityTarget<'a> {
    kind: SlackActivityRowKind,
    channel_id: &'a str,
    message_timestamp: &'a SlackMessageTimestamp,
    thread_timestamp: Option<&'a SlackMessageTimestamp>,
    message: &'a SlackMessage,
    reaction_actor: Option<&'a SlackActivityActor>,
    reaction_name: Option<&'a str>,
}

pub(super) fn slack_activity_row(
    item: &SlackActivityItem,
    workspace: &SlackActivityWorkspaceContext,
    now: OffsetDateTime,
    local_offset: UtcOffset,
) -> Option<SlackActivityRow> {
    let target = slack_activity_target(item)?;
    let channel = slack_activity_channel(workspace, target.channel_id);
    let body = prepare_slack_message_body_from_message(target.message);
    let channel_label = channel.map(|channel| channel.display_label());
    let context_label = slack_activity_context_label(target.kind, channel_label.as_deref());
    let timestamp_label =
        slack_activity_relative_timestamp(&item.feed_timestamp, now, local_offset);
    let reaction_label = target.reaction_name.map(slack_activity_reaction_label);
    let card_height = slack_activity_card_height(&body, reaction_label.is_some());
    let (actor_label, avatar_text, avatar_fill, avatar_image_url) =
        slack_activity_actor(target.message, target.reaction_actor);
    let accessibility_label =
        slack_activity_accessibility_label(&actor_label, &context_label, &body, &timestamp_label);

    Some(SlackActivityRow {
        key: item.key.clone().into(),
        element_id: format!("slack-activity-{}", item.key).into(),
        actions_id: format!("slack-activity-actions-{}", item.key).into(),
        actions_hover_group: format!("slack-activity-hover-{}", item.key).into(),
        read_action_id: format!("slack-activity-read-action-{}", item.key).into(),
        archive_action_id: format!("slack-activity-archive-action-{}", item.key).into(),
        read_target: item.read_target(),
        archive_target: item.archive_target(),
        kind: target.kind,
        channel_id: target.channel_id.to_string().into(),
        message_timestamp: target.message_timestamp.clone(),
        thread_timestamp: target.thread_timestamp.cloned(),
        channel_label: channel_label.map(Into::into),
        actor_label: actor_label.into(),
        avatar_text: avatar_text.into(),
        avatar_fill,
        avatar_image_url: avatar_image_url.map(Into::into),
        timestamp_label: timestamp_label.into(),
        body,
        reaction_label,
        divider_label: None,
        archived: item.archived,
        unread: item.unread,
        card_height,
        accessibility_label: accessibility_label.into(),
    })
}

fn slack_activity_target(item: &SlackActivityItem) -> Option<SlackActivityTarget<'_>> {
    let target = match &item.content {
        SlackActivityContent::MessageReaction(activity) => SlackActivityTarget {
            kind: SlackActivityRowKind::Reaction,
            channel_id: &activity.channel_id,
            message_timestamp: &activity.message_timestamp,
            thread_timestamp: activity.thread_timestamp.as_ref(),
            message: &activity.message,
            reaction_actor: Some(&activity.actor),
            reaction_name: Some(&activity.reaction_name),
        },
        SlackActivityContent::ThreadV2(activity) => SlackActivityTarget {
            kind: SlackActivityRowKind::Thread,
            channel_id: &activity.channel_id,
            message_timestamp: &activity.latest_timestamp,
            thread_timestamp: activity.thread_timestamp.as_ref(),
            message: &activity.message,
            reaction_actor: None,
            reaction_name: None,
        },
        SlackActivityContent::AtUser(activity) => SlackActivityTarget {
            kind: SlackActivityRowKind::Mention,
            channel_id: &activity.channel_id,
            message_timestamp: &activity.message_timestamp,
            thread_timestamp: activity.thread_timestamp.as_ref(),
            message: &activity.message,
            reaction_actor: None,
            reaction_name: None,
        },
        SlackActivityContent::Dm(activity) => SlackActivityTarget {
            kind: SlackActivityRowKind::Dm,
            channel_id: &activity.channel_id,
            message_timestamp: &activity.latest_message_timestamp,
            thread_timestamp: activity.thread_timestamp.as_ref(),
            message: &activity.message,
            reaction_actor: None,
            reaction_name: None,
        },
        SlackActivityContent::BotDmBundle(activity) => SlackActivityTarget {
            kind: SlackActivityRowKind::BotDm,
            channel_id: &activity.channel_id,
            message_timestamp: &activity.message_timestamp,
            thread_timestamp: activity.thread_timestamp.as_ref(),
            message: &activity.message,
            reaction_actor: None,
            reaction_name: None,
        },
        SlackActivityContent::Unsupported { .. } => return None,
    };
    Some(target)
}

fn slack_activity_actor(
    message: &SlackMessage,
    reaction_actor: Option<&SlackActivityActor>,
) -> (String, String, u32, Option<String>) {
    match reaction_actor {
        Some(actor) => (
            actor.display_label.clone(),
            crate::ui::initials(&actor.display_label),
            crate::ui::slack_avatar_fill(&actor.display_label),
            actor.avatar_image_url.clone(),
        ),
        None => (
            message.author.clone(),
            message
                .avatar_label
                .clone()
                .unwrap_or_else(|| crate::ui::initials(&message.author)),
            crate::ui::slack_avatar_fill(
                message
                    .avatar_label
                    .as_deref()
                    .unwrap_or(message.author.as_str()),
            ),
            message.avatar_image_url.clone(),
        ),
    }
}

fn slack_activity_reaction_label(name: &str) -> SharedString {
    get_by_shortcode(name)
        .map(|emoji| emoji.as_str().to_string())
        .unwrap_or_else(|| format!(":{name}:"))
        .into()
}

pub(super) fn slack_activity_card_height(body: &SlackMessageBody, has_reaction: bool) -> f32 {
    let body_line_count = body
        .text
        .lines()
        .map(|line| line.chars().count().div_ceil(80).max(1))
        .sum::<usize>()
        .clamp(1, 2);
    74.0 + body_line_count as f32 * 22.0 + if has_reaction { 34.0 } else { 0.0 }
}

pub(super) fn slack_activity_accessibility_label(
    actor_label: &str,
    context_label: &str,
    body: &SlackMessageBody,
    timestamp_label: &str,
) -> String {
    format!(
        "{actor_label}. {context_label}. {}. {timestamp_label}",
        body.text
    )
}

#[derive(Clone)]
pub(super) struct SlackActivityChannel {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) kind: SlackConversationKind,
}

impl SlackActivityChannel {
    pub(super) fn display_label(&self) -> String {
        match self.kind {
            SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => {
                format!("# {}", self.label)
            }
            SlackConversationKind::DirectMessage
            | SlackConversationKind::GroupMessage
            | SlackConversationKind::Unknown => self.label.to_string(),
        }
    }
}

pub(super) fn slack_activity_channel<'a>(
    workspace: &'a SlackActivityWorkspaceContext,
    channel_id: &str,
) -> Option<&'a SlackActivityChannel> {
    workspace
        .channels
        .iter()
        .find(|channel| channel.id == channel_id)
}

pub(super) fn slack_activity_context_label(
    kind: SlackActivityRowKind,
    channel_label: Option<&str>,
) -> String {
    let base = kind.context_prefix();
    channel_label
        .filter(|_| !kind.is_dm())
        .map(|channel| format!("{base} {channel}"))
        .unwrap_or_else(|| base.to_string())
}

pub(super) fn slack_activity_timestamp(value: &str) -> Option<OffsetDateTime> {
    let seconds = value.split('.').next()?.parse::<i64>().ok()?;
    OffsetDateTime::from_unix_timestamp(seconds).ok()
}

pub(super) fn slack_activity_relative_timestamp(
    value: &str,
    now: OffsetDateTime,
    local_offset: UtcOffset,
) -> String {
    let Some(timestamp) = slack_activity_timestamp(value) else {
        return String::new();
    };
    let age = now - timestamp;
    if age.whole_minutes() < 1 {
        return "Now".to_string();
    }
    if age.whole_hours() < 1 {
        let minutes = age.whole_minutes();
        return format!("{minutes} min{}", if minutes == 1 { "" } else { "s" });
    }
    if age.whole_hours() < 24 {
        let hours = age.whole_hours();
        return format!("{hours} hr{}", if hours == 1 { "" } else { "s" });
    }
    slack_activity_date_label(
        timestamp.to_offset(local_offset).date(),
        now.to_offset(local_offset).date(),
    )
}

pub(super) fn slack_activity_date_label(date: time::Date, today: time::Date) -> String {
    let days_ago = today.to_julian_day() - date.to_julian_day();
    match days_ago {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        2..=6 => slack_activity_weekday_label(date.weekday()).to_string(),
        _ => format!(
            "{} {}{}",
            slack_activity_month_label(date.month()),
            date.day(),
            slack_activity_ordinal_suffix(date.day())
        ),
    }
}

pub(super) fn slack_local_offset() -> UtcOffset {
    UtcOffset::current_local_offset().expect("The desktop must provide the local timezone offset")
}

pub(super) fn slack_activity_weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

pub(super) fn slack_activity_month_label(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

pub(super) fn slack_activity_ordinal_suffix(day: u8) -> &'static str {
    if matches!(day % 100, 11..=13) {
        return "th";
    }
    match day % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}
