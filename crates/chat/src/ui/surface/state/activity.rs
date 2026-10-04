mod detail;
mod loading;
mod paging;
mod read;

use std::sync::Arc;

use gpui::{ListOffset, SharedString};

use super::{
    prepare_slack_conversation_snapshot, prepare_slack_thread_snapshot, px,
    workspace::slack_message_reaction_remote_image_urls, Context,
    PreparedSlackConversationSnapshot, PreparedSlackThreadSnapshot, SlackMessageRow, SlackRailView,
    SurfaceState, WorkspaceApi,
};
use crate::ui::surface::{
    merge_and_prepare_slack_activity_snapshot, prepare_slack_activity_workspace_context,
    PreparedSlackActivitySnapshot, SlackActivityDetailState, SlackActivityDetailTarget,
    SlackActivityFilter, SlackActivityRow, SlackActivityWorkspaceContext,
};
use crate::ui::{SlackActivityCursor, SlackActivitySnapshot, SlackMessageTimestamp};

use loading::{
    collect_slack_activity_detail_row_urls, find_activity_anchor_index, load_slack_activity_detail,
    load_slack_activity_page, slack_activity_thread_detail_rows,
};

const SLACK_ACTIVITY_IMAGE_VISIBLE_OVERDRAW: usize = 2;
const SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS: usize = 8;
const SLACK_ACTIVITY_PAGINATION_THRESHOLD: usize = 4;
const SLACK_ACTIVITY_DETAIL_IMAGE_VISIBLE_OVERDRAW: usize = 3;

struct SlackActivityPageRequest {
    generation: u64,
    team_id: String,
    cursor: Option<SlackActivityCursor>,
    existing: Option<SlackActivitySnapshot>,
    workspace: SlackActivityWorkspaceContext,
}

struct SlackActivityDetailRequest {
    generation: u64,
    team_id: String,
    self_user_id: String,
    key: SharedString,
    channel_id: SharedString,
    channel_label: Option<SharedString>,
    message_timestamp: SlackMessageTimestamp,
    thread_timestamp: Option<SlackMessageTimestamp>,
}

struct SlackActivityDetailInstallation {
    key: SharedString,
    channel_label: SharedString,
    message_timestamp: SlackMessageTimestamp,
    target: SlackActivityDetailTarget,
    rows: Arc<[SlackMessageRow]>,
    anchor_index: usize,
}

enum PreparedSlackActivityDetail {
    Conversation(Box<PreparedSlackConversationSnapshot>),
    Thread(Box<PreparedSlackThreadSnapshot>),
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn refresh_slack_activity_from_realtime_if_visible(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_realtime_activity_stale
            || self.slack_active_rail_view != SlackRailView::Activity
            || self.slack_activity_loading
        {
            return;
        }
        self.slack_realtime_activity_stale = false;
        self.slack_activity_snapshot = None;
        self.slack_activity_error = None;
        self.begin_slack_activity_page(None, cx);
    }

