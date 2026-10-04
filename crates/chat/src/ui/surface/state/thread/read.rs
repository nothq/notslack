use super::{
    Context, Duration, SlackMessageTimestamp, SlackRailView, SlackThreadListRow,
    SlackThreadRequest, SurfaceState, SLACK_THREAD_READ_DEBOUNCE,
};
use crate::ui::surface::{
    prepare_slack_all_threads_snapshot, SlackMainRoute, SlackThreadReadIdentity,
    SlackThreadReadReadiness, SlackThreadReadRequest, SlackThreadReadState,
};
use crate::ui::{SlackMainTab, SlackThreadReadMetadata, SlackThreadReadTarget};

mod visibility;

impl SurfaceState {
    fn queue_slack_thread_read_at(
        &mut self,
        message_timestamp: SlackMessageTimestamp,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.slack_thread_read_state.as_ref() else {
            return;
        };
        if state.readiness != SlackThreadReadReadiness::Ready
            || state.unread_count == 0
            || self.slack_pending_message_mark_unread.is_some()
            || !self.slack_thread_read_identity_is_current(state.generation, &state.target)
            || state
                .floor
                .as_ref()
                .is_some_and(|floor| floor.sort_key() >= message_timestamp.sort_key())
        {
            return;
        }
        let generation = state.generation;
        let target = state.target.clone();
        if self
            .slack_thread_read_request
            .as_ref()
            .is_some_and(|request| {
                slack_thread_read_at_or_after(request, generation, &target, &message_timestamp)
            })
        {
            return;
        }
        if self
            .slack_thread_read_pending
            .as_ref()
            .is_some_and(|request| {
                slack_thread_read_at_or_after(request, generation, &target, &message_timestamp)
            })
        {
            self.schedule_slack_thread_read(SLACK_THREAD_READ_DEBOUNCE, cx);
            return;
        }
        self.slack_thread_read_pending = Some(SlackThreadReadRequest {
            generation,
            target,
            message_timestamp,
        });
        self.schedule_slack_thread_read(SLACK_THREAD_READ_DEBOUNCE, cx);
    }

    fn schedule_slack_thread_read(&mut self, delay: Duration, cx: &mut Context<Self>) {
        if self.slack_thread_read_timer.is_some() || self.slack_thread_read_request.is_some() {
            return;
        }
        let Some(request) = self.slack_thread_read_pending.clone() else {
            return;
        };
        self.slack_thread_read_timer = Some(request.clone());
        self.spawn_timer_task(request.clone(), delay, cx, |this, request, cx| {
            if this.slack_thread_read_timer.as_ref() != Some(&request) {
                return;
            }
            this.slack_thread_read_timer = None;
            this.start_slack_thread_read(cx);
        });
    }

