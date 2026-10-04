use std::sync::Arc;

use chrono_tz::Tz;
use time::Date;

use super::attachments::SlackAttachmentMessageScope;
use super::construction::{slack_message_row, slack_thread_reply_row, SlackThreadReplyRowLayout};
use super::time::{
    slack_local_today, slack_message_divider_label, slack_message_is_compact,
    slack_message_timezone, slack_sticky_date_label, SlackMessageMoment,
};
use crate::ui::surface::{SlackMessageActionTarget, SlackMessageChunk, SlackMessageRow};
use crate::ui::{SlackConversationSnapshot, SlackLastReadTimestamp, SlackMessage, SlackWorkspace};

pub(super) struct SlackMessageRowLayout {
    pub(super) divider_label: Option<String>,
    pub(super) local_date: Option<Date>,
    pub(super) unread_boundary_before: bool,
    pub(super) date_label: Option<gpui::SharedString>,
    pub(super) compact: bool,
}

pub(crate) struct SlackAppendedMessageRowInput<'a> {
    pub(crate) previous_message: Option<&'a SlackMessage>,
    pub(crate) message: &'a SlackMessage,
    pub(crate) timezone: Tz,
    pub(crate) local_today: Date,
    pub(crate) team_id: &'a str,
    pub(crate) conversation_id: &'a str,
}

pub(crate) type SlackOptionalDatedMessageRows = (Arc<[SlackMessageRow]>, Option<Date>);
pub(crate) type SlackDatedMessageRows = (Arc<[SlackMessageRow]>, Date);

#[cfg(test)]
pub(crate) fn build_slack_message_rows(
    workspace: Option<&SlackWorkspace>,
) -> Arc<[SlackMessageRow]> {
    build_slack_message_rows_with_local_today(workspace).0
}

pub(crate) fn build_slack_message_rows_with_local_today(
    workspace: Option<&SlackWorkspace>,
) -> SlackOptionalDatedMessageRows {
    let Some(workspace) = workspace else {
        return (
            Arc::<[SlackMessageRow]>::from(Vec::<SlackMessageRow>::new()),
            None,
        );
    };
    let timezone = slack_message_timezone(workspace.self_timezone_id.as_deref());
    let local_today = slack_local_today(timezone);
    (
        build_slack_message_rows_from_messages(
            &workspace.messages,
            workspace
                .last_read_boundary_loaded
                .then_some(workspace.last_read.as_ref())
                .flatten(),
            timezone,
            local_today,
            SlackAttachmentMessageScope::new(&workspace.team_id, &workspace.conversation_id),
        ),
        Some(local_today),
    )
}

pub(crate) fn build_slack_conversation_message_rows_with_local_today(
    conversation: &SlackConversationSnapshot,
) -> SlackDatedMessageRows {
    let timezone = slack_message_timezone(conversation.self_timezone_id.as_deref());
    let local_today = slack_local_today(timezone);
    (
        build_slack_message_rows_from_messages(
            &conversation.messages,
            conversation
                .last_read_boundary_loaded
                .then_some(conversation.last_read.as_ref())
                .flatten(),
            timezone,
            local_today,
            SlackAttachmentMessageScope::new(&conversation.team_id, &conversation.conversation_id),
        ),
        local_today,
    )
}

pub(crate) fn build_slack_appended_message_row_with_local_today(
    input: SlackAppendedMessageRowInput<'_>,
) -> SlackMessageRow {
    let SlackAppendedMessageRowInput {
        previous_message,
        message,
        timezone,
        local_today,
        team_id,
        conversation_id,
    } = input;
    let previous_moment =
        previous_message.and_then(|message| SlackMessageMoment::parse(&message.id, timezone));
    let moment = SlackMessageMoment::parse(&message.id, timezone);
    let divider_label = slack_message_divider_label(
        previous_message,
        previous_moment,
        message,
        moment,
        local_today,
    );
    let date_label = moment
        .map(|moment| slack_sticky_date_label(moment.local_date, local_today).into())
        .or_else(|| divider_label.clone().map(Into::into));
    let compact = divider_label.is_none()
        && slack_message_is_compact(previous_message, previous_moment, message, moment);
    let mut row = slack_message_row(
        message,
        SlackMessageRowLayout {
            divider_label,
            local_date: moment.map(|moment| moment.local_date),
            unread_boundary_before: false,
            date_label,
            compact,
        },
        timezone,
        SlackAttachmentMessageScope::new(team_id, conversation_id),
    );
    assign_conversation_message_action_targets(&mut row, team_id, conversation_id);
    row
}

pub(crate) fn build_slack_pinned_message_row(
    message: &SlackMessage,
    team_id: &str,
    conversation_id: &str,
) -> SlackMessageRow {
    slack_message_row(
        message,
        SlackMessageRowLayout {
            divider_label: None,
            local_date: None,
            unread_boundary_before: false,
            date_label: None,
            compact: false,
        },
        chrono_tz::UTC,
        SlackAttachmentMessageScope::new(team_id, conversation_id),
    )
}

pub(crate) fn build_slack_thread_parent_row(
    message: &SlackMessage,
    team_id: &str,
    conversation_id: &str,
) -> SlackMessageRow {
    let mut row = slack_message_row(
        message,
        SlackMessageRowLayout {
            divider_label: None,
            local_date: None,
            unread_boundary_before: false,
            date_label: None,
            compact: false,
        },
        chrono_tz::UTC,
        SlackAttachmentMessageScope::new(team_id, conversation_id),
    );
    assign_conversation_message_action_targets(&mut row, team_id, conversation_id);
    row
}

