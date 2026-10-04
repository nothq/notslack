use std::sync::Arc;

use chrono_tz::Tz;
use time::Date;

use super::attachments::{
    slack_attachment_rows_for_local_delivery, slack_attachment_rows_for_message,
    SlackAttachmentMessageScope,
};
use super::body::{
    prepare_slack_message_body_from_local_delivery, prepare_slack_message_body_from_message,
};
use super::reaction_rows::slack_reaction_rows;
use super::time::{
    slack_local_today, slack_message_avatar_label, slack_message_clock_label,
    slack_message_divider_label, slack_message_is_compact, slack_sticky_date_label,
    SlackMessageMoment,
};
use super::timeline::SlackMessageRowLayout;
use crate::ui::surface::{
    SlackComposerDocument, SlackMessageDeliveryState, SlackMessageDividerRow, SlackMessageRow,
    SlackReplyParticipantRow, SlackReplySummaryRow,
};
use crate::ui::{
    initials, slack_avatar_fill, SlackAttachment, SlackMessage, SlackMessageClientId,
    SlackMessageDraft, SlackReplyParticipant, SLACK_MESSAGE_REPLY_PARTICIPANT_VISIBLE_LIMIT,
};

struct SlackLatestReplyMetadata {
    author: Option<String>,
    user_id: Option<String>,
    avatar_text: Option<String>,
    avatar_fill: u32,
    avatar_image_url: Option<String>,
}

struct SlackMessageReplyMetadata {
    count: Option<u32>,
    timestamp: Option<String>,
    summary: Option<SlackReplySummaryRow>,
    participant_user_ids: Arc<[gpui::SharedString]>,
    participants: Arc<[SlackReplyParticipantRow]>,
    latest: SlackLatestReplyMetadata,
}

pub(super) struct SlackThreadReplyRowLayout {
    pub(super) divider_label: Option<String>,
    pub(super) date_label: Option<gpui::SharedString>,
    pub(super) compact: bool,
}

pub(crate) struct SlackLocalDeliveryRowInput<'a> {
    pub(crate) client_message_id: &'a SlackMessageClientId,
    pub(crate) author: &'a str,
    pub(crate) self_user_id: &'a str,
    pub(crate) avatar_label: Option<&'a str>,
    pub(crate) avatar_image_url: Option<&'a str>,
    pub(crate) draft: Option<&'a SlackMessageDraft>,
    pub(crate) attachments: &'a [SlackAttachment],
    pub(crate) timezone: Tz,
    pub(crate) team_id: &'a str,
    pub(crate) delivery: SlackMessageDeliveryState,
}

pub(crate) fn build_slack_local_delivery_row(
    input: SlackLocalDeliveryRowInput<'_>,
) -> SlackMessageRow {
    let SlackLocalDeliveryRowInput {
        client_message_id,
        author,
        self_user_id,
        avatar_label,
        avatar_image_url,
        draft,
        attachments,
        timezone,
        team_id,
        delivery,
    } = input;
    let (attachments, attachment_layout) = slack_attachment_rows_for_local_delivery(
        attachments,
        timezone,
        team_id,
        client_message_id.as_str(),
    );
    let avatar_text = slack_local_delivery_avatar_text(avatar_label, author);
    SlackMessageRow {
        id: format!("slack-local-delivery-{}", client_message_id.as_str()),
        author: author.to_string(),
        timestamp: String::new(),
        compact: false,
        action_target: None,
        delivery: Some(delivery),
        saved_state: None,
        user_id: Some(self_user_id.to_string()),
        avatar_text,
        avatar_fill: slack_avatar_fill(avatar_label.unwrap_or(author)),
        avatar_image_url: avatar_image_url.map(str::to_string),
        body: prepare_slack_message_body_from_local_delivery(client_message_id, draft),
        edit_round_trip_supported: false,
        table_rows: Vec::new(),
        edited_label: None,
        local_date: None,
        divider: None,
        unread_boundary_before: false,
        date_label: None,
        attachments,
        attachment_layout,
        reaction_state: Arc::default(),
        reactions: Vec::new(),
        reply_count: None,
        latest_reply_timestamp: None,
        reply_summary: None,
        reply_participant_user_ids: Arc::default(),
        reply_participants: Arc::default(),
        latest_reply_author: None,
        latest_reply_user_id: None,
        latest_reply_avatar_text: None,
        latest_reply_avatar_fill: 0,
        latest_reply_avatar_image_url: None,
        replies: Vec::new(),
    }
}

