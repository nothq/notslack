use std::sync::Arc;

use super::rows::{
    append_slack_drafts_sent_rows, merge_slack_drafts_sent_snapshots, slack_drafts_sent_tab_index,
};
use super::{Context, SlackDraftsSentTab, SlackRailView, SurfaceState};
use crate::ui::surface::{prepare_slack_drafts_sent_snapshot, PreparedSlackDraftsSentSnapshot};
use crate::ui::{
    SlackDraftsSentCursor, SlackDraftsSentRequest, SlackDraftsSentSnapshot, WorkspaceApi,
};

const SLACK_DRAFTS_SENT_INITIAL_IMAGE_ROWS: usize = 12;

#[derive(Clone)]
struct SlackDraftsSentPageRequest {
    generation: u64,
    request: SlackDraftsSentRequest,
    timezone: chrono_tz::Tz,
    authenticated_self_user_id: String,
}

type SlackDraftsSentPageResult = Result<PreparedSlackDraftsSentSnapshot, String>;

impl SurfaceState {
    pub(crate) fn compose_new_slack_message(&mut self, cx: &mut Context<Self>) {
        self.activate_slack_new_message(cx);
    }

    pub(super) fn show_cached_slack_drafts_sent_tab(&mut self) {
        self.slack_drafts_sent_rows = self.slack_drafts_sent_cached_rows
            [slack_drafts_sent_tab_index(self.slack_drafts_sent_tab)]
        .clone();
        self.slack_drafts_sent_list_state
            .reset(self.slack_drafts_sent_rows.len());
    }

    pub(super) fn current_slack_drafts_sent_snapshot(&self) -> Option<&SlackDraftsSentSnapshot> {
        self.slack_drafts_sent_snapshots[slack_drafts_sent_tab_index(self.slack_drafts_sent_tab)]
            .as_ref()
    }

    pub(super) fn refresh_current_slack_drafts_sent_tab(&mut self, cx: &mut Context<Self>) {
        self.queue_slack_drafts_sent_file_metadata(0, SLACK_DRAFTS_SENT_INITIAL_IMAGE_ROWS, cx);
        self.queue_slack_drafts_sent_images(0, SLACK_DRAFTS_SENT_INITIAL_IMAGE_ROWS, cx);
        if self.slack_drafts_sent_loading {
            return;
        }
        let index = slack_drafts_sent_tab_index(self.slack_drafts_sent_tab);
        self.slack_drafts_sent_snapshots[index] = None;
        self.begin_slack_drafts_sent_page(None, cx);
    }

