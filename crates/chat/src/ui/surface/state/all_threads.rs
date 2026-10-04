mod composer_state;
mod reply;

use std::sync::Arc;

use gpui::AppContext;
use gpui_components::text_input::{TextInput, TextInputProps};

use super::thread_reply::SlackThreadReplySendPayload;
use super::{Context, SlackRailView, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    merge_and_prepare_slack_all_threads_snapshot, prepare_slack_all_threads_snapshot,
    PreparedSlackAllThreadsSnapshot, SlackAllThreadRow, SlackAllThreadsComposerState,
    SlackComposerDestination, SlackComposerDraftKey, SlackComposerFormatAction,
    SlackComposerTarget, SlackMainRoute, SlackPendingThreadReplySend, SlackReplyComposerTarget,
};
use crate::ui::{SlackAllThreadsCursor, SlackAllThreadsSnapshot, SlackMessageTimestamp};

const SLACK_ALL_THREADS_INITIAL_VISIBLE_ROWS: usize = 4;
const SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW: usize = 2;
const SLACK_ALL_THREADS_PAGINATION_THRESHOLD: usize = 2;

struct SlackAllThreadsPageRequest {
    generation: u64,
    team_id: String,
    cursor: Option<SlackAllThreadsCursor>,
    existing: Option<SlackAllThreadsSnapshot>,
}

struct SlackAllThreadsReplyRequest {
    generation: u64,
    team_id: String,
    thread_key: String,
    draft_key: SlackComposerDraftKey,
    draft_token: u64,
    draft_document_revision: u64,
    conversation_id: String,
    thread_timestamp: SlackMessageTimestamp,
    payload: Arc<SlackThreadReplySendPayload>,
    broadcast: bool,
}

impl SlackAllThreadsReplyRequest {
    fn pending_send(&self) -> SlackPendingThreadReplySend {
        SlackPendingThreadReplySend {
            generation: self.generation,
            draft_token: self.draft_token,
            draft_document_revision: self.draft_document_revision,
        }
    }
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn refresh_slack_all_threads_from_realtime_if_visible(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_realtime_all_threads_stale
            || self.slack_main_route != SlackMainRoute::AllThreads
            || self.slack_all_threads_loading
        {
            return;
        }
        self.slack_realtime_all_threads_stale = false;
        self.slack_all_threads_snapshot = None;
        self.slack_all_threads_error = None;
        self.begin_slack_all_threads_page(None, cx);
    }

    pub(crate) fn activate_slack_all_threads(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_all_threads {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            self.slack_all_threads_error =
                Some("Slack All Threads requires a loaded workspace identity.".to_string());
            cx.notify();
            return;
        };
        self.reset_slack_schedule_context(cx);
        if self.slack_all_threads_team_id.as_deref() != Some(team_id.as_str()) {
            self.reset_slack_all_threads_context();
            self.slack_all_threads_team_id = Some(team_id);
        }
        self.leave_slack_bookmark_folder();
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.leave_slack_directory(cx);
        self.leave_slack_new_message(cx);
        self.close_slack_search_results(cx);
        self.close_slack_search(cx);
        self.slack_main_route = SlackMainRoute::AllThreads;
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.slack_all_threads_list_state.remeasure();
        self.sync_slack_all_threads_composer_inputs(cx);
        self.queue_slack_all_threads_visible_images(0, SLACK_ALL_THREADS_INITIAL_VISIBLE_ROWS, cx);
        self.record_slack_all_threads_history();
        cx.notify();
        if self.slack_all_threads_snapshot.is_none() && !self.slack_all_threads_loading {
            self.begin_slack_all_threads_page(None, cx);
        }
    }

    pub(crate) fn leave_slack_all_threads(&mut self) {
        if self.slack_main_route != SlackMainRoute::AllThreads {
            return;
        }
        self.slack_all_threads_generation = next_slack_all_threads_generation(
            self.slack_all_threads_generation,
            "Slack All Threads request generation overflowed",
        );
        self.slack_all_threads_loading = false;
    }

