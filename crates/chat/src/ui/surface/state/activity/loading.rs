use std::{collections::HashSet, sync::Arc};

use super::{
    merge_and_prepare_slack_activity_snapshot, prepare_slack_conversation_snapshot,
    prepare_slack_thread_snapshot, slack_message_reaction_remote_image_urls,
    PreparedSlackActivityDetail, PreparedSlackActivitySnapshot, SlackActivityDetailRequest,
    SlackActivityPageRequest, SlackMessageRow, SlackMessageTimestamp, WorkspaceApi,
};
use crate::ui::SlackThreadSnapshot;

/// Twenty replies a page: enough for a thread of two hundred.
const SLACK_ACTIVITY_THREAD_PAGE_LIMIT: usize = 10;

pub(super) fn load_slack_activity_page(
    workspace_api: Arc<dyn WorkspaceApi>,
    mut request: SlackActivityPageRequest,
) -> (
    SlackActivityPageRequest,
    Result<PreparedSlackActivitySnapshot, String>,
) {
    let existing = request.existing.take();
    let result = workspace_api
        .load_slack_activity(request.cursor.as_ref())
        .map(|page| merge_and_prepare_slack_activity_snapshot(existing, page, &request.workspace));
    (request, result)
}

pub(super) fn load_slack_activity_detail(
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackActivityDetailRequest,
) -> (
    SlackActivityDetailRequest,
    Result<PreparedSlackActivityDetail, String>,
) {
    let result = match request.thread_timestamp.as_ref() {
        Some(thread_timestamp) => load_slack_activity_thread(
            workspace_api.as_ref(),
            request.channel_id.as_ref(),
            thread_timestamp,
            &request.message_timestamp,
        )
        .map(prepare_slack_thread_snapshot)
        .map(Box::new)
        .map(PreparedSlackActivityDetail::Thread),
        None => workspace_api
            .load_slack_conversation_at(request.channel_id.as_ref(), &request.message_timestamp)
            .map(prepare_slack_conversation_snapshot)
            .map(Box::new)
            .map(PreparedSlackActivityDetail::Conversation),
    };
    (request, result)
}

pub(super) fn find_activity_anchor_index(
    rows: &[SlackMessageRow],
    message_timestamp: &SlackMessageTimestamp,
) -> Option<usize> {
    rows.iter()
        .position(|row| slack_activity_row_contains_anchor(row, message_timestamp))
}

fn slack_activity_row_contains_anchor(
    row: &SlackMessageRow,
    message_timestamp: &SlackMessageTimestamp,
) -> bool {
    row.id == message_timestamp.as_str()
        || row
            .replies
            .iter()
            .any(|reply| slack_activity_row_contains_anchor(reply, message_timestamp))
}

/// Loads the thread from its first reply through the page holding
/// `reply_timestamp`, so the detail shows the conversation that led to it. A
/// thread that is too long for that, or that fails to load whole, falls back
/// to the parent and that one reply.
fn load_slack_activity_thread(
    workspace_api: &dyn WorkspaceApi,
    channel_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
    reply_timestamp: &SlackMessageTimestamp,
) -> Result<SlackThreadSnapshot, String> {
    match load_slack_thread_through_reply(
        workspace_api,
        channel_id,
        thread_timestamp,
        reply_timestamp,
    ) {
        Ok(Some(thread)) => return Ok(thread),
        Ok(None) => {}
        Err(error) => eprintln!("Slack Activity thread load failed: {error}"),
    }
    workspace_api.load_slack_thread_containing_reply(channel_id, thread_timestamp, reply_timestamp)
}

/// The thread's pages from the first, joined, up to the one holding
/// `reply_timestamp`; `None` when the page limit comes first.
fn load_slack_thread_through_reply(
    workspace_api: &dyn WorkspaceApi,
    channel_id: &str,
    thread_timestamp: &SlackMessageTimestamp,
    reply_timestamp: &SlackMessageTimestamp,
) -> Result<Option<SlackThreadSnapshot>, String> {
    let mut thread = workspace_api.load_slack_thread(channel_id, thread_timestamp, None)?;
    for _ in 1..SLACK_ACTIVITY_THREAD_PAGE_LIMIT {
        if slack_thread_holds_reply(&thread, reply_timestamp) {
            break;
        }
        let Some(cursor) = thread.next_cursor.take() else {
            break;
        };
        let page = workspace_api.load_slack_thread(channel_id, thread_timestamp, Some(&cursor))?;
        thread.replies.extend(page.replies);
        thread.next_cursor = page.next_cursor;
    }
    Ok(slack_thread_holds_reply(&thread, reply_timestamp).then_some(thread))
}

