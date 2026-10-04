use super::{
    authored_slack_client_message_ids, build_slack_appended_message_row_with_local_today,
    build_slack_message_chunks, build_slack_message_remote_images,
    prepare_slack_conversation_snapshot, slack_history_timestamp_key, slack_local_today,
    slack_message_timezone, validate_slack_refresh_message_order, Arc, Date,
    PreparedSlackConversationSnapshot, SlackAppendedMessageRowInput, SlackConversationSnapshot,
    SlackMessage,
};

pub(crate) fn prepare_slack_conversation_refresh(
    existing: SlackConversationSnapshot,
    refreshed: SlackConversationSnapshot,
) -> Result<Option<PreparedSlackConversationSnapshot>, String> {
    Ok(reconcile_slack_conversation_refresh(existing, refreshed)?
        .map(|refresh| prepare_slack_conversation_snapshot(refresh.snapshot)))
}

pub(crate) fn prepare_slack_conversation_refresh_reusing_rows(
    existing: SlackConversationSnapshot,
    existing_rows: Arc<[crate::ui::surface::SlackMessageRow]>,
    existing_rows_local_today: Option<Date>,
    refreshed: SlackConversationSnapshot,
) -> Result<Option<PreparedSlackConversationSnapshot>, String> {
    let Some(refresh) = reconcile_slack_conversation_refresh(existing, refreshed)? else {
        return Ok(None);
    };
    let Some(appended_message_start) = refresh.appended_message_start else {
        return Ok(Some(prepare_slack_conversation_snapshot(refresh.snapshot)));
    };
    if existing_rows.len() != appended_message_start {
        return Ok(Some(prepare_slack_conversation_snapshot(refresh.snapshot)));
    }
    let timezone = slack_message_timezone(refresh.snapshot.self_timezone_id.as_deref());
    let message_rows_local_today = slack_local_today(timezone);
    if existing_rows_local_today != Some(message_rows_local_today) {
        return Ok(Some(prepare_slack_conversation_snapshot(refresh.snapshot)));
    }
    Ok(Some(prepare_slack_appended_conversation_refresh(
        refresh.snapshot,
        existing_rows,
        appended_message_start,
        message_rows_local_today,
    )))
}

struct ReconciledSlackConversationRefresh {
    snapshot: SlackConversationSnapshot,
    appended_message_start: Option<usize>,
}

fn reconcile_slack_conversation_refresh(
    mut existing: SlackConversationSnapshot,
    mut refreshed: SlackConversationSnapshot,
) -> Result<Option<ReconciledSlackConversationRefresh>, String> {
    if existing.team_id != refreshed.team_id
        || existing.conversation_id != refreshed.conversation_id
    {
        return Err(
            "Slack conversation refresh targeted a different active conversation".to_string(),
        );
    }
    validate_slack_refresh_message_order(&existing.messages)?;
    validate_slack_refresh_message_order(&refreshed.messages)?;
    let preserved_message_count =
        preserved_slack_message_count(&existing.messages, &refreshed.messages)?;
    let existing_messages = std::mem::take(&mut existing.messages);
    let refreshed_messages = std::mem::take(&mut refreshed.messages);
    let existing_message_count = existing_messages.len();
    let row_context_unchanged = existing.self_timezone_id == refreshed.self_timezone_id;
    let messages_unchanged = preserved_message_count + refreshed_messages.len()
        == existing_message_count
        && existing_messages[preserved_message_count..] == refreshed_messages;
    let messages_only_appended = preserved_message_count + refreshed_messages.len()
        >= existing_message_count
        && refreshed_messages.len()
            >= existing_message_count.saturating_sub(preserved_message_count)
        && existing_messages[preserved_message_count..]
            == refreshed_messages[..existing_message_count.saturating_sub(preserved_message_count)];
    let appended_message_start =
        (row_context_unchanged && messages_only_appended).then_some(existing_message_count);
    let reconciled_history_cursor = existing
        .history_next_cursor
        .clone()
        .or_else(|| refreshed.history_next_cursor.clone());
    let history_cursor_changed = existing.history_next_cursor != reconciled_history_cursor;
    existing.history_next_cursor = reconciled_history_cursor.clone();
    refreshed.history_next_cursor = reconciled_history_cursor;
    refreshed.last_read = existing.last_read.clone();
    refreshed.last_read_boundary_loaded = existing.last_read_boundary_loaded;
    refreshed.peer_notifications_paused = existing.peer_notifications_paused;
    refreshed.dm_peer_local_time_context = existing.dm_peer_local_time_context.clone();
    refreshed.composer_draft_text = existing.composer_draft_text.clone();
    if messages_unchanged && !history_cursor_changed && existing == refreshed {
        return Ok(None);
    }
    let mut reconciled_messages =
        Vec::with_capacity(preserved_message_count + refreshed_messages.len());
    reconciled_messages.extend(existing_messages.into_iter().take(preserved_message_count));
    reconciled_messages.extend(refreshed_messages);
    refreshed.messages = reconciled_messages;
    Ok(Some(ReconciledSlackConversationRefresh {
        snapshot: refreshed,
        appended_message_start,
    }))
}

