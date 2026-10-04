use std::{sync::Arc, time::Duration};

use crate::ui::{
    spawn_background_task_for_entity, spawn_timer_task_for_entity, SlackQuickMessageQuery,
    SlackQuickSearchSnapshot, SlackWorkspace,
};

use super::super::{
    prepare_slack_quick_search_snapshot, Context, PreparedSlackQuickSearchSnapshot,
    SlackQuickSearchTarget, SurfaceState, WorkspaceApi,
};

const SLACK_QUICK_SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);
const SLACK_QUICK_MESSAGE_CADENCE: Duration = Duration::from_millis(400);
const SLACK_QUICK_SEARCH_INITIAL_VISIBLE_ROWS: usize = 8;

type SlackQuickSearchWorkspace = Option<Arc<SlackWorkspace>>;
type SlackQuickSwitchTask = (
    Arc<dyn WorkspaceApi>,
    SlackQuickSearchWorkspace,
    SlackQuickSearchRequest,
);

struct SlackQuickMessagesTask {
    workspace_api: Arc<dyn WorkspaceApi>,
    workspace: SlackQuickSearchWorkspace,
    request: SlackQuickSearchRequest,
    message_query: SlackQuickMessageQuery,
}

#[derive(Clone)]
struct SlackQuickSearchRequest {
    generation: u64,
    query: String,
    message_query: Option<SlackQuickMessageQuery>,
    recent_channels: Arc<[String]>,
}

impl SurfaceState {
    pub(super) fn schedule_slack_quick_search(&mut self, cx: &mut Context<Self>) {
        if !self.slack_search_open
            || !self.slack_workspace_api_capabilities.search_quick_switch
            || self.slack_search_query.trim().is_empty()
        {
            self.slack_quick_message_last_started_at = None;
            return;
        }
        let message_query = self
            .slack_search_api_query(self.slack_search_query.trim())
            .as_deref()
            .and_then(SlackQuickMessageQuery::parse);
        let request = SlackQuickSearchRequest {
            generation: self.slack_search_generation,
            query: self.slack_search_query.trim().to_string(),
            message_query,
            recent_channels: self.slack_search_recent_channel_ids().into(),
        };
        if let Some(workspace_api) = self.active_slack_workspace_api() {
            self.schedule_slack_quick_messages(
                workspace_api,
                self.slack_workspace.clone(),
                &request,
                cx,
            );
        }
        spawn_timer_task_for_entity(
            request,
            SLACK_QUICK_SEARCH_DEBOUNCE,
            cx,
            |this, request, cx| {
                if this.slack_quick_search_request_is_current(&request) {
                    this.begin_slack_quick_search(request, cx);
                }
            },
        );
    }

