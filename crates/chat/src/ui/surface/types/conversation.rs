use super::{
    build_slack_appended_message_row_with_local_today,
    build_slack_conversation_message_rows_with_local_today, build_slack_conversation_remote_images,
    build_slack_message_chunks, build_slack_message_remote_images,
    build_slack_thread_page_reply_rows_in_timezone, build_slack_thread_parent_row_in_timezone,
    slack_local_today, slack_message_timezone, Arc, Date, HashSet,
    PreparedSlackConversationHistoryPage, PreparedSlackConversationSnapshot,
    PreparedSlackMessageSendReceipt, PreparedSlackThreadReplyReceipt, PreparedSlackThreadSnapshot,
    SlackAppendedMessageRowInput, SlackAuthoredClientMessageIds, SlackConversationHistoryPage,
    SlackConversationSnapshot, SlackMessage, SlackMessageSendReceipt, SlackThreadReplyReceipt,
    SlackThreadSnapshot,
};

mod refresh;

pub(crate) use refresh::{
    prepare_slack_conversation_refresh, prepare_slack_conversation_refresh_reusing_rows,
};

pub(crate) fn prepare_slack_conversation_snapshot(
    snapshot: SlackConversationSnapshot,
) -> PreparedSlackConversationSnapshot {
    let (message_rows, message_rows_local_today) =
        build_slack_conversation_message_rows_with_local_today(&snapshot);
    let message_chunks = build_slack_message_chunks(&message_rows);
    let remote_images = build_slack_conversation_remote_images(&snapshot);
    let authored_client_message_ids = authored_slack_client_message_ids(snapshot.messages.iter());
    PreparedSlackConversationSnapshot {
        snapshot,
        message_rows,
        message_rows_local_today,
        message_chunks,
        remote_images,
        authored_client_message_ids,
    }
}

fn authored_slack_client_message_ids<'a>(
    messages: impl Iterator<Item = &'a SlackMessage>,
) -> Arc<SlackAuthoredClientMessageIds> {
    Arc::new(
        messages
            .filter_map(|message| {
                Some((message.client_message_id.clone()?, message.user_id.clone()?))
            })
            .collect(),
    )
}

pub(crate) fn validate_slack_refresh_message_order(
    messages: &[crate::ui::SlackMessage],
) -> Result<(), String> {
    let mut previous_timestamp = None;
    for message in messages {
        let timestamp = slack_history_timestamp_key(&message.id)?;
        if previous_timestamp.is_some_and(|previous| previous >= timestamp) {
            return Err(
                "Slack conversation refresh messages must have unique ascending timestamps"
                    .to_string(),
            );
        }
        previous_timestamp = Some(timestamp);
    }
    Ok(())
}

pub(crate) fn prepare_slack_conversation_history_page(
    mut snapshot: SlackConversationSnapshot,
    page: SlackConversationHistoryPage,
) -> Result<PreparedSlackConversationHistoryPage, String> {
    if snapshot.team_id != page.team_id || snapshot.conversation_id != page.conversation_id {
        return Err(
            "Slack conversation history page targeted a different conversation".to_string(),
        );
    }
    let previous_message_count = snapshot.messages.len();
    let mut prepended_messages =
        prepare_slack_history_prepended_messages(&snapshot, page.messages)?;
    let prepended_row_count = prepended_messages.len();
    let remote_images = build_slack_message_remote_images(&prepended_messages);
    prepended_messages.append(&mut snapshot.messages);
    snapshot.messages = prepended_messages;
    snapshot.history_next_cursor = page.next_cursor;
    if !snapshot.last_read_boundary_loaded {
        snapshot.last_read_boundary_loaded = snapshot
            .last_read
            .as_ref()
            .zip(snapshot.messages.first())
            .map(|(last_read, oldest_message)| {
                slack_history_timestamp_key(&oldest_message.id)
                    .map(|oldest| oldest <= last_read.sort_key())
            })
            .transpose()?
            .unwrap_or(false);
    }
    let (message_rows, message_rows_local_today) =
        build_slack_conversation_message_rows_with_local_today(&snapshot);
    let message_chunks = build_slack_message_chunks(&message_rows);
    if message_rows.len() != previous_message_count + prepended_row_count {
        return Err("Slack conversation history merge produced an invalid row count".to_string());
    }
    Ok(PreparedSlackConversationHistoryPage {
        snapshot,
        message_rows,
        message_rows_local_today,
        message_chunks,
        remote_images,
        prepended_row_count,
    })
}

