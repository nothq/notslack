use std::sync::Arc;

use crate::ui::surface::{
    build_slack_message_chunks, slack_history_timestamp_key, PreparedSlackMessageSendReceipt,
    SlackSendRequest, SurfaceState,
};
use crate::ui::{Context, SlackMessage};

use super::super::slack_message_insertion_index;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_prepared_slack_activity_message_send_receipt(
        &mut self,
        request: &SlackSendRequest,
        prepared: PreparedSlackMessageSendReceipt,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let crate::ui::surface::SlackActivityDetailState::Loaded { target, rows, .. } =
            &self.slack_activity_detail
        else {
            return Ok(());
        };
        let Some(composer) = target.composer() else {
            return Ok(());
        };
        if composer.target.team_id != request.team_id
            || composer.target.self_user_id != request.self_user_id
            || composer.target.conversation_id != request.conversation_id
        {
            return Ok(());
        }
        let force_scroll_to_end = composer.source == request.draft_source;
        if rows.iter().any(|row| row.id == prepared.receipt.timestamp) {
            return Ok(());
        }
        let was_following_end = self.slack_activity_detail_list_state.is_following_tail();
        let message_row = prepared.message_row;
        let mut next_rows = rows.to_vec();
        next_rows.push(message_row.clone());
        let authoritative_count = next_rows.len();
        let next_rows: Arc<[crate::ui::surface::SlackMessageRow]> = next_rows.into();
        let local_count = self.slack_activity_local_delivery_rows.len();
        if let crate::ui::surface::SlackActivityDetailState::Loaded { rows, .. } =
            &mut self.slack_activity_detail
        {
            *rows = next_rows;
        }
        self.slack_activity_detail_list_state
            .reset(authoritative_count.saturating_add(local_count));
        if force_scroll_to_end || was_following_end {
            self.slack_activity_detail_list_state.scroll_to_end();
        }
        self.slack_remote_images.extend(prepared.remote_images);
        self.prefetch_slack_message_preview_images(std::slice::from_ref(&message_row), cx);
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
        self.slack_error = None;
        Ok(())
    }

    pub(in crate::ui::surface::state) fn apply_prepared_slack_message_send_receipt(
        &mut self,
        _request: &SlackSendRequest,
        prepared: PreparedSlackMessageSendReceipt,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let PreparedSlackMessageSendReceipt {
            receipt,
            previous_message_id,
            message_row,
            message_row_local_today,
            remote_images,
        } = prepared;
        if self.slack_send_receipt_already_applied(&receipt)? {
            return Ok(());
        }

        let receipt_timestamp_key = slack_history_timestamp_key(&receipt.timestamp)
            .expect("Slack send receipt timestamp was validated above");
        let current_previous_message_id = self
            .slack_conversation_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.messages.last())
            .map(|message| message.id.clone());
        let append_is_current = self.slack_message_rows_local_today
            == Some(message_row_local_today)
            && slack_send_append_is_current(
                current_previous_message_id.as_deref(),
                previous_message_id.as_deref(),
                receipt_timestamp_key,
            )?;
        let message = receipt.message;
        let old_row_count = self.slack_message_rows.len();
        let was_following_end = self.slack_message_list_state.is_following_tail()
            || self.slack_message_list_state.logical_scroll_top().item_ix >= old_row_count;
        let inserted_index = if append_is_current {
            self.append_slack_sent_message(message, message_row.clone(), old_row_count)
        } else {
            self.insert_slack_sent_message(message, receipt_timestamp_key)?
        };
        self.slack_remote_images.extend(remote_images);
        self.finish_slack_sent_message_insertion(
            inserted_index,
            append_is_current,
            was_following_end,
        );
        self.prefetch_slack_message_preview_images(std::slice::from_ref(&message_row), cx);
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
        self.slack_error = None;
        Ok(())
    }

    pub(super) fn validate_slack_send_receipt(
        &self,
        request: &SlackSendRequest,
        receipt: &crate::ui::SlackMessageSendReceipt,
    ) -> Result<(), String> {
        if receipt.team_id != request.team_id
            || receipt.conversation_id != request.conversation_id
            || receipt.timestamp != receipt.message.id
            || receipt.self_user_id != request.self_user_id
            || receipt.message.user_id.as_deref() != Some(receipt.self_user_id.as_str())
            || receipt.message.client_message_id.as_ref() != Some(&request.client_message_id)
            || receipt.message.author.trim().is_empty()
            || (receipt.message.body.is_empty() && receipt.message.attachments.is_empty())
        {
            return Err(
                "Slack send response did not match the requested workspace, conversation, and authenticated author."
                    .to_string(),
            );
        }
        slack_history_timestamp_key(&receipt.timestamp).map_err(|error| {
            format!("Slack send response returned an invalid message timestamp: {error}")
        })?;
        if self
            .slack_workspace()
            .filter(|workspace| workspace.team_id == request.team_id)
            .and_then(|workspace| workspace.self_user_id.as_deref())
            .is_some_and(|self_user_id| self_user_id != receipt.self_user_id)
        {
            return Err(
                "Slack send response author did not match the active workspace identity."
                    .to_string(),
            );
        }
        Ok(())
    }

    fn slack_send_receipt_already_applied(
        &self,
        receipt: &crate::ui::SlackMessageSendReceipt,
    ) -> Result<bool, String> {
        let existing = self
            .slack_conversation_snapshot
            .as_ref()
            .ok_or_else(|| "Slack send response requires an active conversation".to_string())?
            .messages
            .iter()
            .any(|message| message.id == receipt.timestamp);
        if !existing {
            return Ok(false);
        }
        let workspace_contains_message = self
            .slack_workspace()
            .expect("active Slack conversation requires a workspace")
            .messages
            .iter()
            .any(|message| message.id == receipt.timestamp);
        if !workspace_contains_message {
            return Err(
                "Slack workspace and conversation snapshots diverged while applying a sent message."
                    .to_string(),
            );
        }
        Ok(true)
    }

    fn append_slack_sent_message(
        &mut self,
        message: SlackMessage,
        message_row: crate::ui::surface::SlackMessageRow,
        inserted_index: usize,
    ) -> usize {
        self.slack_conversation_snapshot
            .as_mut()
            .expect("active Slack conversation was validated above")
            .messages
            .push(message.clone());
        self.slack_workspace_mut()
            .expect("active Slack conversation requires a workspace")
            .messages
            .push(message);
        let mut message_rows = self.slack_message_rows.to_vec();
        message_rows.push(message_row);
        self.slack_message_rows = Arc::from(message_rows);
        inserted_index
    }

    fn insert_slack_sent_message(
        &mut self,
        message: SlackMessage,
        timestamp_key: (u64, u32),
    ) -> Result<usize, String> {
        let inserted_index = slack_message_insertion_index(
            &self
                .slack_conversation_snapshot
                .as_ref()
                .expect("active Slack conversation was validated above")
                .messages,
            timestamp_key,
        )?;
        self.slack_conversation_snapshot
            .as_mut()
            .expect("active Slack conversation was validated above")
            .messages
            .insert(inserted_index, message.clone());
        self.slack_workspace_mut()
            .expect("active Slack conversation requires a workspace")
            .messages
            .insert(inserted_index, message);
        self.refresh_slack_message_rows();
        Ok(inserted_index)
    }

    fn finish_slack_sent_message_insertion(
        &mut self,
        inserted_index: usize,
        appended: bool,
        was_following_end: bool,
    ) {
        self.advance_slack_conversation_revision();
        self.slack_message_chunks = build_slack_message_chunks(&self.slack_message_rows);
        self.slack_message_list_state
            .splice(inserted_index..inserted_index, 1);
        if !appended {
            self.slack_message_list_state.remeasure_items(
                inserted_index..(inserted_index + 2).min(self.slack_message_rows.len()),
            );
        }
        if was_following_end {
            self.slack_message_list_state.scroll_to_end();
        }
    }
}

fn slack_send_append_is_current(
    current_previous_message_id: Option<&str>,
    request_previous_message_id: Option<&str>,
    receipt_timestamp_key: (u64, u32),
) -> Result<bool, String> {
    Ok(current_previous_message_id == request_previous_message_id
        && current_previous_message_id
            .map(slack_history_timestamp_key)
            .transpose()?
            .is_none_or(|previous_timestamp_key| previous_timestamp_key < receipt_timestamp_key))
}