fn slack_local_delivery_avatar_text(avatar_label: Option<&str>, author: &str) -> String {
    avatar_label
        .map(str::to_string)
        .unwrap_or_else(|| initials(author))
}

pub(super) fn slack_message_row(
    message: &SlackMessage,
    layout: SlackMessageRowLayout,
    timezone: Tz,
    attachment_scope: SlackAttachmentMessageScope<'_>,
) -> SlackMessageRow {
    let divider = slack_message_divider_row(&message.id, layout.divider_label, layout.local_date);
    let body = prepare_slack_message_body_from_message(message);
    let (attachments, attachment_layout) =
        slack_attachment_rows_for_message(message, timezone, attachment_scope);
    let reply = slack_message_reply_metadata(message);
    SlackMessageRow {
        id: message.id.clone(),
        author: message.author.clone(),
        timestamp: slack_message_clock_label(message, timezone),
        compact: layout.compact,
        action_target: None,
        delivery: None,
        saved_state: message.saved_state,
        user_id: message.user_id.clone(),
        avatar_text: slack_message_avatar_label(message),
        avatar_fill: slack_message_avatar_fill(message),
        avatar_image_url: message.avatar_image_url.clone(),
        body,
        edit_round_trip_supported: SlackComposerDocument::from_message(message).is_some(),
        table_rows: message.table_rows.clone(),
        edited_label: message.edited_label.clone().map(Into::into),
        local_date: layout.local_date,
        divider,
        unread_boundary_before: layout.unread_boundary_before,
        date_label: layout.date_label,
        attachments,
        attachment_layout,
        reaction_state: message.reactions.clone().into(),
        reactions: slack_reaction_rows(&message.id, &message.reactions),
        reply_count: reply.count,
        latest_reply_timestamp: reply.timestamp,
        reply_summary: reply.summary,
        reply_participant_user_ids: reply.participant_user_ids,
        reply_participants: reply.participants,
        latest_reply_author: reply.latest.author,
        latest_reply_user_id: reply.latest.user_id,
        latest_reply_avatar_text: reply.latest.avatar_text,
        latest_reply_avatar_fill: reply.latest.avatar_fill,
        latest_reply_avatar_image_url: reply.latest.avatar_image_url,
        replies: build_slack_thread_reply_rows(&message.replies, timezone, attachment_scope),
    }
}

fn slack_message_reply_metadata(message: &SlackMessage) -> SlackMessageReplyMetadata {
    let latest = message.replies.last();
    SlackMessageReplyMetadata {
        count: message.reply_count,
        timestamp: message.latest_reply_timestamp.clone(),
        summary: message.reply_count.map(|reply_count| {
            SlackReplySummaryRow::new(
                &message.id,
                reply_count,
                message
                    .latest_reply_timestamp
                    .as_deref()
                    .filter(|timestamp| !timestamp.is_empty())
                    .unwrap_or(&message.timestamp),
            )
        }),
        participant_user_ids: message
            .reply_participants
            .iter()
            .map(|participant| participant.user_id.clone().into())
            .collect::<Vec<_>>()
            .into(),
        participants: message
            .reply_participants
            .iter()
            .take(SLACK_MESSAGE_REPLY_PARTICIPANT_VISIBLE_LIMIT)
            .map(|participant| slack_reply_participant_row(&message.id, participant))
            .collect::<Vec<_>>()
            .into(),
        latest: SlackLatestReplyMetadata {
            author: latest.map(|reply| reply.author.clone()),
            user_id: latest.and_then(|reply| reply.user_id.clone()),
            avatar_text: latest.map(slack_message_avatar_label),
            avatar_fill: latest.map_or(0, slack_message_avatar_fill),
            avatar_image_url: latest.and_then(|reply| reply.avatar_image_url.clone()),
        },
    }
}

fn slack_message_avatar_fill(message: &SlackMessage) -> u32 {
    slack_avatar_fill(
        message
            .avatar_label
            .as_deref()
            .unwrap_or(message.author.as_str()),
    )
}

