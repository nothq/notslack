use super::{
    prepare_slack_files_snapshot, spawn_background_task_for_entity, Arc, Context, HashSet,
    PreparedSlackFilesSnapshot, SlackFilesPageRequest, SlackFilesRequest, SlackRailView,
    SurfaceState, WorkspaceApi,
};

type SlackFilesPageWork = (Arc<dyn WorkspaceApi>, SlackFilesPageRequest);

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_files_page(
        &mut self,
        page: u32,
        cx: &mut Context<Self>,
    ) {
        if self.slack_files_loading {
            return;
        }
        let (workspace_api, request) = match self.slack_files_page_request(page) {
            Ok(request) => request,
            Err(message) => {
                self.slack_files_error = Some(message);
                cx.notify();
                return;
            }
        };
        self.slack_files_loading = true;
        self.slack_files_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackFilesPageRequest)| {
                let result = workspace_api
                    .load_slack_files(request.request.clone())
                    .and_then(|snapshot| {
                        prepare_slack_files_snapshot(
                            snapshot,
                            &request.request.self_user_id,
                            request.timezone,
                        )
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_files_page(request, result, cx);
            },
        );
    }

    fn slack_files_page_request(&self, page: u32) -> Result<SlackFilesPageWork, String> {
        let workspace_api = self
            .active_slack_workspace_api()
            .ok_or_else(|| "Slack Files requires a connected workspace.".to_string())?;
        let team_id = self
            .slack_files_team_id
            .clone()
            .ok_or_else(|| "Slack Files requires a workspace team id.".to_string())?;
        let self_user_id = self
            .slack_files_self_user_id
            .clone()
            .ok_or_else(|| "Slack Files requires the current user id.".to_string())?;
        let browser_session_id = self
            .slack_files_browser_session_id
            .clone()
            .ok_or_else(|| "Slack Files browser session is unavailable.".to_string())?;
        let timezone = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_timezone_id.as_deref())
            .and_then(|timezone| timezone.parse::<chrono_tz::Tz>().ok())
            .unwrap_or(chrono_tz::UTC);
        let request = SlackFilesPageRequest {
            generation: self.slack_files_generation,
            request: SlackFilesRequest {
                team_id,
                self_user_id,
                browser_session_id,
                search_query: self.slack_files_committed_query.clone(),
                scope: self.slack_files_scope,
                type_filters: self.slack_files_type_filters.clone(),
                sort: self.slack_files_sort,
                page,
            },
            timezone,
        };
        Ok((workspace_api, request))
    }

    fn finish_slack_files_page(
        &mut self,
        request: SlackFilesPageRequest,
        result: Result<PreparedSlackFilesSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        let current_request = self.slack_files_page_request_is_current(&request);
        if !current_request {
            self.slack_files_loading = false;
            if self.slack_active_rail_view == SlackRailView::Files
                && self.slack_files_snapshot.is_none()
            {
                self.begin_slack_files_page(1, cx);
            } else {
                cx.notify();
            }
            self.ensure_slack_realtime_pending_refreshes(cx);
            return;
        }
        self.slack_files_loading = false;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_files_error = Some(error);
                cx.notify();
                self.ensure_slack_realtime_pending_refreshes(cx);
                return;
            }
        };
        if request.request.page == 1 {
            self.slack_files_rows = prepared.rows;
            self.slack_files_scroll_handle = gpui::UniformListScrollHandle::new();
        } else {
            let existing_ids = self
                .slack_files_rows
                .iter()
                .map(|row| row.id.clone())
                .collect::<HashSet<_>>();
            let appended = prepared
                .rows
                .iter()
                .filter(|row| !existing_ids.contains(&row.id))
                .cloned()
                .collect::<Vec<_>>();
            let old_count = self.slack_files_rows.len();
            let mut rows = Vec::with_capacity(old_count + appended.len());
            rows.extend(self.slack_files_rows.iter().cloned());
            rows.extend(appended);
            self.slack_files_rows = rows.into();
        }
        self.slack_files_snapshot = Some(prepared.snapshot);
        self.slack_files_error = None;
        cx.notify();
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    fn slack_files_page_request_is_current(&self, request: &SlackFilesPageRequest) -> bool {
        self.slack_active_rail_view == SlackRailView::Files
            && self.slack_files_generation == request.generation
            && self.slack_files_team_id.as_deref() == Some(request.request.team_id.as_str())
            && self.slack_files_self_user_id.as_deref()
                == Some(request.request.self_user_id.as_str())
            && self.slack_files_browser_session_id.as_ref()
                == Some(&request.request.browser_session_id)
    }
}