    pub(super) fn begin_slack_drafts_sent_page(
        &mut self,
        cursor: Option<SlackDraftsSentCursor>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_drafts_sent_loading
            || self.slack_active_rail_view != SlackRailView::DraftsSent
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_drafts_sent_error =
                Some("Slack Drafts & sent requires a connected workspace.".to_string());
            cx.notify();
            return;
        };
        let Some(team_id) = self.slack_drafts_sent_team_id.clone() else {
            return;
        };
        let Some(authenticated_self_user_id) = self.slack_drafts_sent_self_user_id.clone() else {
            return;
        };
        let Some(timezone) =
            self.slack_drafts_sent_timezone(&team_id, &authenticated_self_user_id, cx)
        else {
            return;
        };
        let request = SlackDraftsSentPageRequest {
            generation: self.slack_drafts_sent_generation,
            request: SlackDraftsSentRequest {
                team_id,
                tab: self.slack_drafts_sent_tab,
                cursor,
            },
            timezone,
            authenticated_self_user_id,
        };
        self.spawn_slack_drafts_sent_page(workspace_api, request, cx);
    }

    fn slack_drafts_sent_timezone(
        &mut self,
        team_id: &str,
        authenticated_self_user_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<chrono_tz::Tz> {
        let workspace = self.slack_workspace()?;
        if workspace.team_id != team_id
            || workspace.self_user_id.as_deref() != Some(authenticated_self_user_id)
        {
            return None;
        }
        let Some(timezone_id) = workspace.self_timezone_id.as_deref() else {
            self.slack_drafts_sent_error =
                Some("Slack Drafts & sent requires the workspace time zone.".to_string());
            cx.notify();
            return None;
        };
        let Ok(timezone) = timezone_id.parse::<chrono_tz::Tz>() else {
            self.slack_drafts_sent_error = Some(format!(
                "Slack returned an invalid workspace time zone: {timezone_id}."
            ));
            cx.notify();
            return None;
        };
        Some(timezone)
    }

    fn spawn_slack_drafts_sent_page(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackDraftsSentPageRequest,
        cx: &mut Context<Self>,
    ) {
        self.slack_drafts_sent_loading = true;
        self.slack_drafts_sent_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackDraftsSentPageRequest)| {
                let result = workspace_api
                    .load_slack_drafts_sent(request.request.clone())
                    .and_then(|snapshot| {
                        prepare_slack_drafts_sent_snapshot(
                            snapshot,
                            request.timezone,
                            &request.authenticated_self_user_id,
                        )
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_drafts_sent_page(request, result, cx);
            },
        );
    }

    fn finish_slack_drafts_sent_page(
        &mut self,
        request: SlackDraftsSentPageRequest,
        result: SlackDraftsSentPageResult,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_drafts_sent_request_is_current(&request) {
            return;
        }
        self.slack_drafts_sent_loading = false;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_drafts_sent_error = Some(error);
                cx.notify();
                return;
            }
        };
        if prepared.snapshot.team_id != request.request.team_id
            || prepared.snapshot.tab != request.request.tab
        {
            self.slack_drafts_sent_error =
                Some("Slack returned a different Drafts & sent workspace or tab.".to_string());
            cx.notify();
            return;
        }
        let old_row_count = self.apply_slack_drafts_sent_page_rows(&request, prepared);
        self.slack_drafts_sent_error = None;
        self.queue_slack_drafts_sent_file_metadata(0, SLACK_DRAFTS_SENT_INITIAL_IMAGE_ROWS, cx);
        self.queue_slack_drafts_sent_images(0, SLACK_DRAFTS_SENT_INITIAL_IMAGE_ROWS, cx);
        let should_continue_empty_page = self.slack_drafts_sent_rows.len() == old_row_count
            && self
                .current_slack_drafts_sent_snapshot()
                .and_then(|snapshot| snapshot.next_cursor.as_ref())
                .is_some();
        if request.request.tab == SlackDraftsSentTab::Scheduled {
            self.resume_protected_slack_schedule_reconciliation(cx);
        }
        cx.notify();
        if should_continue_empty_page {
            self.maybe_load_more_slack_drafts_sent(cx);
        }
    }

    fn apply_slack_drafts_sent_page_rows(
        &mut self,
        request: &SlackDraftsSentPageRequest,
        prepared: PreparedSlackDraftsSentSnapshot,
    ) -> usize {
        let index = slack_drafts_sent_tab_index(request.request.tab);
        let old_row_count = self.slack_drafts_sent_cached_rows[index].len();
        if request.request.cursor.is_none() {
            self.slack_drafts_sent_snapshots[index] = Some(prepared.snapshot);
            self.slack_drafts_sent_cached_rows[index] = prepared.rows;
        } else {
            self.slack_drafts_sent_snapshots[index] = Some(merge_slack_drafts_sent_snapshots(
                self.slack_drafts_sent_snapshots[index].take(),
                prepared.snapshot,
            ));
            self.slack_drafts_sent_cached_rows[index] = append_slack_drafts_sent_rows(
                &self.slack_drafts_sent_cached_rows[index],
                &prepared.rows,
            );
        }
        self.slack_drafts_sent_rows = self.slack_drafts_sent_cached_rows[index].clone();
        if request.request.cursor.is_none() {
            self.slack_drafts_sent_list_state
                .reset(self.slack_drafts_sent_rows.len());
        } else {
            let added = self
                .slack_drafts_sent_rows
                .len()
                .saturating_sub(old_row_count);
            if added > 0 {
                self.slack_drafts_sent_list_state
                    .splice(old_row_count..old_row_count, added);
            }
        }
        old_row_count
    }

    pub(in crate::ui::surface::state) fn apply_authoritative_slack_scheduled_snapshot(
        &mut self,
        prepared: PreparedSlackDraftsSentSnapshot,
    ) -> Result<(), String> {
        if prepared.snapshot.tab != SlackDraftsSentTab::Scheduled {
            return Err("Slack scheduled refresh returned a different tab.".to_string());
        }
        if self.slack_drafts_sent_team_id.as_deref() != Some(prepared.snapshot.team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref()
                != Some(prepared.authenticated_self_user_id.as_str())
        {
            return Err(
                "Slack scheduled refresh returned a different workspace identity.".to_string(),
            );
        }
        let index = slack_drafts_sent_tab_index(SlackDraftsSentTab::Scheduled);
        self.slack_drafts_sent_snapshots[index] = Some(prepared.snapshot);
        self.slack_drafts_sent_cached_rows[index] = prepared.rows;
        self.slack_drafts_sent_error = None;
        if self.slack_active_rail_view == SlackRailView::DraftsSent
            && self.slack_drafts_sent_tab == SlackDraftsSentTab::Scheduled
        {
            self.slack_drafts_sent_rows = self.slack_drafts_sent_cached_rows[index].clone();
            self.slack_drafts_sent_list_state
                .reset(self.slack_drafts_sent_rows.len());
        }
        Ok(())
    }

    pub(in crate::ui::surface::state) fn invalidate_slack_scheduled_snapshot(&mut self) {
        let index = slack_drafts_sent_tab_index(SlackDraftsSentTab::Scheduled);
        self.slack_drafts_sent_snapshots[index] = None;
        self.slack_drafts_sent_cached_rows[index] = Arc::default();
        if self.slack_active_rail_view == SlackRailView::DraftsSent
            && self.slack_drafts_sent_tab == SlackDraftsSentTab::Scheduled
        {
            self.slack_drafts_sent_rows = Arc::default();
            self.slack_drafts_sent_list_state.reset(0);
        }
    }

    fn slack_drafts_sent_request_is_current(&self, request: &SlackDraftsSentPageRequest) -> bool {
        if self.slack_active_rail_view != SlackRailView::DraftsSent
            || self.slack_drafts_sent_generation != request.generation
            || self.slack_drafts_sent_team_id.as_deref() != Some(request.request.team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref()
                != Some(request.authenticated_self_user_id.as_str())
            || self.slack_drafts_sent_tab != request.request.tab
        {
            return false;
        }
        match request.request.cursor.as_ref() {
            None => self.current_slack_drafts_sent_snapshot().is_none(),
            Some(cursor) => {
                self.current_slack_drafts_sent_snapshot()
                    .and_then(|snapshot| snapshot.next_cursor.as_ref())
                    == Some(cursor)
            }
        }
    }

    pub(super) fn maybe_load_more_slack_drafts_sent(&mut self, cx: &mut Context<Self>) {
        if self.slack_drafts_sent_loading {
            return;
        }
        let Some(cursor) = self
            .current_slack_drafts_sent_snapshot()
            .and_then(|snapshot| snapshot.next_cursor.clone())
        else {
            return;
        };
        self.begin_slack_drafts_sent_page(Some(cursor), cx);
    }
}
