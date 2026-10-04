use std::sync::Arc;

use crate::ui::surface::SlackAuthoredClientMessageIds;

use super::{
    advance_slack_last_read, prepare_slack_conversation_refresh_reusing_rows,
    slack_conversation_refresh_delay, Context, Duration, ListOffset,
    PreparedSlackConversationSnapshot, SlackConversationLiveTarget,
    SlackConversationRefreshRequest, SlackConversationRefreshResult, SlackLastReadTimestamp,
    SurfaceState, SLACK_CONVERSATION_REFRESH_INTERVAL,
};

struct SlackConversationRefreshPosition {
    scroll_top: ListOffset,
    last_message_id: Option<String>,
    following_end: bool,
    anchor_message_id: Option<String>,
}

struct SlackConversationRefreshInstallOutcome {
    last_read: Option<SlackLastReadTimestamp>,
    last_read_boundary_loaded: bool,
    authored_client_message_ids: Arc<SlackAuthoredClientMessageIds>,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn schedule_slack_conversation_refresh(
        &mut self,
        generation: u64,
        delay: Duration,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_refresh_timer_generation.is_some()
            || self.slack_conversation_refresh_request.is_some()
            || self.slack_conversation_live_generation != generation
            || self.slack_conversation_live_target.is_none()
        {
            return;
        }
        self.slack_conversation_refresh_timer_serial = self
            .slack_conversation_refresh_timer_serial
            .checked_add(1)
            .expect("Slack conversation refresh timer serial overflowed");
        let timer_serial = self.slack_conversation_refresh_timer_serial;
        self.slack_conversation_refresh_timer_generation = Some(generation);
        self.spawn_timer_task(generation, delay, cx, move |this, generation, cx| {
            if this.slack_conversation_refresh_timer_generation != Some(generation)
                || this.slack_conversation_refresh_timer_serial != timer_serial
            {
                return;
            }
            this.slack_conversation_refresh_timer_generation = None;
            if this.slack_conversation_live_generation != generation
                || this.slack_conversation_live_target.is_none()
            {
                this.ensure_slack_conversation_live_sync(cx);
                return;
            }
            if this.slack_message_history_request.is_some()
                || !this.slack_pending_reactions.is_empty()
                || this.slack_pending_message_saved.is_some()
                || this.slack_pending_message_mark_unread.is_some()
            {
                this.schedule_slack_conversation_refresh(
                    generation,
                    SLACK_CONVERSATION_REFRESH_INTERVAL,
                    cx,
                );
                return;
            }
            this.start_slack_conversation_refresh(generation, cx);
        });
    }

