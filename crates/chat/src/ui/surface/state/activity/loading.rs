use std::{collections::HashSet, sync::Arc};

use super::{
    merge_and_prepare_slack_activity_snapshot, prepare_slack_conversation_snapshot,
    prepare_slack_thread_snapshot, slack_message_reaction_remote_image_urls,
    PreparedSlackActivityDetail, PreparedSlackActivitySnapshot, SlackActivityDetailRequest,
    SlackActivityPageRequest, SlackMessageRow, SlackMessageTimestamp, WorkspaceApi,
};

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
        Some(thread_timestamp) => workspace_api
            .load_slack_thread_containing_reply(
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

pub(super) fn slack_activity_thread_detail_rows(
    rows: &[SlackMessageRow],
    thread_timestamp: &SlackMessageTimestamp,
    reply_timestamp: &SlackMessageTimestamp,
) -> Option<Arc<[SlackMessageRow]>> {
    let mut parent = rows
        .iter()
        .find(|row| row.id == thread_timestamp.as_str())?
        .clone();
    let mut reply = parent
        .replies
        .iter()
        .find(|reply| reply.id == reply_timestamp.as_str())?
        .clone();
    parent.replies.clear();
    reply.replies.clear();
    Some(vec![parent, reply].into())
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