    fn begin_slack_quick_search(
        &mut self,
        request: SlackQuickSearchRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_quick_search_error =
                Some("Slack quick search requires a connected workspace".into());
            self.slack_quick_search_loading = false;
            cx.notify();
            return;
        };
        let workspace = self.slack_workspace.clone();
        self.slack_quick_search_loading = true;
        self.slack_quick_search_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, workspace, request),
            cx,
            move |(workspace_api, workspace, request): SlackQuickSwitchTask| {
                let result = workspace_api
                    .search_slack_quick_switch(&request.query, &request.recent_channels)
                    .map(|snapshot| {
                        prepare_slack_quick_search_snapshot(snapshot, workspace.as_deref())
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_quick_search(request, result, cx);
            },
        );
    }

    fn schedule_slack_quick_messages(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        workspace: SlackQuickSearchWorkspace,
        request: &SlackQuickSearchRequest,
        cx: &mut Context<Self>,
    ) {
        self.slack_quick_search_message_rows = Arc::default();
        let Some(message_query) = request.message_query.clone() else {
            self.slack_quick_message_last_started_at = None;
            return;
        };
        let delay = self
            .slack_quick_message_last_started_at
            .map(|started_at| SLACK_QUICK_MESSAGE_CADENCE.saturating_sub(started_at.elapsed()))
            .unwrap_or_default();
        let request = request.clone();
        spawn_timer_task_for_entity(
            SlackQuickMessagesTask {
                workspace_api,
                workspace,
                request,
                message_query,
            },
            delay,
            cx,
            |this, task, cx| {
                if this.slack_quick_search_request_is_current(&task.request) {
                    this.begin_scheduled_quick_messages(task, cx);
                }
            },
        );
    }

    fn begin_scheduled_quick_messages(
        &mut self,
        task: SlackQuickMessagesTask,
        cx: &mut Context<Self>,
    ) {
        self.slack_quick_message_last_started_at = Some(std::time::Instant::now());
        spawn_background_task_for_entity(
            task,
            cx,
            move |task: SlackQuickMessagesTask| {
                let SlackQuickMessagesTask {
                    workspace_api,
                    workspace,
                    request,
                    message_query,
                } = task;
                let result = workspace_api
                    .search_slack_quick_messages(&message_query, &request.recent_channels)
                    .map(|messages| {
                        prepare_slack_quick_search_snapshot(
                            SlackQuickSearchSnapshot {
                                query: request.query.clone(),
                                channels: Vec::new(),
                                people: Vec::new(),
                                direct_messages: Vec::new(),
                                recent_messages: messages,
                            },
                            workspace.as_deref(),
                        )
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_quick_messages(request, result, cx);
            },
        );
    }

    fn finish_slack_quick_messages(
        &mut self,
        request: SlackQuickSearchRequest,
        result: Result<PreparedSlackQuickSearchSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_quick_search_request_is_current(&request) {
            return;
        }
        let Ok(prepared) = result else {
            return;
        };
        self.slack_quick_search_message_rows = prepared.message_rows;
        self.slack_search_selected_option = self
            .slack_search_selected_option
            .min(self.slack_search_option_count().saturating_sub(1));
        let urls = self
            .slack_quick_search_message_rows
            .iter()
            .filter_map(|row| row.avatar_image_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        self.notify_slack_search_overlay(cx);
    }

    fn finish_slack_quick_search(
        &mut self,
        request: SlackQuickSearchRequest,
        result: Result<PreparedSlackQuickSearchSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_quick_search_request_is_current(&request) {
            return;
        }
        self.slack_quick_search_loading = false;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_quick_search_error = Some(error.into());
                self.notify_slack_search_overlay(cx);
                return;
            }
        };
        debug_assert_eq!(prepared.snapshot.query, request.query);
        let action_count = self.slack_search_action_count();
        let old_stable_count = self.slack_quick_search_rows.len();
        let selected_message_index = self
            .slack_search_selected_option
            .checked_sub(action_count + old_stable_count)
            .filter(|index| *index < self.slack_quick_search_message_rows.len());
        self.slack_quick_search_rows = prepared.rows;
        self.slack_quick_search_options_scroll_handle = gpui::ScrollHandle::new();
        self.slack_quick_search_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_quick_search_prefetched_range = None;
        self.slack_quick_search_error = None;
        self.slack_search_selected_option = selected_message_index.map_or_else(
            || {
                self.slack_search_selected_option
                    .min(self.slack_search_option_count().saturating_sub(1))
            },
            |index| action_count + self.slack_quick_search_rows.len() + index,
        );
        self.queue_slack_quick_search_visible_images(
            0,
            self.slack_quick_search_rows
                .len()
                .min(SLACK_QUICK_SEARCH_INITIAL_VISIBLE_ROWS),
            cx,
        );
        self.notify_slack_search_overlay(cx);
    }

    pub(crate) fn queue_slack_quick_search_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_search_open {
            return;
        }
        let range = (
            visible_start.min(self.slack_quick_search_rows.len()),
            visible_end.min(self.slack_quick_search_rows.len()),
        );
        if self.slack_quick_search_prefetched_range == Some(range) {
            return;
        }
        self.slack_quick_search_prefetched_range = Some(range);
        let urls = self.slack_quick_search_rows[range.0..range.1]
            .iter()
            .filter_map(|row| row.avatar_image_url.as_ref())
            .chain(
                self.slack_quick_search_message_rows
                    .iter()
                    .filter_map(|row| row.avatar_image_url.as_ref()),
            )
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub(crate) fn activate_slack_quick_search_row(
        &mut self,
        row_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self
            .slack_quick_search_rows
            .get(row_index)
            .map(|row| row.target.clone())
        else {
            return;
        };
        match target {
            SlackQuickSearchTarget::Conversation(conversation_id) => {
                self.select_slack_conversation(conversation_id.as_ref(), cx);
            }
            SlackQuickSearchTarget::Profile(user_id) => {
                self.close_slack_search(cx);
                self.open_slack_profile(user_id.as_ref(), cx);
            }
        }
    }

    pub(crate) fn activate_slack_quick_search_message(
        &mut self,
        row_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self
            .slack_quick_search_message_rows
            .get(row_index)
            .map(|row| row.action_target.clone())
        else {
            return;
        };
        self.close_slack_search(cx);
        self.open_slack_message_action_target(&target, cx);
    }

    fn slack_quick_search_request_is_current(&self, request: &SlackQuickSearchRequest) -> bool {
        self.slack_search_open
            && self.slack_search_generation == request.generation
            && self.slack_search_query.trim() == request.query
    }
}
