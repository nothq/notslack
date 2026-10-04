use super::super::{
    merge_slack_thread_rows, normalize_slack_thread_parent_row, refresh_slack_thread_list_rows,
    slack_thread_reply_list_index, PreparedSlackThreadSnapshot, SlackMessageRow,
};
use crate::ui::surface::SlackThreadPanelState;

pub(super) struct SlackAppliedThreadFileShare {
    pub(super) reply_index: Option<usize>,
    pub(super) reply_row: Option<SlackMessageRow>,
    pub(super) expected_reply_count: u32,
}

pub(super) fn merge_sent_slack_thread_file_share(
    panel: &mut SlackThreadPanelState,
    prepared: PreparedSlackThreadSnapshot,
    show_composer: bool,
) -> SlackAppliedThreadFileShare {
    let PreparedSlackThreadSnapshot {
        snapshot,
        timezone,
        parent_row,
        reply_rows,
    } = prepared;
    let old_rows = panel.reply_rows.clone();
    let newest_reply_id = reply_rows
        .iter()
        .rev()
        .find(|reply| old_rows.iter().all(|existing| existing.id != reply.id))
        .map(|reply| reply.id.clone());
    let parent_reply_count = parent_row.as_ref().and_then(|row| row.reply_count);
    panel.loading = false;
    panel.error = None;
    panel.conversation_name = snapshot.conversation_name;
    panel.timezone = timezone;
    panel.next_cursor = snapshot.next_cursor;
    panel.pagination_initialized = true;
    if let Some(mut parent_row) = parent_row {
        normalize_slack_thread_parent_row(&mut parent_row);
        panel.parent_row = parent_row;
        panel.parent_hydrated = true;
    }
    if let Some(parent_reply_count) = parent_reply_count {
        panel.expected_reply_count = panel.expected_reply_count.max(parent_reply_count);
    }
    panel.reply_rows = merge_slack_thread_rows(&old_rows, &reply_rows, panel.timezone).into();
    panel.expected_reply_count = panel
        .expected_reply_count
        .max(u32::try_from(panel.reply_rows.len()).unwrap_or(u32::MAX));
    refresh_slack_thread_list_rows(panel, show_composer);
    panel.list_state.scroll_to_end();
    let reply = newest_reply_id.and_then(|reply_id| {
        panel
            .reply_rows
            .iter()
            .enumerate()
            .find(|(_, reply)| reply.id == reply_id)
            .map(|(reply_index, reply)| {
                let list_index = slack_thread_reply_list_index(panel, reply_index)
                    .expect("file-share Slack thread reply is missing from its list view");
                (list_index, reply.clone())
            })
    });
    SlackAppliedThreadFileShare {
        reply_index: reply.as_ref().map(|(index, _)| *index),
        reply_row: reply.map(|(_, row)| row),
        expected_reply_count: panel.expected_reply_count,
    }
}