    pub(crate) fn activate_slack_activity(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_activity {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            self.slack_activity_error =
                Some("Slack Activity requires a loaded workspace identity.".to_string());
            cx.notify();
            return;
        };
        if self.slack_activity_team_id.as_deref() != Some(team_id.as_str()) {
            self.reset_slack_activity_context(cx);
            self.slack_activity_team_id = Some(team_id);
        }
        self.leave_slack_later();
        self.slack_active_rail_view = SlackRailView::Activity;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        let composer = self.slack_activity_detail.composer().cloned();
        if let Some(composer) = composer {
            self.activate_slack_activity_main_composer(composer, cx);
        } else {
            self.park_slack_main_composer(cx);
        }
        self.rebuild_slack_activity_local_delivery_rows();
        self.refresh_slack_activity_visible_rows(cx);
        self.queue_slack_activity_visible_images(0, SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
        if self.slack_activity_snapshot.is_none() && !self.slack_activity_loading {
            self.begin_slack_activity_page(None, cx);
        }
    }

    pub(crate) fn leave_slack_activity(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Activity {
            return;
        }
        self.restore_slack_routed_main_composer(cx);
        self.rebuild_slack_activity_local_delivery_rows();
        self.slack_activity_generation = next_slack_activity_generation(
            self.slack_activity_generation,
            "Slack Activity request generation overflowed",
        );
        self.slack_activity_loading = false;
        self.slack_activity_detail_generation = next_slack_activity_generation(
            self.slack_activity_detail_generation,
            "Slack Activity detail generation overflowed",
        );
        if matches!(
            self.slack_activity_detail,
            SlackActivityDetailState::Loading { .. }
        ) {
            self.slack_activity_detail = SlackActivityDetailState::Empty;
        }
    }

    pub(crate) fn reset_slack_activity_context(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_main_composer_context.is_some() {
            self.restore_slack_routed_main_composer(cx);
        }
        self.slack_activity_generation = next_slack_activity_generation(
            self.slack_activity_generation,
            "Slack Activity request generation overflowed",
        );
        self.slack_activity_detail_generation = next_slack_activity_generation(
            self.slack_activity_detail_generation,
            "Slack Activity detail generation overflowed",
        );
        self.slack_activity_team_id = None;
        self.slack_activity_snapshot = None;
        self.slack_activity_rows = Arc::default();
        self.slack_activity_visible_row_indices = Arc::default();
        self.slack_activity_list_state.reset(0);
        self.slack_activity_filter = SlackActivityFilter::All;
        self.slack_activity_filter_counts = [0; 4];
        self.slack_activity_unread_only = false;
        self.slack_activity_selected_key = None;
        self.slack_activity_loading = false;
        self.slack_activity_error = None;
        self.slack_activity_detail = SlackActivityDetailState::Empty;
        self.slack_activity_detail_list_state.reset(0);
        self.slack_activity_local_delivery_rows = Arc::default();
        self.reset_slack_activity_mutation_context();
    }

    pub(crate) fn retry_slack_activity(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Activity || self.slack_activity_loading {
            return;
        }
        let cursor = self
            .slack_activity_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        self.begin_slack_activity_page(cursor, cx);
    }

    pub(crate) fn select_slack_activity_filter(
        &mut self,
        filter: SlackActivityFilter,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_filter == filter {
            return;
        }
        self.slack_activity_filter = filter;
        self.refresh_slack_activity_visible_rows(cx);
        self.queue_slack_activity_visible_images(0, SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn toggle_slack_activity_unread_only(&mut self, cx: &mut Context<Self>) {
        self.slack_activity_unread_only = !self.slack_activity_unread_only;
        self.refresh_slack_activity_visible_rows(cx);
        self.queue_slack_activity_visible_images(0, SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn slack_activity_filter_count(&self, filter: SlackActivityFilter) -> u32 {
        self.slack_activity_filter_counts[slack_activity_filter_index(filter)]
    }

    pub(crate) fn handle_slack_activity_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_activity_visible_images(visible_start, visible_end, cx);
        if visible_end.saturating_add(SLACK_ACTIVITY_PAGINATION_THRESHOLD) >= count {
            self.maybe_load_more_slack_activity(cx);
        }
    }

    pub(crate) fn handle_slack_activity_detail_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_activity_detail_visible_images(visible_start, visible_end, cx);
    }

    pub(crate) fn select_slack_activity_row(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self
            .slack_activity_visible_rows()
            .any(|row| row.key.as_ref() == key)
        {
            return;
        }
        self.slack_activity_selected_key = Some(key.to_string().into());
        self.activate_selected_slack_activity(cx);
    }

    pub(crate) fn move_slack_activity_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        let row_count = self.slack_activity_visible_row_indices.len();
        if row_count == 0 {
            return;
        }
        let selected_index = self
            .slack_activity_selected_key
            .as_deref()
            .and_then(|selected_key| {
                self.slack_activity_visible_rows()
                    .position(|row| row.key.as_ref() == selected_key)
            });
        let next_index = match (selected_index, direction < 0) {
            (Some(selected_index), true) => selected_index.checked_sub(1).unwrap_or(row_count - 1),
            (Some(selected_index), false) => (selected_index + 1) % row_count,
            (None, true) => row_count - 1,
            (None, false) => 0,
        };
        let row_index = self.slack_activity_visible_row_indices[next_index];
        self.slack_activity_selected_key = Some(self.slack_activity_rows[row_index].key.clone());
        self.slack_activity_list_state.scroll_to(ListOffset {
            item_ix: next_index,
            offset_in_item: px(0.0),
        });
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_activity(&mut self, cx: &mut Context<Self>) {
        let Some(selected_key) = self.slack_activity_selected_key.clone() else {
            return;
        };
        let Some(row) = self
            .slack_activity_visible_rows()
            .find(|row| row.key == selected_key)
            .cloned()
        else {
            return;
        };
        self.queue_slack_activity_item_read(&row, cx);
        if self.slack_activity_detail.key() == Some(row.key.as_ref())
            && !matches!(
                self.slack_activity_detail,
                SlackActivityDetailState::Error { .. }
            )
        {
            return;
        }
        self.load_slack_activity_detail(row, cx);
    }

    pub(crate) fn close_slack_activity_detail(&mut self, cx: &mut Context<Self>) {
        self.slack_activity_detail_generation = next_slack_activity_generation(
            self.slack_activity_detail_generation,
            "Slack Activity detail generation overflowed",
        );
        self.restore_slack_routed_main_composer(cx);
        self.slack_activity_detail = SlackActivityDetailState::Empty;
        self.slack_activity_detail_list_state.reset(0);
        self.slack_activity_local_delivery_rows = Arc::default();
        cx.notify();
    }

    pub(crate) fn retry_slack_activity_detail(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.slack_activity_detail.key().map(str::to_string) else {
            return;
        };
        let Some(row) = self
            .slack_activity_rows
            .iter()
            .find(|row| row.key.as_ref() == key)
            .cloned()
        else {
            return;
        };
        self.load_slack_activity_detail(row, cx);
    }
}

fn slack_activity_filter_index(filter: SlackActivityFilter) -> usize {
    match filter {
        SlackActivityFilter::All => 0,
        SlackActivityFilter::Dms => 1,
        SlackActivityFilter::Mentions => 2,
        SlackActivityFilter::Threads => 3,
    }
}

fn next_slack_activity_generation(current: u64, message: &'static str) -> u64 {
    current.checked_add(1).expect(message)
}