    fn start_slack_thread_read(&mut self, cx: &mut Context<Self>) {
        if self.slack_thread_read_request.is_some() {
            return;
        }
        let Some(request) = self.slack_thread_read_pending.take() else {
            return;
        };
        if !self.slack_thread_read_request_is_allowed(&request) {
            if self.slack_thread_read_request_matches_state(&request) {
                self.slack_thread_read_pending = Some(request.clone());
                if let Some(state) = self.slack_thread_read_state.as_mut() {
                    state.readiness = SlackThreadReadReadiness::AwaitingVisibility {
                        check_scheduled: false,
                    };
                }
                cx.notify();
            }
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let target = SlackThreadReadTarget::new(
            request.target.conversation_id.clone(),
            request.target.thread_timestamp.clone(),
            request.message_timestamp.clone(),
        )
        .expect("validated Slack thread reply must produce a read target");
        self.slack_thread_read_request = Some(request.clone());
        let completion_request = request.clone();
        self.spawn_background_task(
            (workspace_api, target),
            cx,
            |(workspace_api, target)| workspace_api.mark_slack_thread_read(&target),
            move |this, result, cx| {
                this.finish_slack_thread_read(&completion_request, result, cx);
            },
        );
    }

    fn finish_slack_thread_read(
        &mut self,
        request: &SlackThreadReadRequest,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_thread_read_request.as_ref() != Some(request) {
            return;
        }
        self.slack_thread_read_request = None;
        let request_is_current =
            self.slack_thread_read_identity_is_current(request.generation, &request.target);
        match result {
            Ok(()) => {
                self.apply_slack_all_threads_read_success(request, cx);
                self.queue_slack_realtime_sidebar_refresh_after_read(cx);
                if self.slack_thread_read_request_matches_state(request) {
                    self.slack_thread_read_failures = 0;
                    self.advance_slack_thread_read_floor(request);
                }
            }
            Err(error) if request_is_current => {
                self.slack_thread_read_failures = self.slack_thread_read_failures.saturating_add(1);
                eprintln!(
                    "Slack thread read cursor update failed for {}/{}: {error}",
                    request.target.conversation_id, request.target.thread_timestamp
                );
                if self.slack_thread_read_request_is_allowed(request) {
                    self.requeue_latest_slack_thread_read(request);
                    self.schedule_slack_thread_read(
                        slack_thread_read_retry_delay(self.slack_thread_read_failures),
                        cx,
                    );
                }
            }
            Err(_) if self.slack_thread_read_request_matches_state(request) => {
                self.requeue_latest_slack_thread_read(request);
                self.defer_slack_thread_read_until_visible();
            }
            Err(_) => {}
        }
        self.queue_current_slack_thread_read(cx);
    }

    fn requeue_latest_slack_thread_read(&mut self, request: &SlackThreadReadRequest) {
        let retry = self
            .slack_thread_read_pending
            .take()
            .filter(|pending| {
                pending.generation == request.generation
                    && pending.target == request.target
                    && pending.message_timestamp.sort_key() > request.message_timestamp.sort_key()
            })
            .unwrap_or_else(|| request.clone());
        self.slack_thread_read_pending = Some(retry);
    }

    fn queue_current_slack_thread_read(&mut self, cx: &mut Context<Self>) {
        if let Some(message_timestamp) = self.newest_visible_slack_thread_reply_timestamp() {
            self.queue_slack_thread_read_at(message_timestamp, cx);
        }
    }

    fn slack_thread_read_request_is_allowed(&self, request: &SlackThreadReadRequest) -> bool {
        self.slack_pending_message_mark_unread.is_none()
            && self.slack_thread_read_state.as_ref().is_some_and(|state| {
                state.readiness == SlackThreadReadReadiness::Ready
                    && self.slack_thread_read_request_matches_state(request)
            })
            && self.slack_thread_read_identity_is_current(request.generation, &request.target)
    }

    fn slack_thread_read_request_matches_state(&self, request: &SlackThreadReadRequest) -> bool {
        self.slack_thread_read_state.as_ref().is_some_and(|state| {
            state.unread_count > 0
                && state.generation == request.generation
                && state.target == request.target
                && state
                    .floor
                    .as_ref()
                    .is_none_or(|floor| floor.sort_key() < request.message_timestamp.sort_key())
        })
    }

    fn defer_slack_thread_read_until_visible(&mut self) {
        if let Some(state) = self.slack_thread_read_state.as_mut() {
            state.readiness = SlackThreadReadReadiness::AwaitingVisibility {
                check_scheduled: false,
            };
        }
    }

    fn slack_thread_read_identity_is_current(
        &self,
        generation: u64,
        target: &SlackThreadReadIdentity,
    ) -> bool {
        if !self.active
            || !self.slack_thread_read_window_active
            || !self.slack_workspace_api_capabilities.mark_thread_read
            || self.slack_search_open
            || self.slack_search_results_open
            || self.slack_profile_panel.is_some()
            || self.slack_expanded_attachment.is_some()
            || self.slack_message_forward_modal.is_some()
            || self.slack_message_delete_modal.is_some()
            || self.slack_video_clip_modal.is_some()
            || self.slack_schedule_overlay.is_some()
            || self.slack_date_jump_overlay.is_some()
            || self.slack_composer_link_dialog.is_some()
            || self.slack_reaction_picker.is_some()
            || self.slack_members_panel_open
        {
            return false;
        }
        let Some(workspace) = self.slack_workspace() else {
            return false;
        };
        if workspace.team_id != target.team_id {
            return false;
        }
        self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.generation == generation
                && panel.conversation_id == target.conversation_id
                && panel.parent_message_id == target.thread_timestamp.as_str()
                && self.slack_thread_panel_origin_is_current(panel)
                && match &panel.origin {
                    super::SlackThreadPanelOrigin::Conversation => {
                        matches!(
                            self.slack_active_rail_view,
                            SlackRailView::Home | SlackRailView::Dms
                        ) && self.slack_main_route == SlackMainRoute::Conversation
                            && self.slack_active_tab == SlackMainTab::Messages
                    }
                    super::SlackThreadPanelOrigin::Later { .. } => {
                        self.slack_active_rail_view == SlackRailView::Later
                    }
                }
        })
    }

    fn advance_slack_thread_read_floor(&mut self, request: &SlackThreadReadRequest) {
        let crossed_replies = self.slack_thread_panel.as_ref().map_or(1, |panel| {
            let floor = self
                .slack_thread_read_state
                .as_ref()
                .and_then(|state| state.floor.as_ref());
            panel
                .reply_rows
                .iter()
                .filter_map(|reply| SlackMessageTimestamp::parse(reply.id.as_ref()).ok())
                .filter(|timestamp| {
                    floor.is_none_or(|floor| floor.sort_key() < timestamp.sort_key())
                        && timestamp.sort_key() <= request.message_timestamp.sort_key()
                })
                .count()
                .max(1)
        });
        let Some(state) = self.slack_thread_read_state.as_mut() else {
            return;
        };
        if state.generation != request.generation || state.target != request.target {
            return;
        }
        state.floor = Some(request.message_timestamp.clone());
        state.unread_count = state
            .unread_count
            .saturating_sub(u32::try_from(crossed_replies).unwrap_or(u32::MAX));
    }

    fn apply_slack_all_threads_read_success(
        &mut self,
        request: &SlackThreadReadRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self.slack_all_threads_snapshot.as_ref() else {
            return;
        };
        if snapshot.team_id != request.target.team_id {
            return;
        }
        let thread_index = snapshot.threads.iter().position(|thread| {
            thread.conversation_id == request.target.conversation_id
                && thread.thread_timestamp == request.target.thread_timestamp.as_str()
        });
        let Some(thread_index) = thread_index else {
            self.invalidate_slack_all_threads_read_cache(cx);
            return;
        };
        let snapshot = self
            .slack_all_threads_snapshot
            .as_mut()
            .expect("validated Slack All Threads snapshot must remain available");
        let thread = &mut snapshot.threads[thread_index];
        let previous_count = thread.unread_reply_timestamps.len();
        thread
            .unread_reply_timestamps
            .retain(|timestamp| timestamp.sort_key() > request.message_timestamp.sort_key());
        let removed = previous_count - thread.unread_reply_timestamps.len();
        if removed == 0 {
            self.invalidate_slack_all_threads_read_cache(cx);
            return;
        }
        if let Some(total) = snapshot.total_unread_replies.as_mut() {
            *total = total.saturating_sub(u32::try_from(removed).unwrap_or(u32::MAX));
        }
        let snapshot = self
            .slack_all_threads_snapshot
            .clone()
            .expect("changed Slack All Threads read state requires a snapshot");
        let mut prepared = prepare_slack_all_threads_snapshot(snapshot);
        self.slack_presence_authority
            .overlay_prepared_all_threads(&mut prepared);
        self.slack_all_threads_snapshot = Some(prepared.snapshot);
        self.slack_all_threads_rows = prepared.rows;
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_all_threads(data);
        }
        self.slack_all_threads_list_state.remeasure();
        cx.notify();
    }

    fn invalidate_slack_all_threads_read_cache(&mut self, cx: &mut Context<Self>) {
        self.slack_all_threads_snapshot = None;
        self.slack_all_threads_rows = Default::default();
        self.slack_all_threads_list_state.reset(0);
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_all_threads(data);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn reset_slack_thread_read_context(&mut self) {
        self.slack_thread_read_pending = None;
        self.slack_thread_read_timer = None;
        self.slack_thread_read_state = None;
        self.slack_thread_read_failures = 0;
    }
}

fn slack_thread_read_at_or_after(
    request: &SlackThreadReadRequest,
    generation: u64,
    target: &SlackThreadReadIdentity,
    timestamp: &SlackMessageTimestamp,
) -> bool {
    request.generation == generation
        && request.target == *target
        && request.message_timestamp.sort_key() >= timestamp.sort_key()
}

fn slack_thread_read_retry_delay(failures: u8) -> Duration {
    match failures {
        0 => SLACK_THREAD_READ_DEBOUNCE,
        1 => Duration::from_secs(15),
        2 => Duration::from_secs(30),
        3 => Duration::from_secs(60),
        _ => Duration::from_secs(120),
    }
}
