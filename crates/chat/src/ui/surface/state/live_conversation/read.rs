mod application;

use super::{
    later_slack_message_timestamp, slack_conversation_read_at_or_after,
    slack_conversation_read_retry_delay, Context, Duration, SlackConversationLiveTarget,
    SlackConversationReadReadiness, SlackConversationReadRequest, SlackMessageTimestamp,
    SurfaceState, SLACK_CONVERSATION_READ_DEBOUNCE,
};
use crate::model::{SlackConversationReadReceipt, SlackLastReadTimestamp};

impl SurfaceState {
    pub(crate) fn queue_slack_conversation_read(&mut self, cx: &mut Context<Self>) {
        if !self.slack_conversation_read_is_ready() {
            return;
        }
        let Some(message_timestamp) = self.newest_visible_slack_conversation_timestamp() else {
            return;
        };
        self.queue_slack_conversation_read_at(message_timestamp, cx);
    }

    pub(crate) fn release_slack_conversation_unread_hold_for_reposition(&mut self) {
        let Some(state) = self
            .slack_conversation_read_state
            .as_mut()
            .filter(|state| state.readiness == SlackConversationReadReadiness::HeldUnread)
        else {
            return;
        };
        state.readiness = SlackConversationReadReadiness::Ready;
    }

    pub(crate) fn queue_slack_conversation_read_for_visible_range(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        is_following_tail: bool,
        cx: &mut Context<Self>,
    ) {
        if visible_start >= visible_end {
            return;
        }
        let index = if is_following_tail {
            self.slack_message_rows.len().checked_sub(1)
        } else {
            visible_end.checked_sub(1)
        };
        let Some(message_timestamp) = index
            .and_then(|index| self.slack_message_rows.get(index))
            .and_then(|row| SlackMessageTimestamp::parse(row.id.as_ref()).ok())
        else {
            return;
        };
        let Some((generation, revision, target)) = self
            .slack_conversation_read_state
            .as_ref()
            .map(|state| (state.generation, state.revision, state.target.clone()))
        else {
            return;
        };
        if !self.enable_slack_conversation_read_for_visible_state(generation, revision, &target) {
            return;
        }
        self.queue_slack_conversation_read_at(message_timestamp, cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_conversation_read_at(
        &mut self,
        message_timestamp: SlackMessageTimestamp,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_mark_unread.is_some()
            || !self.slack_workspace_api_capabilities.mark_conversation_read
        {
            return;
        }
        let Some((generation, target, floor, readiness)) =
            self.slack_conversation_read_state.as_ref().map(|state| {
                (
                    state.generation,
                    state.target.clone(),
                    state.floor.clone(),
                    state.readiness,
                )
            })
        else {
            return;
        };
        if readiness != SlackConversationReadReadiness::Ready
            || !self.slack_conversation_live_request_is_current(generation, &target)
        {
            return;
        }
        if self.apply_slack_conversation_read_floor(&target, floor, &message_timestamp, cx) {
            return;
        }
        if slack_conversation_read_option_at_or_after(
            self.slack_conversation_read_request.as_ref(),
            generation,
            &target,
            &message_timestamp,
        ) {
            return;
        }
        if slack_conversation_read_option_at_or_after(
            self.slack_conversation_read_pending.as_ref(),
            generation,
            &target,
            &message_timestamp,
        ) {
            self.schedule_slack_conversation_read(SLACK_CONVERSATION_READ_DEBOUNCE, cx);
            return;
        }
        self.slack_conversation_read_pending = Some(SlackConversationReadRequest {
            generation,
            target,
            message_timestamp,
        });
        self.schedule_slack_conversation_read(SLACK_CONVERSATION_READ_DEBOUNCE, cx);
    }

    fn apply_slack_conversation_read_floor(
        &mut self,
        target: &SlackConversationLiveTarget,
        floor: Option<SlackMessageTimestamp>,
        message_timestamp: &SlackMessageTimestamp,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(floor) = floor.filter(|floor| floor.sort_key() >= message_timestamp.sort_key())
        else {
            return false;
        };
        let receipt = SlackConversationReadReceipt {
            team_id: target.team_id.clone(),
            conversation_id: target.conversation_id.clone(),
            last_read: SlackLastReadTimestamp::parse(floor.as_str())
                .expect("typed Slack message timestamp must be a valid last_read timestamp"),
        };
        self.apply_slack_conversation_read_receipt(&receipt, cx);
        if self.clear_slack_conversation_read_overlay(target) {
            cx.notify();
        }
        self.queue_slack_realtime_sidebar_refresh_after_read(cx);
        true
    }

    pub(in crate::ui::surface::state) fn newest_visible_slack_conversation_timestamp(
        &self,
    ) -> Option<SlackMessageTimestamp> {
        let viewport = self.slack_message_list_state.viewport_bounds();
        if viewport.size.height <= gpui::px(0.0) {
            return None;
        }
        let mut index = self
            .slack_message_list_state
            .logical_scroll_top()
            .item_ix
            .min(self.slack_message_rows.len().checked_sub(1)?);
        let mut newest_visible = None;
        while let Some(bounds) = self.slack_message_list_state.bounds_for_item(index) {
            if bounds.top() >= viewport.bottom() {
                break;
            }
            if bounds.bottom() > viewport.top() {
                newest_visible = self
                    .slack_message_rows
                    .get(index)
                    .and_then(|message| SlackMessageTimestamp::parse(message.id.as_ref()).ok());
            }
            index += 1;
            if index >= self.slack_message_rows.len() {
                break;
            }
        }
        newest_visible
    }

    pub(in crate::ui::surface::state) fn schedule_slack_conversation_read(
        &mut self,
        delay: Duration,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_read_timer.is_some()
            || self.slack_conversation_read_request.is_some()
        {
            return;
        }
        let Some(request) = self.slack_conversation_read_pending.clone() else {
            return;
        };
        self.slack_conversation_read_timer = Some(request.clone());
        self.spawn_timer_task(request.clone(), delay, cx, |this, request, cx| {
            if this.slack_conversation_read_timer.as_ref() != Some(&request) {
                return;
            }
            this.slack_conversation_read_timer = None;
            if !this.slack_conversation_live_request_is_current(request.generation, &request.target)
            {
                this.ensure_slack_conversation_live_sync(cx);
                return;
            }
            this.start_slack_conversation_read(cx);
        });
    }

    pub(in crate::ui::surface::state) fn start_slack_conversation_read(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_read_request.is_some() {
            return;
        }
        let Some(request) = self.slack_conversation_read_pending.take() else {
            return;
        };
        if !self.slack_conversation_read_request_is_allowed(&request) {
            if self.clear_slack_conversation_read_overlay(&request.target) {
                cx.notify();
            }
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if self.clear_slack_conversation_read_overlay(&request.target) {
                cx.notify();
            }
            return;
        };
        self.slack_conversation_read_request = Some(request.clone());
        let completion_request = request.clone();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                workspace_api.mark_slack_conversation_read(
                    &request.target.conversation_id,
                    &request.message_timestamp,
                )
            },
            move |this, result, cx| {
                this.finish_slack_conversation_read(&completion_request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn finish_slack_conversation_read(
        &mut self,
        request: &SlackConversationReadRequest,
        result: Result<SlackConversationReadReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_read_request.as_ref() != Some(request) {
            return;
        }
        self.slack_conversation_read_request = None;
        let request_is_current =
            self.slack_conversation_live_request_is_current(request.generation, &request.target);
        let result =
            result.and_then(|receipt| validate_slack_conversation_read_receipt(request, receipt));
        match result {
            Ok(receipt) => {
                if request_is_current {
                    self.slack_conversation_read_failures = 0;
                    self.advance_slack_conversation_read_floor(request);
                }
                self.apply_slack_conversation_read_receipt(&receipt, cx);
                if !self.clear_slack_conversation_read_overlay(&request.target) {
                    self.advance_slack_sidebar_read_epoch();
                } else {
                    cx.notify();
                }
                self.queue_slack_realtime_sidebar_refresh_after_read(cx);
            }
            Err(error) => {
                if request_is_current {
                    self.slack_conversation_read_failures =
                        self.slack_conversation_read_failures.saturating_add(1);
                }
                eprintln!(
                    "Slack read cursor update failed for {}: {error}",
                    request.target.conversation_id
                );
                if self.clear_slack_conversation_read_overlay(&request.target) {
                    cx.notify();
                }
                self.queue_slack_realtime_sidebar_refresh_after_read(cx);
                if request_is_current && self.slack_conversation_read_request_is_allowed(request) {
                    self.slack_conversation_read_pending = Some(request.clone());
                    self.schedule_slack_conversation_read(
                        slack_conversation_read_retry_delay(self.slack_conversation_read_failures),
                        cx,
                    );
                }
            }
        }
        self.ensure_slack_conversation_live_sync(cx);
        self.queue_slack_conversation_read(cx);
    }

    pub(in crate::ui::surface::state) fn slack_conversation_read_is_ready(&self) -> bool {
        self.slack_conversation_read_state
            .as_ref()
            .is_some_and(|state| {
                state.readiness == SlackConversationReadReadiness::Ready
                    && self
                        .slack_conversation_live_request_is_current(state.generation, &state.target)
            })
    }

    pub(in crate::ui::surface::state) fn enable_slack_conversation_read_for_visible_state(
        &mut self,
        generation: u64,
        revision: u64,
        target: &SlackConversationLiveTarget,
    ) -> bool {
        if !self.slack_conversation_live_request_is_current(generation, target) {
            return false;
        }
        let Some(state) = self.slack_conversation_read_state.as_mut() else {
            return false;
        };
        if state.generation != generation || state.revision != revision || state.target != *target {
            return false;
        }
        state.readiness = SlackConversationReadReadiness::Ready;
        true
    }

    pub(in crate::ui::surface::state) fn slack_conversation_read_request_is_allowed(
        &self,
        request: &SlackConversationReadRequest,
    ) -> bool {
        let Some(state) = self.slack_conversation_read_state.as_ref() else {
            return false;
        };
        state.readiness == SlackConversationReadReadiness::Ready
            && state.generation == request.generation
            && state.target == request.target
            && self.slack_conversation_live_request_is_current(request.generation, &request.target)
            && state
                .floor
                .as_ref()
                .is_none_or(|floor| floor.sort_key() < request.message_timestamp.sort_key())
    }

    pub(in crate::ui::surface::state) fn advance_slack_conversation_read_floor(
        &mut self,
        request: &SlackConversationReadRequest,
    ) {
        let Some(state) = self.slack_conversation_read_state.as_mut() else {
            return;
        };
        if state.generation != request.generation || state.target != request.target {
            return;
        }
        state.floor = later_slack_message_timestamp(
            state.floor.take(),
            Some(request.message_timestamp.clone()),
        );
    }
}

fn validate_slack_conversation_read_receipt(
    request: &SlackConversationReadRequest,
    receipt: SlackConversationReadReceipt,
) -> Result<SlackConversationReadReceipt, String> {
    if receipt.team_id != request.target.team_id
        || receipt.conversation_id != request.target.conversation_id
        || receipt.last_read.as_str() != request.message_timestamp.as_str()
    {
        return Err("Slack read receipt targeted another conversation or cursor".to_string());
    }
    Ok(receipt)
}

fn slack_conversation_read_option_at_or_after(
    request: Option<&SlackConversationReadRequest>,
    generation: u64,
    target: &SlackConversationLiveTarget,
    message_timestamp: &SlackMessageTimestamp,
) -> bool {
    request.is_some_and(|request| {
        slack_conversation_read_at_or_after(request, generation, target, message_timestamp)
    })
}
