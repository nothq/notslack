mod loading;
mod open;
mod read;

use std::{cmp::Ordering, ops::Range, sync::Arc, time::Duration};

use chrono::{TimeZone, Utc};
use chrono_tz::Tz;
use gpui::{ListAlignment, ListOffset, ListScrollEvent, ListState, SharedString};

use super::super::{
    slack_message_timezone, slack_thread_broadcast_label, SlackLaterThreadTarget,
    SlackThreadListRow, SlackThreadPanelOrigin,
};
use super::{
    prepare_slack_thread_snapshot, px, Context, PreparedSlackThreadSnapshot,
    SlackComposerFormatAction, SlackMessageRow, SlackRailView, SlackThreadPanelState, SurfaceState,
    WorkspaceApi,
};
use crate::ui::SlackMessageTimestamp;

use super::workspace::{
    slack_attachment_remote_image_urls, slack_message_reaction_remote_image_urls,
};

const SLACK_THREAD_LIST_OVERDRAW: f32 = 360.0;
const SLACK_THREAD_INITIAL_IMAGE_ROW_LIMIT: usize = 8;
const SLACK_THREAD_PAGINATION_THRESHOLD: usize = 4;
const SLACK_THREAD_READ_DEBOUNCE: Duration = Duration::from_millis(350);
const SLACK_THREAD_IMAGE_LEADING_ROWS: usize = 2;
const SLACK_THREAD_IMAGE_TRAILING_ROWS: usize = 4;
const SLACK_THREAD_IMAGE_PREFETCH_URL_LIMIT: usize = 24;

#[derive(Clone)]
struct SlackThreadRequest {
    generation: u64,
    team_id: String,
    conversation_id: String,
    parent_message_id: String,
    thread_timestamp: SlackMessageTimestamp,
    cursor: Option<String>,
    replace_existing: bool,
}

impl SurfaceState {}

pub(super) fn normalize_slack_thread_parent_row(row: &mut SlackMessageRow) {
    row.compact = false;
    row.divider = None;
    row.unread_boundary_before = false;
    row.date_label = None;
    row.reply_count = None;
    row.latest_reply_timestamp = None;
    row.reply_summary = None;
    row.reply_participant_user_ids = Arc::default();
    row.reply_participants = Arc::default();
    row.latest_reply_author = None;
    row.latest_reply_user_id = None;
    row.latest_reply_avatar_text = None;
    row.latest_reply_avatar_image_url = None;
    row.replies.clear();
}

fn normalize_slack_thread_reply_row(row: &mut SlackMessageRow) {
    row.divider = None;
    row.unread_boundary_before = false;
    row.date_label = None;
}

fn shape_slack_thread_reply_rows(rows: &mut [SlackMessageRow], timezone: Tz) {
    for index in 0..rows.len() {
        let (previous_rows, current_rows) = rows.split_at_mut(index);
        let current = current_rows
            .first_mut()
            .expect("Slack thread reply index should exist");
        let compact = previous_rows.last().is_some_and(|previous| {
            slack_thread_rows_have_same_sender(previous, current)
                && slack_thread_rows_are_compact_neighbors(previous, current, timezone)
        });
        current.compact = compact;
        current.divider = None;
        current.unread_boundary_before = false;
        current.date_label = None;
    }
}

fn slack_thread_rows_have_same_sender(left: &SlackMessageRow, right: &SlackMessageRow) -> bool {
    match (left.user_id.as_deref(), right.user_id.as_deref()) {
        (Some(left_user_id), Some(right_user_id)) => left_user_id == right_user_id,
        _ => !right.author.trim().is_empty() && left.author.trim() == right.author.trim(),
    }
}

fn slack_thread_rows_are_compact_neighbors(
    left: &SlackMessageRow,
    right: &SlackMessageRow,
    timezone: Tz,
) -> bool {
    const FIVE_MINUTES_NANOS: u128 = 5 * 60 * 1_000_000_000;
    let Some((left_seconds, left_nanoseconds)) = slack_timestamp_sort_key(&left.id) else {
        return false;
    };
    let Some((right_seconds, right_nanoseconds)) = slack_timestamp_sort_key(&right.id) else {
        return false;
    };
    let left = u128::from(left_seconds) * 1_000_000_000 + u128::from(left_nanoseconds);
    let right = u128::from(right_seconds) * 1_000_000_000 + u128::from(right_nanoseconds);
    right >= left
        && right - left <= FIVE_MINUTES_NANOS
        && slack_thread_local_date(left_seconds, left_nanoseconds, timezone)
            == slack_thread_local_date(right_seconds, right_nanoseconds, timezone)
}

fn slack_thread_local_date(
    seconds: u64,
    nanoseconds: u32,
    timezone: Tz,
) -> Option<chrono::NaiveDate> {
    let seconds = i64::try_from(seconds).ok()?;
    Utc.timestamp_opt(seconds, nanoseconds)
        .single()
        .map(|timestamp| timestamp.with_timezone(&timezone).date_naive())
}

fn slack_thread_reply_label(reply_count: u32) -> SharedString {
    match reply_count {
        0 => "".into(),
        1 => "1 reply".into(),
        count => format!("{count} replies").into(),
    }
}

struct SlackThreadListLayout {
    is_conversation: bool,
    show_parent: bool,
    expected_reply_count: u32,
    reply_count: usize,
    loading: bool,
    has_error: bool,
    show_composer: bool,
}