fn preserved_slack_message_count(
    existing_messages: &[SlackMessage],
    refreshed_messages: &[SlackMessage],
) -> Result<usize, String> {
    let Some(oldest_refreshed_timestamp) = refreshed_messages
        .first()
        .map(|message| slack_history_timestamp_key(&message.id))
        .transpose()?
    else {
        return Ok(0);
    };
    let mut preserved_message_count = 0;
    for message in existing_messages {
        if slack_history_timestamp_key(&message.id)? >= oldest_refreshed_timestamp {
            break;
        }
        preserved_message_count += 1;
    }
    Ok(preserved_message_count)
}

fn prepare_slack_appended_conversation_refresh(
    snapshot: SlackConversationSnapshot,
    existing_rows: Arc<[crate::ui::surface::SlackMessageRow]>,
    appended_message_start: usize,
    message_rows_local_today: Date,
) -> PreparedSlackConversationSnapshot {
    let timezone = slack_message_timezone(snapshot.self_timezone_id.as_deref());
    let mut message_rows = Vec::with_capacity(snapshot.messages.len());
    message_rows.extend(existing_rows.iter().cloned());
    let mut unread_boundary_pending = snapshot.last_read_boundary_loaded
        && snapshot.last_read.is_some()
        && !existing_rows.iter().any(|row| row.unread_boundary_before);
    let last_read_key = snapshot
        .last_read
        .as_ref()
        .map(crate::ui::SlackLastReadTimestamp::sort_key);
    for message_index in appended_message_start..snapshot.messages.len() {
        let message = &snapshot.messages[message_index];
        let mut row =
            build_slack_appended_message_row_with_local_today(SlackAppendedMessageRowInput {
                previous_message: message_index
                    .checked_sub(1)
                    .and_then(|index| snapshot.messages.get(index)),
                message,
                timezone,
                local_today: message_rows_local_today,
                team_id: &snapshot.team_id,
                conversation_id: &snapshot.conversation_id,
            });
        if unread_boundary_pending
            && last_read_key.is_some_and(|last_read| {
                slack_history_timestamp_key(&message.id)
                    .expect("validated Slack refresh timestamp must remain valid")
                    > last_read
            })
        {
            row.unread_boundary_before = true;
            row.compact = false;
            unread_boundary_pending = false;
        }
        message_rows.push(row);
    }
    let message_rows = Arc::from(message_rows);
    let message_chunks = build_slack_message_chunks(&message_rows);
    let appended_messages = &snapshot.messages[appended_message_start..];
    let remote_images = build_slack_message_remote_images(appended_messages);
    let authored_client_message_ids = authored_slack_client_message_ids(appended_messages.iter());
    PreparedSlackConversationSnapshot {
        snapshot,
        message_rows,
        message_rows_local_today,
        message_chunks,
        remote_images,
        authored_client_message_ids,
    }
}
