use super::{
    initials, prepare_slack_message_body, slack_avatar_fill, slack_dm_clock_label,
    slack_dm_month_label, slack_dm_ordinal_suffix, slack_dm_relative_timestamp,
    slack_inline_video_dimensions, slack_relative_timestamp, Arc, OffsetDateTime,
    PreparedSlackSearchSnapshot, SharedString, SlackAttachmentMediaKind, SlackConversationKind,
    SlackDmInboxItem, SlackDmInboxSnapshot, SlackDmRow, SlackDmRowParticipant, SlackMediaHostId,
    SlackMessageActionTarget, SlackQuickSearchConversation, SlackQuickSearchMessage,
    SlackQuickSearchMessageRow, SlackQuickSearchPerson, SlackQuickSearchRow,
    SlackQuickSearchRowKind, SlackQuickSearchSnapshot, SlackQuickSearchTarget,
    SlackSearchAttachmentPresentation, SlackSearchAttachmentRow, SlackSearchRow,
    SlackSearchSnapshot, SlackWorkspace, UtcOffset,
};
use crate::ui::surface::{slack_attachment_row_with_identity, slack_reaction_rows};
use chrono_tz::Tz;

const SLACK_SEARCH_BODY_PREVIEW_MAX_CHARS: usize = 240;

mod dm;
mod highlights;
mod quick;

pub(crate) use dm::{build_slack_dm_rows, normalize_slack_dm_finder_text};
use highlights::{prepare_slack_search_body_preview, prepare_slack_search_text_highlights};
pub(crate) use quick::prepare_slack_quick_search_snapshot;

pub(crate) fn prepare_slack_search_snapshot(
    snapshot: SlackSearchSnapshot,
    highlight_query: &str,
    search_generation: u64,
    timezone: Tz,
) -> PreparedSlackSearchSnapshot {
    let now = OffsetDateTime::now_utc();
    let rows = snapshot
        .messages
        .iter()
        .map(|message| {
            prepare_slack_search_row(message, highlight_query, search_generation, timezone, now)
        })
        .collect::<Vec<_>>()
        .into();
    PreparedSlackSearchSnapshot { snapshot, rows }
}

fn prepare_slack_search_row(
    message: &crate::model::SlackSearchMessage,
    highlight_query: &str,
    search_generation: u64,
    timezone: Tz,
    now: OffsetDateTime,
) -> SlackSearchRow {
    let body = prepare_slack_message_body(&message.body);
    let (body_preview, preview_source_end) =
        prepare_slack_search_body_preview(&body.text, SLACK_SEARCH_BODY_PREVIEW_MAX_CHARS);
    let text_highlights = prepare_slack_search_text_highlights(
        &body,
        &body_preview,
        preview_source_end,
        highlight_query,
    );
    let attachments = slack_search_attachment_rows(message, search_generation, timezone);
    let action_target = slack_search_message_action_target(message);
    let reaction_state = message.reactions.clone().into();
    let reactions = slack_reaction_rows(&message.timestamp, &message.reactions).into();
    let show_thread_action = message.reply_count.is_some() || action_target.is_thread_reply();
    let full_timestamp_label = slack_search_full_timestamp(&message.timestamp);
    let accessibility_label = format!(
        "Message from {} in {} at {}: {}",
        message.username, message.conversation_name, full_timestamp_label, body.text
    );
    SlackSearchRow {
        result_id: message.id.clone().into(),
        result_element_id: format!("slack-search-result-{}", message.id).into(),
        action_target,
        conversation_label: slack_search_conversation_label(message),
        author: message.username.clone().into(),
        avatar_text: crate::ui::initials(&message.username).into(),
        avatar_fill: crate::ui::slack_avatar_fill(&message.username),
        avatar_image_url: message.avatar_image_url.clone().map(Into::into),
        timestamp_label: slack_search_relative_timestamp(&message.timestamp, now).into(),
        full_timestamp_label: full_timestamp_label.into(),
        body_preview,
        text_highlights,
        card_height: slack_search_card_height(&attachments, show_thread_action),
        attachments: attachments.into(),
        reaction_state,
        reactions,
        reply_count: message.reply_count,
        show_thread_action,
        latest_reply_label: message
            .latest_reply_timestamp
            .as_deref()
            .map(|timestamp| slack_search_reply_age(timestamp, now))
            .filter(|label| !label.is_empty())
            .map(Into::into),
        reply_avatar_image_urls: slack_search_reply_avatar_image_urls(message),
        accessibility_label: accessibility_label.into(),
    }
}

fn slack_search_reply_avatar_image_urls(
    message: &crate::model::SlackSearchMessage,
) -> Arc<[SharedString]> {
    message
        .reply_user_avatar_image_urls
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into()
}

fn slack_search_card_height(
    attachments: &[SlackSearchAttachmentRow],
    show_thread_action: bool,
) -> f32 {
    let attachment_height = attachments
        .iter()
        .map(|attachment| attachment.presentation.height() + 11.0)
        .sum::<f32>();
    64.0 + attachment_height + 36.0 + if show_thread_action { 48.0 } else { 0.0 }
}

