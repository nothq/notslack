mod hydration;
mod paging;
mod reminder;

use std::{collections::HashSet, sync::Arc};

use gpui::ListOffset;

use super::{
    px, workspace::slack_message_reaction_remote_image_urls, Context, SlackRailView, SurfaceState,
    WorkspaceApi,
};
use crate::ui::surface::{
    build_slack_later_rows, mark_slack_later_hydration_error, prepare_slack_later_hydrated_row,
    SlackLaterRow, SlackLaterRowContent,
};
use crate::ui::{
    SlackLaterCursor, SlackLaterFilter, SlackLaterHydrationTarget, SlackLaterItemKey,
    SlackLaterSnapshot,
};

const SLACK_LATER_INITIAL_VISIBLE_ROWS: usize = 8;
const SLACK_LATER_IMAGE_VISIBLE_OVERDRAW: usize = 2;
const SLACK_LATER_IMAGE_PREFETCH_LIMIT: usize = 24;
const SLACK_LATER_PAGINATION_THRESHOLD: usize = 3;
const SLACK_LATER_MAX_HYDRATIONS: usize = 4;

struct SlackLaterPageRequest {
    generation: u64,
    team_id: String,
    filter: SlackLaterFilter,
    cursor: Option<SlackLaterCursor>,
}

struct SlackLaterHydrationRequest {
    generation: u64,
    team_id: String,
    filter: SlackLaterFilter,
    target: SlackLaterHydrationTarget,
}

type SlackLaterPreparedPage = Result<(SlackLaterSnapshot, Vec<SlackLaterRow>), String>;
type SlackLaterPageLoadResult = (SlackLaterPageRequest, SlackLaterPreparedPage);

impl SurfaceState {
    pub(in crate::ui::surface::state) fn refresh_slack_later_from_realtime_if_visible(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_realtime_later_stale
            || self.slack_active_rail_view != SlackRailView::Later
            || self.slack_later_loading
        {
            return;
        }
        self.slack_realtime_later_stale = false;
        self.slack_later_generation = next_slack_later_generation(self.slack_later_generation);
        self.slack_later_snapshot = None;
        self.slack_later_error = None;
        self.begin_slack_later_page(None, cx);
    }

