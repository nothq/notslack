mod sidebar_state;

use std::collections::HashSet;

use super::{
    build_slack_dm_rows, prepare_slack_dm_inbox_snapshot, Context, PreparedSlackDmInboxSnapshot,
    SurfaceState,
};
use crate::ui::{SlackConversationKind, SlackDmInboxSnapshot, SlackWorkspace};

use self::sidebar_state::merge_slack_dm_sidebar_state;

const SLACK_DM_IMAGE_VISIBLE_OVERDRAW: usize = 2;
const SLACK_DM_INITIAL_VISIBLE_ROWS: usize = 12;
const SLACK_DM_PAGINATION_THRESHOLD: usize = 24;

impl SurfaceState {
    pub(crate) fn clear_slack_dm_inbox(&mut self) {
        self.slack_dms_peek_visible = false;
        self.slack_dm_inbox_snapshot = None;
        self.slack_dm_rows = Default::default();
        self.slack_dm_visible_row_indices = Default::default();
        self.slack_dm_pagination_cursor = None;
        self.slack_dm_list_state.reset(0);
        self.reset_slack_dm_finder();
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_dm(data);
    }

    pub(super) fn slack_dm_inbox_item_count(&self, _workspace: &SlackWorkspace) -> usize {
        self.slack_dm_visible_row_indices.len()
    }

    pub(super) fn sync_slack_dm_list_state(&self, _workspace: &SlackWorkspace) {
        let next_count = self.slack_dm_visible_row_indices.len();
        if self.slack_dm_list_state.item_count() != next_count {
            self.slack_dm_list_state.reset(next_count);
        } else {
            self.slack_dm_list_state.remeasure();
        }
    }