pub(crate) fn build_slack_thread_parent_row_in_timezone(
    message: &SlackMessage,
    timezone: Tz,
    team_id: &str,
    conversation_id: &str,
) -> SlackMessageRow {
    let mut row = slack_message_row(
        message,
        SlackMessageRowLayout {
            divider_label: None,
            local_date: None,
            unread_boundary_before: false,
            date_label: None,
            compact: false,
        },
        timezone,
        SlackAttachmentMessageScope::new(team_id, conversation_id),
    );
    assign_conversation_message_action_targets(&mut row, team_id, conversation_id);
    row
}

pub(crate) fn build_slack_thread_page_reply_rows_in_timezone(
    messages: &[SlackMessage],
    timezone: Tz,
    team_id: &str,
    conversation_id: &str,
    thread_timestamp: &str,
) -> Arc<[SlackMessageRow]> {
    messages
        .iter()
        .map(|message| {
            let mut row = slack_thread_reply_row(
                message,
                SlackThreadReplyRowLayout {
                    divider_label: None,
                    date_label: None,
                    compact: false,
                },
                timezone,
                SlackAttachmentMessageScope::new(team_id, conversation_id),
            );
            row.action_target = SlackMessageActionTarget::thread_reply(
                team_id,
                conversation_id,
                thread_timestamp,
                &row.id,
            );
            row
        })
        .collect::<Vec<_>>()
        .into()
}

fn build_slack_message_rows_from_messages(
    messages: &[SlackMessage],
    last_read: Option<&SlackLastReadTimestamp>,
    timezone: Tz,
    today: Date,
    attachment_scope: SlackAttachmentMessageScope<'_>,
) -> Arc<[SlackMessageRow]> {
    let mut rows = Vec::with_capacity(messages.len());
    let mut previous_message = None;
    let mut previous_moment = None;
    let mut date_label = None;
    let mut unread_boundary_pending = last_read.is_some();
    let last_read_key = last_read.map(SlackLastReadTimestamp::sort_key);
    for message in messages {
        let moment = SlackMessageMoment::parse(&message.id, timezone);
        let divider_label =
            slack_message_divider_label(previous_message, previous_moment, message, moment, today);
        if let Some(moment) = moment {
            date_label = Some(slack_sticky_date_label(moment.local_date, today).into());
        } else if let Some(divider_label) = divider_label.as_ref() {
            date_label = Some(divider_label.clone().into());
        }
        let unread_boundary_before = unread_boundary_pending
            && last_read_key
                .zip(moment.map(|moment| moment.sort_key))
                .is_some_and(|(last_read, message)| message > last_read);
        unread_boundary_pending &= !unread_boundary_before;
        let compact = !unread_boundary_before
            && divider_label.is_none()
            && slack_message_is_compact(previous_message, previous_moment, message, moment);
        let mut row = slack_message_row(
            message,
            SlackMessageRowLayout {
                divider_label,
                local_date: moment.map(|moment| moment.local_date),
                unread_boundary_before,
                date_label: date_label.clone(),
                compact,
            },
            timezone,
            attachment_scope,
        );
        assign_conversation_message_action_targets(
            &mut row,
            attachment_scope.team_id,
            attachment_scope.conversation_id,
        );
        rows.push(row);
        previous_message = Some(message);
        previous_moment = moment;
    }
    Arc::from(rows)
}

fn assign_conversation_message_action_targets(
    row: &mut SlackMessageRow,
    team_id: &str,
    conversation_id: &str,
) {
    row.action_target =
        SlackMessageActionTarget::conversation_message(team_id, conversation_id, &row.id);
    let thread_timestamp = row.id.clone();
    for reply in &mut row.replies {
        reply.action_target = SlackMessageActionTarget::thread_reply(
            team_id,
            conversation_id,
            &thread_timestamp,
            &reply.id,
        );
    }
}

pub(crate) fn build_slack_message_chunks(rows: &[SlackMessageRow]) -> Arc<[SlackMessageChunk]> {
    const SLACK_MESSAGE_CHUNK_MAX_UNITS: usize = 10;

    let mut chunks = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_units = 0usize;

    for (index, row) in rows.iter().enumerate() {
        if index > chunk_start && (row.divider.is_some() || row.unread_boundary_before) {
            chunks.push(SlackMessageChunk {
                row_range: chunk_start..index,
            });
            chunk_start = index;
            chunk_units = 0;
        }

        let row_units = slack_message_chunk_units(row);
        let row_requires_own_chunk = slack_message_requires_own_chunk(row);

        if index > chunk_start
            && (row_requires_own_chunk || chunk_units + row_units > SLACK_MESSAGE_CHUNK_MAX_UNITS)
        {
            chunks.push(SlackMessageChunk {
                row_range: chunk_start..index,
            });
            chunk_start = index;
            chunk_units = 0;
        }

        chunk_units += row_units;

        if row_requires_own_chunk {
            chunks.push(SlackMessageChunk {
                row_range: chunk_start..(index + 1),
            });
            chunk_start = index + 1;
            chunk_units = 0;
        }
    }

    if chunk_start < rows.len() {
        chunks.push(SlackMessageChunk {
            row_range: chunk_start..rows.len(),
        });
    }

    Arc::from(chunks)
}

fn slack_message_chunk_units(row: &SlackMessageRow) -> usize {
    let body_units = row.body.len() / 320;
    let table_units = row.table_rows.len().div_ceil(4);
    let reaction_units = row.reactions.len().div_ceil(4);
    let attachment_units = row.attachments.len() * 3;
    usize::from(row.divider.is_some())
        + usize::from(row.unread_boundary_before)
        + 1
        + body_units
        + table_units
        + reaction_units
        + attachment_units
}

fn slack_message_requires_own_chunk(row: &SlackMessageRow) -> bool {
    !row.attachments.is_empty()
        || !row.table_rows.is_empty()
        || row.reactions.len() > 4
        || row.body.len() > 720
}