    pub(crate) fn activate_slack_later(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_later {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            self.slack_later_error =
                Some("Slack Later requires a loaded workspace identity.".to_string());
            cx.notify();
            return;
        };
        if self.slack_later_team_id.as_deref() != Some(team_id.as_str()) {
            self.reset_slack_later_context();
            self.slack_later_team_id = Some(team_id);
        }
        self.leave_slack_activity(cx);
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.origin.is_conversation())
        {
            self.reset_slack_thread_context();
        }
        self.slack_active_rail_view = SlackRailView::Later;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.slack_later_list_state.remeasure();
        self.queue_selected_slack_later_hydration(cx);
        self.queue_slack_later_hydration_range(0, SLACK_LATER_INITIAL_VISIBLE_ROWS, cx);
        self.queue_slack_later_visible_images(0, SLACK_LATER_INITIAL_VISIBLE_ROWS, cx);
        self.sync_slack_later_selected_detail(cx);
        cx.notify();
        if self.slack_later_snapshot.is_none() && !self.slack_later_loading {
            self.begin_slack_later_page(None, cx);
        }
    }

    pub(crate) fn leave_slack_later(&mut self) {
        if self.slack_active_rail_view != SlackRailView::Later {
            return;
        }
        self.slack_later_generation = next_slack_later_generation(self.slack_later_generation);
        self.slack_later_loading = false;
        self.slack_later_hydration_queue.reset();
        if self.slack_later_reminder_mutating {
            self.slack_later_snapshot = None;
            self.slack_later_rows.clear();
            self.slack_later_list_state.reset(0);
            self.slack_later_error = None;
        } else {
            self.slack_later_reminder_dialog = None;
            self.slack_later_reminder_generation =
                next_slack_later_generation(self.slack_later_reminder_generation);
            self.slack_later_reminder_error = None;
        }
        self.reset_slack_later_thread_context();
    }

    pub(crate) fn reset_slack_later_context(&mut self) {
        self.slack_later_generation = next_slack_later_generation(self.slack_later_generation);
        self.slack_later_team_id = None;
        self.slack_later_snapshot = None;
        self.slack_later_rows.clear();
        self.slack_later_list_state.reset(0);
        self.slack_later_filter = SlackLaterFilter::Saved;
        self.slack_later_selected_key = None;
        self.slack_later_hydration_queue.reset();
        self.slack_later_loading = false;
        self.slack_later_error = None;
        self.slack_later_reminder_dialog = None;
        self.slack_later_reminder_generation =
            next_slack_later_generation(self.slack_later_reminder_generation);
        self.slack_later_reminder_mutating = false;
        self.slack_later_reminder_error = None;
        self.reset_slack_later_thread_context();
    }

    pub(crate) fn select_slack_later_filter(
        &mut self,
        filter: SlackLaterFilter,
        cx: &mut Context<Self>,
    ) {
        if self.slack_later_reminder_mutating || self.slack_later_filter == filter {
            return;
        }
        self.slack_later_generation = next_slack_later_generation(self.slack_later_generation);
        self.slack_later_filter = filter;
        self.slack_later_snapshot = None;
        self.slack_later_rows.clear();
        self.slack_later_list_state.reset(0);
        self.slack_later_selected_key = None;
        self.slack_later_hydration_queue.reset();
        self.slack_later_loading = false;
        self.slack_later_error = None;
        self.slack_later_reminder_dialog = None;
        self.slack_later_reminder_generation =
            next_slack_later_generation(self.slack_later_reminder_generation);
        self.slack_later_reminder_mutating = false;
        self.slack_later_reminder_error = None;
        self.reset_slack_later_thread_context();
        cx.notify();
        self.begin_slack_later_page(None, cx);
    }

    pub(crate) fn retry_slack_later(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::Later || self.slack_later_loading {
            return;
        }
        let cursor = self
            .slack_later_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        self.begin_slack_later_page(cursor, cx);
    }

    pub(crate) fn slack_later_filter_count(&self, filter: SlackLaterFilter) -> u32 {
        let Some(snapshot) = self.slack_later_snapshot.as_ref() else {
            return 0;
        };
        match filter {
            SlackLaterFilter::Saved => snapshot.counts.in_progress,
            SlackLaterFilter::Archived => snapshot.counts.archived,
            SlackLaterFilter::Completed => snapshot.counts.completed,
        }
    }

    pub(crate) fn select_slack_later_row(
        &mut self,
        key: SlackLaterItemKey,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_later_rows.iter().any(|row| row.key == key) {
            return;
        }
        self.slack_later_selected_key = Some(key);
        self.slack_later_reminder_error = None;
        self.queue_selected_slack_later_hydration(cx);
        self.queue_selected_slack_later_images(cx);
        self.sync_slack_later_selected_detail(cx);
        cx.notify();
    }

    pub(crate) fn close_slack_later_detail(&mut self, cx: &mut Context<Self>) {
        if self.slack_later_selected_key.take().is_none() {
            return;
        }
        self.slack_later_reminder_error = None;
        self.reset_slack_later_thread_context();
        cx.notify();
    }

    pub(crate) fn move_slack_later_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        let count = self.slack_later_rows.len();
        if count == 0 {
            return;
        }
        let selected = self
            .slack_later_selected_key
            .as_ref()
            .and_then(|key| self.slack_later_rows.iter().position(|row| &row.key == key))
            .unwrap_or(0);
        let next = if direction < 0 {
            selected.checked_sub(1).unwrap_or(count - 1)
        } else {
            (selected + 1) % count
        };
        self.slack_later_selected_key = Some(self.slack_later_rows[next].key.clone());
        self.slack_later_list_state.scroll_to(ListOffset {
            item_ix: next,
            offset_in_item: px(0.0),
        });
        self.queue_selected_slack_later_hydration(cx);
        self.queue_selected_slack_later_images(cx);
        self.sync_slack_later_selected_detail(cx);
        cx.notify();
    }

    pub(crate) fn handle_slack_later_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_later_hydration_range(visible_start, visible_end, cx);
        self.queue_slack_later_visible_images(visible_start, visible_end, cx);
        if visible_end.saturating_add(SLACK_LATER_PAGINATION_THRESHOLD) >= count {
            self.maybe_load_more_slack_later(cx);
        }
    }

    pub(crate) fn selected_slack_later_row(&self) -> Option<&SlackLaterRow> {
        let key = self.slack_later_selected_key.as_ref()?;
        self.slack_later_rows.iter().find(|row| &row.key == key)
    }
}

