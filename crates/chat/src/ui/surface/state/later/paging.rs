use super::{
    append_slack_later_rows, load_slack_later_page, merge_slack_later_snapshots, Context,
    SlackLaterCursor, SlackLaterPageRequest, SlackLaterPreparedPage, SlackLaterRow,
    SlackLaterSnapshot, SlackRailView, SurfaceState, SLACK_LATER_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_later_page(
        &mut self,
        cursor: Option<SlackLaterCursor>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_later_loading || self.slack_active_rail_view != SlackRailView::Later {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_later_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        let Some(team_id) = self.slack_later_team_id.clone() else {
            return;
        };
        let request = SlackLaterPageRequest {
            generation: self.slack_later_generation,
            team_id,
            filter: self.slack_later_filter,
            cursor,
        };
        self.slack_later_loading = true;
        self.slack_later_error = None;
        cx.notify();
        self.spawn_background_task(
            request,
            cx,
            move |request| load_slack_later_page(workspace_api, request),
            move |this, (request, result), cx| {
                this.finish_slack_later_page(request, result, cx);
            },
        );
    }

    fn finish_slack_later_page(
        &mut self,
        request: SlackLaterPageRequest,
        result: SlackLaterPreparedPage,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_later_page_request_is_current(&request) {
            return;
        }
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            self.reset_slack_later_context();
            self.activate_slack_later(cx);
            return;
        }
        self.slack_later_loading = false;
        match result {
            Ok((page, page_rows)) => self.apply_slack_later_page(request, page, page_rows, cx),
            Err(message) => self.slack_later_error = Some(message),
        }
        cx.notify();
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    fn apply_slack_later_page(
        &mut self,
        request: SlackLaterPageRequest,
        page: SlackLaterSnapshot,
        page_rows: Vec<SlackLaterRow>,
        cx: &mut Context<Self>,
    ) {
        if page.filter != request.filter {
            self.slack_later_error = Some("Slack returned a different Later filter.".to_string());
            return;
        }
        if request.cursor.as_ref() == page.next_cursor.as_ref() && page.next_cursor.is_some() {
            self.slack_later_error =
                Some("Slack Later pagination returned the same cursor.".to_string());
            return;
        }
        if request.cursor.is_none() {
            self.slack_later_rows = page_rows;
            self.slack_later_snapshot = Some(page);
            self.slack_later_list_state
                .reset(self.slack_later_rows.len());
        } else {
            self.append_slack_later_page(page, page_rows);
        }
        self.slack_later_error = None;
        if self
            .slack_later_selected_key
            .as_ref()
            .is_none_or(|key| !self.slack_later_rows.iter().any(|row| &row.key == key))
        {
            self.slack_later_selected_key =
                self.slack_later_rows.first().map(|row| row.key.clone());
        }
        self.queue_selected_slack_later_hydration(cx);
        self.queue_slack_later_hydration_range(0, SLACK_LATER_INITIAL_VISIBLE_ROWS, cx);
        self.queue_slack_later_visible_images(0, SLACK_LATER_INITIAL_VISIBLE_ROWS, cx);
        self.sync_slack_later_selected_detail(cx);
    }

    fn append_slack_later_page(&mut self, page: SlackLaterSnapshot, page_rows: Vec<SlackLaterRow>) {
        let old_count = self.slack_later_rows.len();
        append_slack_later_rows(&mut self.slack_later_rows, page_rows);
        self.slack_later_snapshot = Some(merge_slack_later_snapshots(
            self.slack_later_snapshot.take(),
            page,
        ));
        let added = self.slack_later_rows.len().saturating_sub(old_count);
        if added > 0 {
            self.slack_later_list_state
                .splice(old_count..old_count, added);
        }
    }

    fn slack_later_page_request_is_current(&self, request: &SlackLaterPageRequest) -> bool {
        if self.slack_later_generation != request.generation
            || self.slack_active_rail_view != SlackRailView::Later
            || self.slack_later_team_id.as_deref() != Some(request.team_id.as_str())
            || self.slack_later_filter != request.filter
        {
            return false;
        }
        match request.cursor.as_ref() {
            None => self.slack_later_snapshot.is_none(),
            Some(cursor) => {
                self.slack_later_snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.next_cursor.as_ref())
                    == Some(cursor)
            }
        }
    }

    pub(in crate::ui::surface::state) fn maybe_load_more_slack_later(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_later_loading {
            return;
        }
        let Some(cursor) = self
            .slack_later_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone())
        else {
            return;
        };
        self.begin_slack_later_page(Some(cursor), cx);
    }
}