fn slack_thread_holds_reply(
    thread: &SlackThreadSnapshot,
    reply_timestamp: &SlackMessageTimestamp,
) -> bool {
    thread
        .replies
        .iter()
        .any(|reply| reply.id == reply_timestamp.as_str())
}

/// The thread as the detail lists it: the parent, then every reply, with the
/// index of the reply the activity item is about.
pub(super) fn slack_activity_thread_detail_rows(
    mut parent: SlackMessageRow,
    replies: &[SlackMessageRow],
    reply_timestamp: &SlackMessageTimestamp,
) -> Option<(Arc<[SlackMessageRow]>, usize)> {
    let reply_index = replies
        .iter()
        .position(|reply| reply.id == reply_timestamp.as_str())?;
    parent.replies.clear();
    let rows = std::iter::once(parent)
        .chain(replies.iter().cloned().map(|mut reply| {
            reply.replies.clear();
            reply
        }))
        .collect();
    Some((rows, reply_index + 1))
}

pub(super) fn collect_slack_activity_detail_row_urls(
    row: &SlackMessageRow,
    urls: &mut HashSet<String>,
) {
    if let Some(url) = row.avatar_image_url.as_ref() {
        urls.insert(url.clone());
    }
    urls.extend(slack_message_reaction_remote_image_urls(row).map(str::to_string));
    urls.extend(row.reply_participants.iter().filter_map(|participant| {
        participant
            .avatar_image_url
            .as_ref()
            .map(ToString::to_string)
    }));
    for attachment in &row.attachments {
        if let Some(url) = attachment.attachment.preview_image_url.as_ref() {
            urls.insert(url.clone());
        }
    }
    for reply in &row.replies {
        collect_slack_activity_detail_row_urls(reply, urls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, replies: Vec<SlackMessageRow>) -> SlackMessageRow {
        SlackMessageRow {
            id: id.to_string(),
            author: "notslack".to_string(),
            timestamp: "10:00 AM".to_string(),
            compact: false,
            action_target: None,
            delivery: None,
            saved_state: None,
            user_id: None,
            avatar_text: "O".to_string(),
            avatar_fill: 0,
            avatar_image_url: None,
            body: Default::default(),
            edit_round_trip_supported: true,
            table_rows: Vec::new(),
            edited_label: None,
            local_date: None,
            divider: None,
            unread_boundary_before: false,
            date_label: None,
            attachments: Vec::new(),
            attachment_layout: Default::default(),
            reaction_state: Default::default(),
            reactions: Vec::new(),
            reply_count: None,
            latest_reply_timestamp: None,
            reply_summary: None,
            reply_participant_user_ids: Default::default(),
            reply_participants: Default::default(),
            latest_reply_author: None,
            latest_reply_user_id: None,
            latest_reply_avatar_text: None,
            latest_reply_avatar_fill: 0,
            latest_reply_avatar_image_url: None,
            replies,
        }
    }

    fn timestamp(raw: &str) -> SlackMessageTimestamp {
        SlackMessageTimestamp::parse(raw).expect("test timestamp should parse")
    }

    #[test]
    fn the_thread_detail_lists_every_reply_and_points_at_the_activity_one() {
        let parent = row(
            "1700000000.000100",
            vec![row("1700000001.000100", Vec::new())],
        );
        let replies = [
            row("1700000001.000100", Vec::new()),
            row("1700000002.000100", Vec::new()),
            row("1700000003.000100", Vec::new()),
        ];
        let (rows, anchor_index) =
            slack_activity_thread_detail_rows(parent, &replies, &timestamp("1700000002.000100"))
                .expect("the thread holds the activity reply");
        let ids = rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "1700000000.000100",
                "1700000001.000100",
                "1700000002.000100",
                "1700000003.000100",
            ]
        );
        assert_eq!(anchor_index, 2);
        assert!(rows[0].replies.is_empty());
    }

    #[test]
    fn a_thread_without_the_activity_reply_has_no_detail() {
        let parent = row("1700000000.000100", Vec::new());
        let replies = [row("1700000001.000100", Vec::new())];
        assert!(slack_activity_thread_detail_rows(
            parent,
            &replies,
            &timestamp("1700000009.000100")
        )
        .is_none());
    }
}