fn slack_search_message_action_target(
    message: &crate::model::SlackSearchMessage,
) -> Arc<SlackMessageActionTarget> {
    match message.thread_timestamp.as_deref() {
        Some(thread_timestamp) if thread_timestamp != message.timestamp => {
            SlackMessageActionTarget::thread_reply(
                &message.team_id,
                &message.conversation_id,
                thread_timestamp,
                &message.timestamp,
            )
            .expect("validated Slack search thread target must remain valid")
        }
        _ => SlackMessageActionTarget::conversation_message(
            &message.team_id,
            &message.conversation_id,
            &message.timestamp,
        )
        .expect("validated Slack search message target must remain valid"),
    }
}

fn slack_search_attachment_rows(
    message: &crate::model::SlackSearchMessage,
    search_generation: u64,
    timezone: Tz,
) -> Vec<SlackSearchAttachmentRow> {
    message
        .attachments
        .iter()
        .enumerate()
        .map(|(attachment_index, attachment)| {
            let attachment = slack_attachment_row_with_identity(
                attachment,
                timezone,
                Some(&message.team_id),
                format!(
                    "slack-search-attachment-g{search_generation}-t{}:{}-c{}:{}-m{}:{}-a{attachment_index}",
                    message.team_id.len(),
                    message.team_id,
                    message.conversation_id.len(),
                    message.conversation_id,
                    message.id.len(),
                    message.id,
                ),
            );
            let presentation = match attachment.attachment.media.as_ref().map(|media| media.kind()) {
                Some(SlackAttachmentMediaKind::Audio) => SlackSearchAttachmentPresentation::Audio,
                Some(SlackAttachmentMediaKind::Video) => {
                    let (width, height) = slack_inline_video_dimensions(&attachment);
                    SlackSearchAttachmentPresentation::Video { width, height }
                }
                None => SlackSearchAttachmentPresentation::Summary,
            };
            SlackSearchAttachmentRow {
                host: SlackMediaHostId::search(
                    search_generation,
                    attachment.attachment_id.clone(),
                ),
                attachment,
                presentation,
                subtitle: format!("Shared by {}", message.username).into(),
            }
        })
        .collect()
}

fn slack_search_conversation_label(message: &crate::model::SlackSearchMessage) -> SharedString {
    match message.conversation_kind {
        SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => {
            format!("# {}", message.conversation_name).into()
        }
        SlackConversationKind::DirectMessage => {
            format!("Direct Message with {}", message.conversation_name).into()
        }
        SlackConversationKind::GroupMessage | SlackConversationKind::Unknown => {
            message.conversation_name.clone().into()
        }
    }
}

pub(crate) fn slack_search_relative_timestamp(timestamp: &str, now: OffsetDateTime) -> String {
    slack_relative_timestamp(slack_search_timestamp(timestamp), now)
}

pub(crate) fn slack_search_timestamp(timestamp: &str) -> OffsetDateTime {
    let seconds = timestamp
        .split('.')
        .next()
        .expect("Slack search timestamp must contain seconds")
        .parse::<i64>()
        .expect("Slack search timestamp seconds must be numeric");
    OffsetDateTime::from_unix_timestamp(seconds)
        .expect("Slack search timestamp must be in the supported date range")
}

pub(crate) fn slack_search_full_timestamp(timestamp: &str) -> String {
    let offset = UtcOffset::current_local_offset()
        .expect("The desktop must provide the local timezone offset");
    let timestamp = slack_search_timestamp(timestamp).to_offset(offset);
    let month = slack_dm_month_label(timestamp.month())
        .chars()
        .take(3)
        .collect::<String>();
    format!(
        "{month} {}{} at {}",
        timestamp.day(),
        slack_dm_ordinal_suffix(timestamp.day()),
        slack_dm_clock_label(timestamp)
    )
}

pub(crate) fn slack_quick_search_timestamp(timestamp: &str) -> String {
    let offset = UtcOffset::current_local_offset()
        .expect("The desktop must provide the local timezone offset");
    let timestamp = slack_search_timestamp(timestamp).to_offset(offset);
    let today = OffsetDateTime::now_utc().to_offset(offset).date();
    if timestamp.date() == today {
        return "Today".to_string();
    }
    if timestamp.date()
        == today
            .previous_day()
            .expect("current date must have a prior day")
    {
        return "Yesterday".to_string();
    }
    let month = slack_dm_month_label(timestamp.month())
        .chars()
        .take(3)
        .collect::<String>();
    format!(
        "{month} {}{} at {}",
        timestamp.day(),
        slack_dm_ordinal_suffix(timestamp.day()),
        slack_dm_clock_label(timestamp)
    )
}

pub(crate) fn slack_search_reply_age(timestamp: &str, now: OffsetDateTime) -> String {
    let age = now - slack_search_timestamp(timestamp);
    let hours = age.whole_hours();
    if hours < 1 {
        return "Last reply less than an hour ago".to_string();
    }
    if hours < 24 {
        return format!("Last reply {hours} hours ago");
    }
    let days = age.whole_days();
    format!(
        "Last reply {days} day{} ago",
        if days == 1 { "" } else { "s" }
    )
}
