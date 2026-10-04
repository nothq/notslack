use super::{
    prepare_slack_conversation_history_page, Context, PreparedSlackConversationHistoryPage,
    SlackConversationHistoryRequest, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi,
};
use crate::ui::{
    surface::{build_slack_message_chunks, SlackConversationLiveTarget},
    SlackConversationSnapshot, SlackWorkspace,
};
use gpui::ListOffset;
use std::sync::Arc;

const SLACK_HISTORY_PAGE_TRIGGER_ROW: usize = 6;

type SlackMessageHistoryPageLoad = (
    Arc<dyn WorkspaceApi>,
    SlackConversationHistoryRequest,
    SlackConversationSnapshot,
    Arc<SlackWorkspace>,
);

impl SurfaceState {
    pub(crate) fn handle_slack_message_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        is_following_tail: bool,
        cx: &mut Context<Self>,
    ) {
        self.slack_message_list_auto_position_active = false;
        if is_following_tail && self.slack_new_message_count > 0 {
            self.slack_new_message_count = 0;
            cx.notify();
        }
        self.mark_slack_remote_image_queue_dirty();
        self.queue_slack_conversation_read_for_visible_range(
            visible_start,
            visible_end,
            is_following_tail,
            cx,
        );
        if self.slack_message_rows.is_empty() || visible_start > SLACK_HISTORY_PAGE_TRIGGER_ROW {
            return;
        }
        self.start_slack_message_history_page_load(cx);
    }

    pub(crate) fn jump_to_first_unread_slack_message(&mut self, cx: &mut Context<Self>) {
        let item_ix = if self.slack_new_message_count > 0 {
            self.slack_message_rows
                .len()
                .saturating_sub(self.slack_new_message_count)
        } else {
            let Some(item_ix) = self
                .slack_message_rows
                .iter()
                .position(|row| row.unread_boundary_before)
            else {
                return;
            };
            item_ix
        };
        if item_ix >= self.slack_message_rows.len() {
            return;
        }
        self.slack_message_list_auto_position_active = false;
        self.slack_new_message_count = 0;
        self.slack_message_list_state.scroll_to(ListOffset {
            item_ix,
            offset_in_item: gpui::px(0.0),
        });
        cx.notify();
    }

    pub(crate) fn dismiss_slack_new_messages(&mut self, cx: &mut Context<Self>) {
        if self.slack_new_message_count == 0 {
            return;
        }
        self.slack_new_message_count = 0;
        cx.notify();
    }

    pub(in crate::ui::surface) fn invalidate_slack_message_history_requests(&mut self) {
        self.slack_message_history_generation = self
            .slack_message_history_generation
            .checked_add(1)
            .expect("Slack message history request generation overflowed");
        self.slack_message_history_request = None;
    }

    pub(super) fn start_slack_message_history_page_load(&mut self, cx: &mut Context<Self>) {
        if !self
            .slack_workspace_api_capabilities
            .load_conversation_history
            || self.slack_message_history_request.is_some()
            || self.slack_conversation_refresh_request.is_some()
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
            || self.slack_active_tab != SlackMainTab::Messages
        {
            return;
        }
        let Some((workspace_api, request, snapshot, workspace)) =
            self.prepare_slack_message_history_page_load(cx)
        else {
            return;
        };

        let completion_request = request.clone();
        self.spawn_background_task(
            (request, snapshot, workspace),
            cx,
            move |(request, snapshot, workspace)| {
                load_and_prepare_slack_message_history_page(
                    workspace_api,
                    &request,
                    snapshot,
                    workspace,
                )
            },
            move |this, result, cx| {
                this.finish_slack_message_history_page_load(&completion_request, result, cx);
            },
        );
    }

    fn prepare_slack_message_history_page_load(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<SlackMessageHistoryPageLoad> {
        let snapshot = self.slack_conversation_snapshot.clone()?;
        let cursor = snapshot.history_next_cursor.clone()?;
        let workspace = self.slack_workspace.clone()?;
        if workspace.team_id != snapshot.team_id
            || workspace.conversation_id != snapshot.conversation_id
            || self
                .slack_pending_conversation_id
                .as_deref()
                .is_some_and(|pending| pending != snapshot.conversation_id)
        {
            return None;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return None;
        };
        self.slack_message_history_generation = self
            .slack_message_history_generation
            .checked_add(1)
            .expect("Slack message history request generation overflowed");
        let request = SlackConversationHistoryRequest {
            generation: self.slack_message_history_generation,
            team_id: snapshot.team_id.clone(),
            conversation_id: snapshot.conversation_id.clone(),
            cursor,
        };
        self.slack_message_history_request = Some(request.clone());
        self.slack_error = None;
        cx.notify();
        Some((workspace_api, request, snapshot, workspace))
    }

    fn finish_slack_message_history_page_load(
        &mut self,
        request: &SlackConversationHistoryRequest,
        result: Result<PreparedSlackConversationHistoryPage, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_message_history_request.as_ref() != Some(request) {
            return;
        }
        self.slack_message_history_request = None;
        self.schedule_queued_slack_conversation_reconciliation(cx);
        if self.slack_message_history_generation != request.generation
            || !self.slack_message_history_target_is_current(request)
            || self
                .slack_conversation_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.history_next_cursor.as_ref())
                != Some(&request.cursor)
        {
            return;
        }
        match result {
            Ok(prepared) => self.apply_prepared_slack_message_history_page(prepared, cx),
            Err(message) => {
                self.cancel_slack_message_navigation();
                self.slack_error = Some(message);
                cx.notify();
            }
        }
    }

    fn slack_message_history_target_is_current(
        &self,
        request: &SlackConversationHistoryRequest,
    ) -> bool {
        self.slack_workspace.as_ref().is_some_and(|workspace| {
            workspace.team_id == request.team_id
                && workspace.conversation_id == request.conversation_id
        }) && self
            .slack_conversation_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.team_id == request.team_id
                    && snapshot.conversation_id == request.conversation_id
            })
            && self
                .slack_pending_conversation_id
                .as_deref()
                .is_none_or(|pending| pending == request.conversation_id)
    }

    fn apply_prepared_slack_message_history_page(
        &mut self,
        prepared: PreparedSlackConversationHistoryPage,
        cx: &mut Context<Self>,
    ) {
        let PreparedSlackConversationHistoryPage {
            snapshot,
            message_rows,
            message_rows_local_today,
            message_chunks,
            remote_images,
            prepended_row_count,
        } = prepared;
        let read_target = SlackConversationLiveTarget {
            team_id: snapshot.team_id.clone(),
            conversation_id: snapshot.conversation_id.clone(),
        };
        let last_read = snapshot.last_read.clone();
        let last_read_boundary_loaded = snapshot.last_read_boundary_loaded;
        let previous_row_count = self.slack_message_rows.len();
        assert_eq!(
            message_rows.len(),
            previous_row_count + prepended_row_count,
            "prepared Slack history rows must only prepend messages"
        );
        self.pause_slack_conversation_read_for_live_positioning();
        self.advance_slack_conversation_revision();
        self.slack_remote_images.extend(remote_images);
        self.slack_message_rows = message_rows;
        self.slack_message_rows_local_today = Some(message_rows_local_today);
        self.slack_message_chunks = message_chunks;
        self.slack_conversation_snapshot = Some(snapshot.clone());
        self.slack_workspace_mut()
            .expect("Slack history page requires an active workspace")
            .apply_shaped_conversation_snapshot(snapshot);
        if prepended_row_count > 0 {
            self.slack_message_list_state
                .splice(0..0, prepended_row_count);
            if prepended_row_count < self.slack_message_rows.len() {
                self.slack_message_list_state
                    .remeasure_items(prepended_row_count..(prepended_row_count + 1));
            }
            let rows = self.slack_message_rows.clone();
            self.prefetch_slack_message_preview_images(&rows[..prepended_row_count], cx);
        }
        self.install_slack_conversation_read_after_live_positioning(
            read_target,
            last_read.as_ref(),
            last_read_boundary_loaded,
            cx,
        );
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
        self.slack_error = None;
        self.continue_slack_message_navigation(cx);
        cx.notify();
    }
}

fn load_and_prepare_slack_message_history_page(
    workspace_api: Arc<dyn WorkspaceApi>,
    request: &SlackConversationHistoryRequest,
    snapshot: SlackConversationSnapshot,
    mut workspace: Arc<SlackWorkspace>,
) -> Result<PreparedSlackConversationHistoryPage, String> {
    let page = workspace_api
        .load_slack_conversation_history_page(&request.conversation_id, &request.cursor)?;
    if page.team_id != request.team_id || page.conversation_id != request.conversation_id {
        return Err(
            "Slack conversation history response targeted a different conversation".to_string(),
        );
    }
    let mut prepared = prepare_slack_conversation_history_page(snapshot, page)?;
    if Arc::make_mut(&mut workspace).apply_conversation_snapshot(prepared.snapshot.clone()) {
        prepared.snapshot = workspace.conversation_snapshot();
        let (message_rows, message_rows_local_today) =
            crate::ui::surface::build_slack_conversation_message_rows_with_local_today(
                &prepared.snapshot,
            );
        prepared.message_rows = message_rows;
        prepared.message_rows_local_today = message_rows_local_today;
        prepared.message_chunks = build_slack_message_chunks(&prepared.message_rows);
    }
    Ok(prepared)
}