fn load_slack_later_page(
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackLaterPageRequest,
) -> SlackLaterPageLoadResult {
    let result = workspace_api
        .load_slack_later(request.filter, request.cursor.as_ref())
        .map(|snapshot| {
            let rows = build_slack_later_rows(&snapshot);
            (snapshot, rows)
        });
    (request, result)
}

fn hydrate_slack_later_row(
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackLaterHydrationRequest,
) -> (SlackLaterHydrationRequest, Result<SlackLaterRow, String>) {
    let team_id = request.team_id.clone();
    let result = workspace_api
        .hydrate_slack_later_item(request.target.clone())
        .map(|hydrated| prepare_slack_later_hydrated_row(hydrated, &team_id));
    (request, result)
}

fn append_slack_later_rows(existing: &mut Vec<SlackLaterRow>, page: Vec<SlackLaterRow>) {
    let mut keys = existing
        .iter()
        .map(|row| row.key.clone())
        .collect::<HashSet<_>>();
    existing.extend(page.into_iter().filter(|row| keys.insert(row.key.clone())));
}

fn merge_slack_later_snapshots(
    existing: Option<SlackLaterSnapshot>,
    page: SlackLaterSnapshot,
) -> SlackLaterSnapshot {
    let Some(existing) = existing else {
        return page;
    };
    let mut keys = existing
        .items
        .iter()
        .map(|item| item.key.clone())
        .collect::<HashSet<_>>();
    let mut items = existing.items;
    items.extend(
        page.items
            .into_iter()
            .filter(|item| keys.insert(item.key.clone())),
    );
    SlackLaterSnapshot {
        filter: page.filter,
        items,
        counts: page.counts,
        next_cursor: page.next_cursor,
    }
}

fn collect_slack_later_row_urls(rows: &[SlackLaterRow]) -> Vec<String> {
    let mut urls = HashSet::new();
    for row in rows {
        if let Some(url) = row.avatar_image_url.as_deref() {
            urls.insert(url.to_string());
        }
        match &row.content {
            SlackLaterRowContent::Message { detail } => {
                urls.extend(
                    slack_message_reaction_remote_image_urls(&detail.selected_message_row)
                        .map(str::to_string),
                );
                for attachment in &detail.selected_message_row.attachments {
                    if let Some(url) = attachment.attachment.preview_image_url.as_ref() {
                        urls.insert(url.clone());
                    }
                }
            }
            SlackLaterRowContent::File { attachment } => {
                if let Some(url) = attachment.preview_image_url.as_ref() {
                    urls.insert(url.clone());
                }
            }
            SlackLaterRowContent::Placeholder { .. }
            | SlackLaterRowContent::HydrationError { .. }
            | SlackLaterRowContent::Reminder { .. }
            | SlackLaterRowContent::Tombstone
            | SlackLaterRowContent::Unsupported => {}
        }
        if urls.len() >= SLACK_LATER_IMAGE_PREFETCH_LIMIT {
            break;
        }
    }
    urls.into_iter().collect()
}

fn next_slack_later_generation(generation: u64) -> u64 {
    generation
        .checked_add(1)
        .expect("Slack Later request generation overflowed")
}