fn prepare_slack_history_prepended_messages(
    snapshot: &SlackConversationSnapshot,
    page_messages: Vec<crate::ui::SlackMessage>,
) -> Result<Vec<crate::ui::SlackMessage>, String> {
    let oldest_loaded_timestamp = snapshot
        .messages
        .first()
        .map(|message| slack_history_timestamp_key(&message.id))
        .transpose()?;
    let existing_timestamps = snapshot
        .messages
        .iter()
        .map(|message| slack_history_timestamp_key(&message.id))
        .collect::<Result<HashSet<_>, _>>()?;
    if existing_timestamps.len() != snapshot.messages.len() {
        return Err("loaded Slack conversation contains duplicate message timestamps".to_string());
    }
    let mut prepended_messages = Vec::with_capacity(page_messages.len());
    let mut prepended_timestamps = HashSet::with_capacity(page_messages.len());
    for message in page_messages {
        let timestamp = slack_history_timestamp_key(&message.id)?;
        if oldest_loaded_timestamp.is_some_and(|oldest| timestamp > oldest) {
            return Err(format!(
                "Slack conversation history page returned newer message {} before the loaded boundary",
                message.id
            ));
        }
        if !existing_timestamps.contains(&timestamp) && prepended_timestamps.insert(timestamp) {
            prepended_messages.push(message);
        }
    }
    prepended_messages.sort_by_key(|message| {
        slack_history_timestamp_key(&message.id)
            .expect("validated Slack history message timestamp must remain valid")
    });
    Ok(prepended_messages)
}

pub(crate) fn slack_history_timestamp_key(timestamp: &str) -> Result<(u64, u32), String> {
    let (seconds, fractional) = timestamp.split_once('.').ok_or_else(|| {
        format!("Slack history message timestamp {timestamp:?} is missing a fractional separator")
    })?;
    if fractional.is_empty() || fractional.len() > 9 {
        return Err(format!(
            "Slack history message timestamp {timestamp:?} has invalid fractional precision"
        ));
    }
    let fractional_digits = fractional.len();
    let seconds = seconds.parse::<u64>().map_err(|_| {
        format!("Slack history message timestamp {timestamp:?} has invalid seconds")
    })?;
    let fractional = fractional.parse::<u32>().map_err(|_| {
        format!("Slack history message timestamp {timestamp:?} has invalid fractional seconds")
    })?;
    let nanoseconds = fractional
        .checked_mul(
            10_u32.pow(
                u32::try_from(9 - fractional_digits)
                    .expect("validated Slack history fractional precision must fit u32"),
            ),
        )
        .ok_or_else(|| {
            format!("Slack history message timestamp {timestamp:?} fractional seconds overflowed")
        })?;
    Ok((seconds, nanoseconds))
}

pub(crate) fn prepare_slack_thread_snapshot(
    snapshot: SlackThreadSnapshot,
) -> PreparedSlackThreadSnapshot {
    let timezone = slack_message_timezone(snapshot.self_timezone_id.as_deref());
    let parent_row = snapshot.parent.as_ref().map(|message| {
        build_slack_thread_parent_row_in_timezone(
            message,
            timezone,
            &snapshot.team_id,
            &snapshot.conversation_id,
        )
    });
    let reply_rows = build_slack_thread_page_reply_rows_in_timezone(
        &snapshot.replies,
        timezone,
        &snapshot.team_id,
        &snapshot.conversation_id,
        &snapshot.thread_timestamp,
    );
    PreparedSlackThreadSnapshot {
        snapshot,
        timezone,
        parent_row,
        reply_rows,
    }
}

pub(crate) fn prepare_slack_thread_reply_receipt(
    receipt: SlackThreadReplyReceipt,
) -> PreparedSlackThreadReplyReceipt {
    let timezone = slack_message_timezone(receipt.self_timezone_id.as_deref());
    let reply_row = build_slack_thread_page_reply_rows_in_timezone(
        std::slice::from_ref(&receipt.reply),
        timezone,
        &receipt.team_id,
        &receipt.conversation_id,
        &receipt.thread_timestamp,
    )
    .first()
    .cloned()
    .expect("a Slack thread reply receipt contains one reply");
    PreparedSlackThreadReplyReceipt { receipt, reply_row }
}