fn build_slack_thread_reply_rows(
    messages: &[SlackMessage],
    timezone: Tz,
    attachment_scope: SlackAttachmentMessageScope<'_>,
) -> Vec<SlackMessageRow> {
    let mut rows = Vec::with_capacity(messages.len());
    let mut previous_message = None;
    let mut previous_moment = None;
    let mut date_label = None;
    let today = slack_local_today(timezone);
    for (index, message) in messages.iter().enumerate() {
        let moment = SlackMessageMoment::parse(&message.id, timezone);
        let divider_label = message.date_divider_label.clone().or_else(|| {
            if index == 0 && !message.timestamp.is_empty() {
                Some("Replies".to_string())
            } else {
                slack_message_divider_label(
                    previous_message,
                    previous_moment,
                    message,
                    moment,
                    today,
                )
            }
        });
        if let Some(moment) = moment {
            date_label = Some(slack_sticky_date_label(moment.local_date, today).into());
        } else if let Some(divider_label) = divider_label.as_ref() {
            date_label = Some(divider_label.clone().into());
        }
        let compact = divider_label.is_none()
            && slack_message_is_compact(previous_message, previous_moment, message, moment);
        rows.push(slack_thread_reply_row(
            message,
            SlackThreadReplyRowLayout {
                divider_label,
                date_label: date_label.clone(),
                compact,
            },
            timezone,
            attachment_scope,
        ));
        previous_message = Some(message);
        previous_moment = moment;
    }
    rows
}

pub(super) fn slack_thread_reply_row(
    message: &SlackMessage,
    layout: SlackThreadReplyRowLayout,
    timezone: Tz,
    attachment_scope: SlackAttachmentMessageScope<'_>,
) -> SlackMessageRow {
    let divider = slack_message_divider_row(&message.id, layout.divider_label, None);
    let body = prepare_slack_message_body_from_message(message);
    let (attachments, attachment_layout) =
        slack_attachment_rows_for_message(message, timezone, attachment_scope);
    SlackMessageRow {
        id: message.id.clone(),
        author: message.author.clone(),
        timestamp: slack_message_clock_label(message, timezone),
        compact: layout.compact,
        action_target: None,
        delivery: None,
        saved_state: message.saved_state,
        user_id: message.user_id.clone(),
        avatar_text: slack_message_avatar_label(message),
        avatar_fill: slack_message_avatar_fill(message),
        avatar_image_url: message.avatar_image_url.clone(),
        body,
        edit_round_trip_supported: SlackComposerDocument::from_message(message).is_some(),
        table_rows: message.table_rows.clone(),
        edited_label: message.edited_label.clone().map(Into::into),
        local_date: None,
        divider,
        unread_boundary_before: false,
        date_label: layout.date_label,
        attachments,
        attachment_layout,
        reaction_state: message.reactions.clone().into(),
        reactions: slack_reaction_rows(&message.id, &message.reactions),
        reply_count: None,
        latest_reply_timestamp: None,
        reply_summary: None,
        reply_participant_user_ids: Arc::default(),
        reply_participants: Arc::default(),
        latest_reply_author: None,
        latest_reply_user_id: None,
        latest_reply_avatar_text: None,
        latest_reply_avatar_fill: 0,
        latest_reply_avatar_image_url: None,
        replies: Vec::new(),
    }
}

fn slack_message_divider_row(
    message_id: &str,
    label: Option<String>,
    local_date: Option<Date>,
) -> Option<SlackMessageDividerRow> {
    label.map(|label| SlackMessageDividerRow {
        local_date,
        element_id: format!("slack-message-divider-{message_id}").into(),
        accessibility_label: local_date.map_or_else(
            || label.clone().into(),
            |_| format!("Jump to another date from {label}").into(),
        ),
        label: label.into(),
    })
}

fn slack_reply_participant_row(
    message_id: &str,
    participant: &SlackReplyParticipant,
) -> SlackReplyParticipantRow {
    SlackReplyParticipantRow {
        profile_element_id: format!(
            "slack-reply-participant-{message_id}-{}",
            participant.user_id
        )
        .into(),
        profile_accessibility_label: format!("View {}’s profile", participant.display_name).into(),
        user_id: participant.user_id.clone().into(),
        avatar_text: participant.avatar_label.clone().into(),
        avatar_fill: slack_avatar_fill(&participant.display_name),
        avatar_image_url: participant.avatar_image_url.clone().map(Into::into),
    }
}