    pub(in crate::ui::surface::state) fn start_slack_conversation_refresh(
        &mut self,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_refresh_request.is_some() {
            return;
        }
        let Some(target) = self.slack_conversation_live_target.clone() else {
            return;
        };
        let Some(snapshot) = self.slack_conversation_snapshot.clone() else {
            return;
        };
        let message_rows = self.slack_message_rows.clone();
        let message_rows_local_today = self.slack_message_rows_local_today;
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let request = SlackConversationRefreshRequest {
            generation,
            revision: self.slack_conversation_revision,
            target,
        };
        if self.slack_conversation_reconciliation_target.as_ref() == Some(&request.target) {
            self.slack_conversation_reconciliation_target = None;
        }
        self.slack_conversation_refresh_request = Some(request.clone());
        let completion_request = request.clone();
        self.spawn_background_task(
            (
                workspace_api,
                request,
                snapshot,
                message_rows,
                message_rows_local_today,
            ),
            cx,
            |(workspace_api, request, snapshot, message_rows, message_rows_local_today)| {
                workspace_api
                    .refresh_slack_conversation(&request.target.conversation_id)
                    .and_then(|refreshed| {
                        prepare_slack_conversation_refresh_reusing_rows(
                            snapshot,
                            message_rows,
                            message_rows_local_today,
                            refreshed,
                        )
                    })
            },
            move |this, result, cx| {
                this.finish_slack_conversation_refresh(&completion_request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface) fn finish_slack_conversation_refresh(
        &mut self,
        request: &SlackConversationRefreshRequest,
        result: SlackConversationRefreshResult,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_refresh_request.as_ref() != Some(request) {
            return;
        }
        self.slack_conversation_refresh_request = None;
        if !self.slack_conversation_live_request_is_current(request.generation, &request.target)
            || self.slack_conversation_revision != request.revision
        {
            self.ensure_slack_conversation_live_sync(cx);
            return;
        }

        match result {
            Ok(Some(prepared)) => {
                self.slack_conversation_refresh_failures = 0;
                self.apply_prepared_slack_conversation_refresh(&request.target, prepared, cx);
            }
            Ok(None) => {
                self.slack_conversation_refresh_failures = 0;
            }
            Err(error) => {
                self.slack_conversation_refresh_failures =
                    self.slack_conversation_refresh_failures.saturating_add(1);
                eprintln!(
                    "Slack active conversation refresh failed for {}: {error}",
                    request.target.conversation_id
                );
            }
        }

        self.schedule_queued_slack_conversation_reconciliation(cx);
        if self.slack_conversation_refresh_timer_generation.is_none() {
            let delay = slack_conversation_refresh_delay(self.slack_conversation_refresh_failures);
            self.schedule_slack_conversation_refresh(request.generation, delay, cx);
        }
        self.queue_slack_conversation_read(cx);
    }

    pub(in crate::ui::surface::state) fn apply_prepared_slack_conversation_refresh(
        &mut self,
        target: &SlackConversationLiveTarget,
        prepared: PreparedSlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_live_target.as_ref() != Some(target) {
            return;
        }
        let position = self.capture_slack_conversation_refresh_position();
        let SlackConversationRefreshInstallOutcome {
            last_read,
            last_read_boundary_loaded,
            authored_client_message_ids,
        } = self.install_prepared_slack_conversation_refresh(prepared);
        let appended_message_count = self.slack_conversation_refresh_appended_count(&position);
        if position.following_end {
            self.slack_new_message_count = 0;
        } else {
            self.slack_new_message_count = self
                .slack_new_message_count
                .saturating_add(appended_message_count)
                .min(self.slack_message_rows.len());
        }
        self.reconcile_slack_thread_parent();
        self.sync_slack_message_list_after_refresh(
            position.following_end,
            position.scroll_top,
            position.anchor_message_id.as_deref(),
        );
        self.reconcile_slack_observed_client_messages(
            &target.team_id,
            &target.conversation_id,
            &authored_client_message_ids,
            cx,
        );
        self.install_slack_conversation_read_after_live_positioning(
            target.clone(),
            last_read.as_ref(),
            last_read_boundary_loaded,
            cx,
        );
        let rows = self.slack_message_rows.clone();
        self.prefetch_slack_message_preview_images(&rows, cx);
        self.mark_slack_remote_image_queue_dirty();
        self.rebuild_slack_remote_image_queue();
        cx.notify();
    }

    fn capture_slack_conversation_refresh_position(&self) -> SlackConversationRefreshPosition {
        let scroll_top = self.slack_message_list_state.logical_scroll_top();
        let previous_row_count = self.slack_message_rows.len();
        let following_end = self.slack_message_list_state.is_following_tail()
            || scroll_top.item_ix >= previous_row_count;
        let anchor_message_id = (!following_end)
            .then(|| {
                self.slack_message_rows
                    .get(scroll_top.item_ix)
                    .map(|row| row.id.clone())
            })
            .flatten();
        SlackConversationRefreshPosition {
            scroll_top,
            last_message_id: self.slack_message_rows.last().map(|row| row.id.clone()),
            following_end,
            anchor_message_id,
        }
    }

    fn install_prepared_slack_conversation_refresh(
        &mut self,
        prepared: PreparedSlackConversationSnapshot,
    ) -> SlackConversationRefreshInstallOutcome {
        let PreparedSlackConversationSnapshot {
            mut snapshot,
            message_rows,
            message_rows_local_today,
            message_chunks,
            remote_images,
            authored_client_message_ids,
        } = prepared;
        let team_id = snapshot.team_id.clone();
        let conversation_id = snapshot.conversation_id.clone();
        let local_read = self
            .slack_conversation_read_state
            .as_ref()
            .filter(|state| {
                state.generation == self.slack_conversation_live_generation
                    && state.target.team_id == team_id
                    && state.target.conversation_id == conversation_id
            })
            .and_then(|state| state.floor.as_ref())
            .map(|floor| {
                SlackLastReadTimestamp::parse(floor.as_str())
                    .expect("typed Slack read floor must be a valid last_read timestamp")
            });
        let local_read_advanced_snapshot = local_read
            .as_ref()
            .is_some_and(|local_read| advance_slack_last_read(&mut snapshot.last_read, local_read));
        if local_read_advanced_snapshot {
            snapshot.last_read_boundary_loaded = true;
        }
        let last_read = snapshot.last_read.clone();
        let last_read_boundary_loaded = snapshot.last_read_boundary_loaded;
        self.pause_slack_conversation_read_for_live_positioning();
        self.advance_slack_conversation_revision();
        self.slack_remote_images.extend(remote_images);
        self.slack_message_rows = message_rows;
        self.slack_message_rows_local_today = Some(message_rows_local_today);
        self.slack_message_chunks = message_chunks;
        self.slack_conversation_snapshot = Some(snapshot.clone());
        let resolved_attachment_channel_labels = self
            .slack_workspace_mut()
            .expect("active Slack refresh requires a workspace")
            .apply_conversation_snapshot(snapshot);
        if resolved_attachment_channel_labels || local_read_advanced_snapshot {
            self.refresh_slack_message_rows();
        }
        SlackConversationRefreshInstallOutcome {
            last_read,
            last_read_boundary_loaded,
            authored_client_message_ids,
        }
    }

    fn slack_conversation_refresh_appended_count(
        &self,
        position: &SlackConversationRefreshPosition,
    ) -> usize {
        if position.following_end {
            return 0;
        }
        position
            .last_message_id
            .as_deref()
            .and_then(|message_id| {
                self.slack_message_rows
                    .iter()
                    .position(|row| row.id == message_id)
            })
            .map_or(0, |previous_last_index| {
                self.slack_message_rows
                    .len()
                    .saturating_sub(previous_last_index.saturating_add(1))
            })
    }

    pub(in crate::ui::surface::state) fn sync_slack_message_list_after_refresh(
        &self,
        was_following_end: bool,
        previous_scroll_top: ListOffset,
        anchor_message_id: Option<&str>,
    ) {
        let next_count = self.slack_message_display_row_count();
        if self.slack_message_list_state.item_count() != next_count {
            self.slack_message_list_state.reset(next_count);
        } else {
            self.slack_message_list_state.remeasure();
        }
        if self.slack_message_list_auto_position_active {
            self.restore_slack_message_list_auto_position();
            return;
        }
        if was_following_end {
            self.slack_message_list_state.scroll_to_end();
            return;
        }
        let anchor_index = anchor_message_id.and_then(|anchor_message_id| {
            self.slack_message_rows
                .iter()
                .position(|row| row.id == anchor_message_id)
        });
        let item_ix = anchor_index.unwrap_or_else(|| {
            previous_scroll_top
                .item_ix
                .min(next_count.saturating_sub(1))
        });
        if next_count > 0 {
            self.slack_message_list_state.scroll_to(ListOffset {
                item_ix,
                offset_in_item: previous_scroll_top.offset_in_item,
            });
        }
    }
}