fn build_slack_thread_list_rows(layout: SlackThreadListLayout) -> Arc<[SlackThreadListRow]> {
    let SlackThreadListLayout {
        is_conversation,
        show_parent,
        expected_reply_count,
        reply_count,
        loading,
        has_error,
        show_composer,
    } = layout;
    if !is_conversation {
        return (0..reply_count)
            .map(SlackThreadListRow::Reply)
            .collect::<Vec<_>>()
            .into();
    }
    let mut rows = Vec::with_capacity(
        usize::from(show_parent)
            + usize::from(expected_reply_count > 0)
            + reply_count
            + usize::from(loading || has_error)
            + usize::from(show_composer),
    );
    if show_parent {
        rows.push(SlackThreadListRow::Parent);
    }
    if expected_reply_count > 0 {
        rows.push(SlackThreadListRow::ReplyDivider);
    }
    rows.extend((0..reply_count).map(SlackThreadListRow::Reply));
    if loading {
        rows.push(SlackThreadListRow::Loading);
    } else if has_error {
        rows.push(SlackThreadListRow::Error);
    }
    if show_composer {
        rows.push(SlackThreadListRow::Composer);
    }
    rows.into()
}

pub(super) fn refresh_slack_thread_list_rows(
    panel: &mut SlackThreadPanelState,
    show_composer: bool,
) {
    panel.reply_label = slack_thread_reply_label(panel.expected_reply_count);
    let next_rows = build_slack_thread_list_rows(SlackThreadListLayout {
        is_conversation: panel.origin.is_conversation(),
        show_parent: panel.parent_hydrated,
        expected_reply_count: panel.expected_reply_count,
        reply_count: panel.reply_rows.len(),
        loading: panel.loading,
        has_error: panel.error.is_some(),
        show_composer,
    });
    if panel.list_rows == next_rows {
        panel.list_state.remeasure();
        return;
    }
    let common_prefix = panel
        .list_rows
        .iter()
        .zip(next_rows.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let common_suffix = panel.list_rows[common_prefix..]
        .iter()
        .rev()
        .zip(next_rows[common_prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let old_end = panel.list_rows.len() - common_suffix;
    let inserted_count = next_rows.len() - common_prefix - common_suffix;
    panel
        .list_state
        .splice(common_prefix..old_end, inserted_count);
    panel.list_rows = next_rows;
}

pub(super) fn slack_thread_reply_list_index(
    panel: &SlackThreadPanelState,
    reply_index: usize,
) -> Option<usize> {
    panel
        .list_rows
        .iter()
        .position(|row| matches!(row, SlackThreadListRow::Reply(index) if *index == reply_index))
}

fn slack_thread_row_remote_image_urls(row: &SlackMessageRow) -> impl Iterator<Item = String> + '_ {
    row.avatar_image_url
        .iter()
        .cloned()
        .chain(slack_message_reaction_remote_image_urls(row).map(str::to_string))
        .chain(
            row.attachments
                .iter()
                .flat_map(|attachment| slack_attachment_remote_image_urls(&attachment.attachment))
                .map(str::to_string),
        )
}

pub(super) fn merge_slack_thread_rows(
    existing: &[SlackMessageRow],
    incoming: &[SlackMessageRow],
    timezone: Tz,
) -> Vec<SlackMessageRow> {
    let mut incoming = incoming.to_vec();
    incoming.sort_by(slack_thread_row_order);
    incoming.dedup_by(|left, right| left.id == right.id);
    let mut merged = Vec::with_capacity(existing.len() + incoming.len());
    let mut existing_index = 0;
    let mut incoming_index = 0;
    while existing_index < existing.len() && incoming_index < incoming.len() {
        match slack_thread_row_order(&existing[existing_index], &incoming[incoming_index]) {
            Ordering::Less => {
                merged.push(existing[existing_index].clone());
                existing_index += 1;
            }
            Ordering::Greater => {
                merged.push(incoming[incoming_index].clone());
                incoming_index += 1;
            }
            Ordering::Equal => {
                merged.push(incoming[incoming_index].clone());
                existing_index += 1;
                incoming_index += 1;
            }
        }
    }
    merged.extend(existing[existing_index..].iter().cloned());
    merged.extend(incoming[incoming_index..].iter().cloned());
    shape_slack_thread_reply_rows(&mut merged, timezone);
    merged
}

fn slack_thread_row_order(left: &SlackMessageRow, right: &SlackMessageRow) -> Ordering {
    match (
        slack_timestamp_sort_key(&left.id),
        slack_timestamp_sort_key(&right.id),
    ) {
        (Some(left_key), Some(right_key)) => left_key
            .cmp(&right_key)
            .then_with(|| left.id.cmp(&right.id)),
        _ => left.id.cmp(&right.id),
    }
}

fn slack_timestamp_sort_key(value: &str) -> Option<(u64, u32)> {
    let (seconds, fractional) = value.split_once('.')?;
    if fractional.is_empty() || fractional.len() > 9 {
        return None;
    }
    let seconds = seconds.parse().ok()?;
    let fractional_digits = fractional.len();
    let fractional = fractional.parse::<u32>().ok()?;
    let scale = 10_u32.checked_pow(u32::try_from(9 - fractional_digits).ok()?)?;
    Some((seconds, fractional.checked_mul(scale)?))
}