pub(crate) fn prepare_slack_message_send_receipt(
    receipt: SlackMessageSendReceipt,
    previous_message: Option<SlackMessage>,
) -> PreparedSlackMessageSendReceipt {
    let previous_message_id = previous_message.as_ref().map(|message| message.id.clone());
    let timezone = slack_message_timezone(receipt.self_timezone_id.as_deref());
    let message_row_local_today = slack_local_today(timezone);
    let message_row =
        build_slack_appended_message_row_with_local_today(SlackAppendedMessageRowInput {
            previous_message: previous_message.as_ref(),
            message: &receipt.message,
            timezone,
            local_today: message_row_local_today,
            team_id: &receipt.team_id,
            conversation_id: &receipt.conversation_id,
        });
    let remote_images = build_slack_message_remote_images(std::slice::from_ref(&receipt.message));
    PreparedSlackMessageSendReceipt {
        receipt,
        previous_message_id,
        message_row,
        message_row_local_today,
        remote_images,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        prepare_slack_conversation_refresh_reusing_rows, prepare_slack_conversation_snapshot,
    };
    use crate::ui::test_support::slack_test_workspace_with_message_count;

    #[gpui::test]
    fn appended_refresh_reuses_rows_and_marks_the_first_unread_message() {
        let mut existing =
            slack_test_workspace_with_message_count("C_AICRAZE", "design", false, 2)
                .conversation_snapshot();
        existing.last_read = Some(
            crate::ui::SlackLastReadTimestamp::parse("1700000000.000002")
                .expect("test last-read timestamp must be valid"),
        );
        let existing_prepared = prepare_slack_conversation_snapshot(existing.clone());
        assert!(!existing_prepared
            .message_rows
            .iter()
            .any(|row| row.unread_boundary_before));

        let client_message_id = crate::ui::SlackMessageClientId::generate();
        let mut appended_message = existing
            .messages
            .last()
            .cloned()
            .expect("test conversation must contain a previous message");
        appended_message.id = "1700000000.000003".to_string();
        appended_message.client_message_id = Some(client_message_id.clone());
        appended_message.user_id = Some("U_SELF".to_string());
        appended_message.body = "Accepted after an ambiguous response".to_string();
        let mut refreshed = existing.clone();
        refreshed.messages.push(appended_message);

        let prepared = prepare_slack_conversation_refresh_reusing_rows(
            existing,
            existing_prepared.message_rows.clone(),
            Some(existing_prepared.message_rows_local_today),
            refreshed,
        )
        .expect("append-only refresh must be valid")
        .expect("append-only refresh must change the conversation");

        assert_eq!(
            &prepared.message_rows[..2],
            &existing_prepared.message_rows[..]
        );
        assert!(prepared.message_rows[2].unread_boundary_before);
        assert!(!prepared.message_rows[2].compact);
        assert!(prepared
            .authored_client_message_ids
            .contains(&(client_message_id, "U_SELF".to_string())));
    }

    #[gpui::test]
    fn appended_refresh_rebuilds_rows_after_local_date_rollover() {
        let mut existing =
            slack_test_workspace_with_message_count("C_AICRAZE", "design", false, 1)
                .conversation_snapshot();
        let existing_client_message_id = crate::ui::SlackMessageClientId::generate();
        existing.messages[0].client_message_id = Some(existing_client_message_id.clone());
        existing.messages[0].user_id = Some("U_SELF".to_string());
        let existing_prepared = prepare_slack_conversation_snapshot(existing.clone());

        let appended_client_message_id = crate::ui::SlackMessageClientId::generate();
        let mut appended_message = existing.messages[0].clone();
        appended_message.id = "1700000000.000002".to_string();
        appended_message.client_message_id = Some(appended_client_message_id);
        appended_message.body = "Message after midnight".to_string();
        let mut refreshed = existing.clone();
        refreshed.messages.push(appended_message);

        let stale_local_today = existing_prepared
            .message_rows_local_today
            .previous_day()
            .expect("test local date must have a previous day");
        let prepared = prepare_slack_conversation_refresh_reusing_rows(
            existing,
            existing_prepared.message_rows,
            Some(stale_local_today),
            refreshed,
        )
        .expect("append-only refresh must be valid")
        .expect("append-only refresh must change the conversation");

        assert_eq!(
            prepared.message_rows_local_today,
            existing_prepared.message_rows_local_today
        );
        assert!(prepared
            .authored_client_message_ids
            .contains(&(existing_client_message_id, "U_SELF".to_string())));
    }
}