    pub(crate) fn reset_slack_all_threads_context(&mut self) {
        self.slack_all_threads_generation = next_slack_all_threads_generation(
            self.slack_all_threads_generation,
            "Slack All Threads request generation overflowed",
        );
        self.slack_all_threads_team_id = None;
        self.slack_all_threads_snapshot = None;
        self.slack_all_threads_rows = Arc::default();
        self.slack_all_threads_list_state.reset(0);
        self.slack_all_threads_visible_range = (0, SLACK_ALL_THREADS_INITIAL_VISIBLE_ROWS);
        self.slack_all_threads_loading = false;
        self.slack_all_threads_error = None;
        self.slack_all_threads_reply_errors.clear();
        self.slack_all_threads_composers.clear();
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_all_threads(data);
        }
        if matches!(
            self.slack_composer_aux_target.as_ref(),
            Some(SlackComposerTarget::Reply(
                SlackReplyComposerTarget::AllThreads { .. }
            ))
        ) {
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
            self.slack_mention_picker_state = None;
        }
    }

    pub(crate) fn retry_slack_all_threads(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::AllThreads || self.slack_all_threads_loading {
            return;
        }
        let cursor = self
            .slack_all_threads_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        self.begin_slack_all_threads_page(cursor, cx);
    }

    pub(in crate::ui::surface::state) fn reconcile_slack_all_threads_after_reply(
        &mut self,
        reply_applied: bool,
        cx: &mut Context<Self>,
    ) {
        if !reply_applied {
            self.slack_all_threads_snapshot = None;
        }
        if !reply_applied || self.slack_all_threads_loading {
            self.slack_all_threads_generation = next_slack_all_threads_generation(
                self.slack_all_threads_generation,
                "Slack All Threads request generation overflowed",
            );
            self.slack_all_threads_loading = false;
            self.retry_slack_all_threads(cx);
        }
    }

    pub(crate) fn handle_slack_all_threads_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.slack_all_threads_visible_range = (visible_start, visible_end);
        self.sync_slack_all_threads_composer_inputs(cx);
        self.queue_slack_all_threads_visible_images(visible_start, visible_end, cx);
        if visible_end.saturating_add(SLACK_ALL_THREADS_PAGINATION_THRESHOLD) >= count {
            self.maybe_load_more_slack_all_threads(cx);
        }
    }

    fn begin_slack_all_threads_page(
        &mut self,
        cursor: Option<SlackAllThreadsCursor>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_all_threads_loading || self.slack_main_route != SlackMainRoute::AllThreads {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_all_threads_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        let Some(team_id) = self.slack_all_threads_team_id.clone() else {
            return;
        };
        self.slack_all_threads_generation = next_slack_all_threads_generation(
            self.slack_all_threads_generation,
            "Slack All Threads request generation overflowed",
        );
        let request = SlackAllThreadsPageRequest {
            generation: self.slack_all_threads_generation,
            team_id,
            cursor,
            existing: self.slack_all_threads_snapshot.clone(),
        };
        self.slack_all_threads_loading = true;
        self.slack_all_threads_error = None;
        cx.notify();
        self.spawn_background_task(
            request,
            cx,
            move |request| load_slack_all_threads_page(workspace_api, request),
            move |this, (request, result), cx| {
                this.finish_slack_all_threads_page(request, result, cx);
            },
        );
    }

    fn finish_slack_all_threads_page(
        &mut self,
        request: SlackAllThreadsPageRequest,
        result: Result<PreparedSlackAllThreadsSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_all_threads_generation != request.generation
            || self.slack_main_route != SlackMainRoute::AllThreads
            || self.slack_all_threads_team_id.as_deref() != Some(request.team_id.as_str())
        {
            return;
        }
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            self.reset_slack_all_threads_context();
            self.activate_slack_all_threads(cx);
            return;
        }
        self.slack_all_threads_loading = false;
        match result {
            Ok(mut prepared) => {
                self.slack_presence_authority
                    .overlay_prepared_all_threads(&mut prepared);
                self.slack_all_threads_snapshot = Some(prepared.snapshot);
                self.slack_all_threads_rows = prepared.rows;
                {
                    let (authority, data) = (&mut self.slack_presence_authority, &self.data);
                    authority.reindex_all_threads(data);
                }
                self.sync_slack_all_threads_composer_inputs(cx);
                self.slack_all_threads_error = None;
                self.slack_all_threads_list_state
                    .reset(self.slack_all_threads_rows.len());
                let (visible_start, visible_end) = self.slack_all_threads_visible_range;
                self.queue_slack_all_threads_visible_images(visible_start, visible_end, cx);
            }
            Err(message) => self.slack_all_threads_error = Some(message),
        }
        cx.notify();
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    fn maybe_load_more_slack_all_threads(&mut self, cx: &mut Context<Self>) {
        if self.slack_all_threads_loading {
            return;
        }
        let Some(cursor) = self
            .slack_all_threads_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone())
        else {
            return;
        };
        self.begin_slack_all_threads_page(Some(cursor), cx);
    }

    fn queue_slack_all_threads_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.saturating_sub(SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_all_threads_rows.len());
        let urls = self.slack_all_threads_rows[start..end]
            .iter()
            .flat_map(|row| row.remote_image_urls.iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}

fn load_slack_all_threads_page(
    workspace_api: Arc<dyn WorkspaceApi>,
    mut request: SlackAllThreadsPageRequest,
) -> (
    SlackAllThreadsPageRequest,
    Result<PreparedSlackAllThreadsSnapshot, String>,
) {
    let existing = request.existing.take();
    let result = workspace_api
        .load_slack_all_threads(request.cursor.as_ref())
        .and_then(|page| merge_and_prepare_slack_all_threads_snapshot(existing, page));
    (request, result)
}

fn next_slack_all_threads_generation(generation: u64, overflow_message: &str) -> u64 {
    generation.checked_add(1).expect(overflow_message)
}
