use super::{
    load_slack_activity_page, next_slack_activity_generation,
    prepare_slack_activity_workspace_context, slack_activity_filter_index, Context,
    PreparedSlackActivitySnapshot, SlackActivityCursor, SlackActivityDetailState,
    SlackActivityFilter, SlackActivityPageRequest, SlackActivityRow, SlackRailView, SurfaceState,
    SLACK_ACTIVITY_IMAGE_VISIBLE_OVERDRAW, SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_activity_page(
        &mut self,
        cursor: Option<SlackActivityCursor>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_loading || self.slack_active_rail_view != SlackRailView::Activity {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_activity_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let team_id = workspace.team_id.clone();
        if self.slack_activity_team_id.as_deref() != Some(team_id.as_str()) {
            return;
        }
        let workspace = prepare_slack_activity_workspace_context(workspace);
        self.slack_activity_generation = next_slack_activity_generation(
            self.slack_activity_generation,
            "Slack Activity request generation overflowed",
        );
        let generation = self.slack_activity_generation;
        let request = SlackActivityPageRequest {
            generation,
            team_id,
            cursor,
            existing: self.slack_activity_snapshot.clone(),
            workspace,
        };
        self.slack_activity_loading = true;
        self.slack_activity_error = None;
        cx.notify();
        self.spawn_background_task(
            request,
            cx,
            move |request| load_slack_activity_page(workspace_api, request),
            move |this, (request, result), cx| {
                this.finish_slack_activity_page(request, result, cx);
            },
        );
    }

    fn finish_slack_activity_page(
        &mut self,
        request: SlackActivityPageRequest,
        result: Result<PreparedSlackActivitySnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_generation != request.generation
            || self.slack_active_rail_view != SlackRailView::Activity
            || self.slack_activity_team_id.as_deref() != Some(request.team_id.as_str())
        {
            return;
        }
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            self.reset_slack_activity_context(cx);
            self.activate_slack_activity(cx);
            return;
        }
        self.slack_activity_loading = false;
        match result {
            Ok(prepared) => {
                self.slack_activity_snapshot = Some(prepared.snapshot);
                self.slack_activity_rows = prepared.rows;
                self.slack_activity_error = None;
                self.refresh_slack_activity_visible_rows(cx);
                self.queue_slack_activity_visible_images(
                    0,
                    SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS,
                    cx,
                );
            }
            Err(message) => {
                self.slack_activity_error = Some(message);
            }
        }
        cx.notify();
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    pub(in crate::ui::surface::state) fn maybe_load_more_slack_activity(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_loading {
            return;
        }
        let Some(cursor) = self
            .slack_activity_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone())
        else {
            return;
        };
        self.begin_slack_activity_page(Some(cursor), cx);
    }

    pub(in crate::ui::surface::state) fn refresh_slack_activity_visible_rows(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let mut filter_counts = [0; 4];
        for row in self.slack_activity_rows.iter().filter(|row| row.unread) {
            filter_counts[slack_activity_filter_index(SlackActivityFilter::All)] += 1;
            for filter in [
                SlackActivityFilter::Dms,
                SlackActivityFilter::Mentions,
                SlackActivityFilter::Threads,
            ] {
                if filter.includes(row.kind) {
                    filter_counts[slack_activity_filter_index(filter)] += 1;
                }
            }
        }
        self.slack_activity_filter_counts = filter_counts;
        self.slack_activity_visible_row_indices = self
            .slack_activity_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                self.slack_activity_filter.includes(row.kind)
                    && (!self.slack_activity_unread_only || row.unread)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        let next_count = self.slack_activity_visible_row_indices.len();
        if self.slack_activity_list_state.item_count() != next_count {
            self.slack_activity_list_state.reset(next_count);
        } else {
            self.slack_activity_list_state.remeasure();
        }
        let selected_visible =
            self.slack_activity_selected_key
                .as_deref()
                .is_some_and(|selected_key| {
                    self.slack_activity_visible_rows()
                        .any(|row| row.key.as_ref() == selected_key)
                });
        if !selected_visible {
            self.restore_slack_routed_main_composer(cx);
            self.slack_activity_selected_key = None;
            self.slack_activity_detail_generation = next_slack_activity_generation(
                self.slack_activity_detail_generation,
                "Slack Activity detail generation overflowed",
            );
            self.slack_activity_detail = SlackActivityDetailState::Empty;
            self.slack_activity_detail_list_state.reset(0);
            self.slack_activity_local_delivery_rows = Default::default();
        }
    }

    pub(in crate::ui::surface::state) fn slack_activity_visible_rows(
        &self,
    ) -> impl Iterator<Item = &SlackActivityRow> {
        self.slack_activity_visible_row_indices
            .iter()
            .filter_map(|index| self.slack_activity_rows.get(*index))
    }

    pub(in crate::ui::surface::state) fn queue_slack_activity_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.saturating_sub(SLACK_ACTIVITY_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_ACTIVITY_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_activity_visible_row_indices.len());
        let urls = self.slack_activity_visible_row_indices[start..end]
            .iter()
            .filter_map(|index| self.slack_activity_rows.get(*index))
            .filter_map(|row| row.avatar_image_url.as_deref())
            .map(str::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