    pub(super) fn apply_prepared_slack_dm_inbox_snapshot(
        &mut self,
        mut prepared: PreparedSlackDmInboxSnapshot,
        append: bool,
        cx: &mut Context<Self>,
    ) {
        if prepared.snapshot.team_id.is_empty() {
            return;
        }
        if !self.slack_dm_snapshot_team_applies(&prepared.snapshot.team_id) {
            return;
        }
        self.slack_presence_authority
            .overlay_prepared_dm(&mut prepared);
        let PreparedSlackDmInboxSnapshot {
            mut snapshot,
            mut rows,
        } = prepared;
        if merge_slack_dm_sidebar_state(&mut snapshot, self.slack_sidebar_snapshot.as_ref()) {
            rows = build_slack_dm_rows(&snapshot);
        }
        if append {
            self.append_slack_dm_inbox_snapshot(snapshot, rows);
        } else {
            self.slack_dm_inbox_snapshot = Some(snapshot);
            self.slack_dm_rows = self.project_slack_conversation_read_dm_rows(rows);
        }
        self.slack_dm_pagination_cursor = None;
        self.refresh_slack_dm_visible_rows();
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_dm(data);
        }
        self.queue_slack_dm_visible_images_for_current_view(cx);
        cx.notify();
    }

    fn slack_dm_snapshot_team_applies(&self, snapshot_team_id: &str) -> bool {
        self.slack_workspace
            .as_ref()
            .map(|workspace| workspace.team_id.as_str())
            .or_else(|| {
                self.slack_shell_snapshot
                    .as_ref()
                    .map(|shell| shell.team_id.as_str())
            })
            .or_else(|| {
                self.slack_shell
                    .as_ref()
                    .map(|shell| shell.team_id.as_str())
            })
            .is_none_or(|team_id| team_id == snapshot_team_id)
    }

    fn append_slack_dm_inbox_snapshot(
        &mut self,
        snapshot: SlackDmInboxSnapshot,
        rows: std::sync::Arc<[crate::ui::surface::SlackDmRow]>,
    ) {
        let mut current_snapshot =
            self.slack_dm_inbox_snapshot
                .take()
                .unwrap_or_else(|| SlackDmInboxSnapshot {
                    team_id: snapshot.team_id.clone(),
                    self_user_id: snapshot.self_user_id.clone(),
                    ..SlackDmInboxSnapshot::default()
                });
        let mut current_rows = self.slack_dm_rows.iter().cloned().collect::<Vec<_>>();
        let mut seen = current_snapshot
            .items
            .iter()
            .map(|item| item.conversation_id.clone())
            .collect::<HashSet<_>>();
        for (item, row) in snapshot.items.into_iter().zip(rows.iter().cloned()) {
            if seen.insert(item.conversation_id.clone()) {
                current_snapshot.items.push(item);
                current_rows.push(row);
            }
        }
        if snapshot.slackbot_conversation_id.is_some() {
            current_snapshot.slackbot_conversation_id = snapshot.slackbot_conversation_id;
        }
        current_snapshot.next_cursor = snapshot.next_cursor;
        self.slack_dm_inbox_snapshot = Some(current_snapshot);
        self.slack_dm_rows = self.project_slack_conversation_read_dm_rows(current_rows.into());
    }

    pub(super) fn merge_slack_dm_sidebar_state(&mut self) {
        let sidebar = self.slack_sidebar_snapshot.clone();
        let Some(snapshot) = self.slack_dm_inbox_snapshot.as_mut() else {
            return;
        };
        if !merge_slack_dm_sidebar_state(snapshot, sidebar.as_ref()) {
            return;
        }
        let rows = build_slack_dm_rows(snapshot);
        self.slack_dm_rows = self.project_slack_conversation_read_dm_rows(rows);
        self.refresh_slack_dm_visible_rows();
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_dm(data);
    }

    pub(super) fn refresh_slack_dm_visible_rows(&mut self) {
        self.slack_dm_visible_row_indices = self
            .slack_dm_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                !self.slack_dms_show_unread_only
                    || row.unread
                    || row.mention_count.is_some_and(|count| count > 0)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        if self.slack_dm_list_state.item_count() != self.slack_dm_visible_row_indices.len() {
            self.slack_dm_list_state
                .reset(self.slack_dm_visible_row_indices.len());
        } else {
            self.slack_dm_list_state.remeasure();
        }
        if self.slack_dm_finder_active() {
            self.rebuild_slack_dm_finder_results();
        }
    }

    pub(crate) fn handle_slack_dm_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_dm_visible_images(visible_start, visible_end, cx);
        self.maybe_load_more_slack_dms(visible_end, count, cx);
    }

    pub(super) fn queue_slack_dm_visible_images_for_current_view(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_rail_view != super::SlackRailView::Dms && !self.slack_dms_peek_visible
        {
            return;
        }
        if self.slack_active_rail_view == super::SlackRailView::Dms && self.slack_dm_finder_active()
        {
            return;
        }
        self.queue_slack_dm_visible_images(0, SLACK_DM_INITIAL_VISIBLE_ROWS, cx);
    }

    pub(crate) fn queue_slack_dm_finder_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_dm_finder_active() {
            return;
        }
        let start = visible_start
            .saturating_sub(SLACK_DM_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_dm_finder_row_indices.len());
        let end = visible_end
            .saturating_add(SLACK_DM_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_dm_finder_row_indices.len());
        if self.slack_dm_finder_prefetched_range == Some((start, end)) {
            return;
        }
        self.slack_dm_finder_prefetched_range = Some((start, end));
        let mut queued = self
            .slack_pending_remote_image_urls
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let urls = self.slack_dm_finder_row_indices[start..end]
            .iter()
            .filter_map(|index| self.slack_dm_rows.get(*index))
            .filter(|row| row.kind != SlackConversationKind::GroupMessage)
            .filter_map(|row| row.participants.first())
            .filter_map(|participant| participant.avatar_image_url.as_ref())
            .map(|url| url.as_ref())
            .filter(|url| !self.slack_remote_images.contains_key(*url))
            .filter(|url| !self.slack_active_remote_image_urls.contains(*url))
            .filter(|url| queued.insert(url))
            .map(str::to_string)
            .collect::<Vec<_>>();
        self.slack_pending_remote_image_urls
            .extend(urls.into_iter().rev());
        self.start_next_slack_remote_image_load(cx);
    }

    fn queue_slack_dm_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.saturating_sub(SLACK_DM_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_DM_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_dm_visible_row_indices.len());
        let mut queued = self
            .slack_pending_remote_image_urls
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let urls = self.slack_dm_visible_row_indices[start..end]
            .iter()
            .filter_map(|index| self.slack_dm_rows.get(*index))
            .flat_map(|row| row.participants.iter())
            .filter_map(|participant| participant.avatar_image_url.as_ref())
            .map(|url| url.as_ref())
            .filter(|url| !self.slack_remote_images.contains_key(*url))
            .filter(|url| !self.slack_active_remote_image_urls.contains(*url))
            .filter(|url| queued.insert(url))
            .map(str::to_string)
            .collect::<Vec<_>>();
        self.slack_pending_remote_image_urls
            .extend(urls.into_iter().rev());
        self.start_next_slack_remote_image_load(cx);
    }

    fn maybe_load_more_slack_dms(
        &mut self,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        if visible_end.saturating_add(SLACK_DM_PAGINATION_THRESHOLD) < count
            || self.slack_dm_pagination_cursor.is_some()
        {
            return;
        }
        let Some(cursor) = self
            .slack_dm_inbox_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone())
        else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_dm_pagination_cursor = Some(cursor.clone());
        let requested_cursor = cursor.clone();
        self.spawn_background_task(
            cursor,
            cx,
            move |cursor| {
                workspace_api
                    .load_slack_dm_inbox(Some(cursor.as_str()))
                    .map(prepare_slack_dm_inbox_snapshot)
            },
            move |this, result, cx| {
                if this.slack_dm_pagination_cursor.as_deref() != Some(requested_cursor.as_str()) {
                    return;
                }
                match result {
                    Ok(prepared) => {
                        this.apply_prepared_slack_dm_inbox_snapshot(prepared, true, cx);
                    }
                    Err(error) => {
                        this.slack_dm_pagination_cursor = None;
                        this.slack_error = Some(error);
                        cx.notify();
                    }
                }
            },
        );
    }
}
