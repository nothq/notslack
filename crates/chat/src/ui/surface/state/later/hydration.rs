use super::{
    collect_slack_later_row_urls, hydrate_slack_later_row, mark_slack_later_hydration_error,
    Context, SlackLaterHydrationRequest, SlackLaterRow, SlackRailView, SurfaceState,
    SLACK_LATER_IMAGE_VISIBLE_OVERDRAW, SLACK_LATER_MAX_HYDRATIONS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn queue_slack_later_hydration_range(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.saturating_sub(SLACK_LATER_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_LATER_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_later_rows.len());
        let targets = self.slack_later_rows[start..end]
            .iter()
            .filter_map(SlackLaterRow::pending_hydration_target)
            .cloned()
            .collect::<Vec<_>>();
        for target in targets {
            self.slack_later_hydration_queue.enqueue(target, false);
        }
        self.dispatch_slack_later_hydrations(cx);
    }

    pub(in crate::ui::surface::state) fn queue_selected_slack_later_hydration(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .selected_slack_later_row()
            .and_then(SlackLaterRow::retry_hydration_target)
            .cloned();
        if let Some(target) = target {
            self.slack_later_hydration_queue.enqueue(target, true);
            self.dispatch_slack_later_hydrations(cx);
        }
    }

    pub(in crate::ui::surface::state) fn dispatch_slack_later_hydrations(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let Some(team_id) = self.slack_later_team_id.clone() else {
            return;
        };
        while let Some(target) = self
            .slack_later_hydration_queue
            .start_next(SLACK_LATER_MAX_HYDRATIONS)
        {
            let request = SlackLaterHydrationRequest {
                generation: self.slack_later_generation,
                team_id: team_id.clone(),
                filter: self.slack_later_filter,
                target,
            };
            let request_api = workspace_api.clone();
            self.spawn_background_task(
                request,
                cx,
                move |request| hydrate_slack_later_row(request_api, request),
                move |this, (request, result), cx| {
                    this.finish_slack_later_hydration(request, result, cx);
                },
            );
        }
    }

    fn finish_slack_later_hydration(
        &mut self,
        request: SlackLaterHydrationRequest,
        result: Result<SlackLaterRow, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_later_hydration_request_is_current(&request) {
            return;
        }
        self.slack_later_hydration_queue
            .finish(request.target.key());
        let row_index = self
            .slack_later_rows
            .iter()
            .position(|row| row.key == *request.target.key());
        if let Some(row_index) = row_index {
            match result {
                Ok(row) if row.key == *request.target.key() => {
                    let urls = collect_slack_later_row_urls(std::slice::from_ref(&row));
                    self.slack_later_rows[row_index] = row;
                    for url in urls {
                        self.enqueue_slack_remote_image_url(url, cx);
                    }
                }
                Ok(_) => {
                    mark_slack_later_hydration_error(
                        &mut self.slack_later_rows[row_index],
                        "Slack returned a different saved item.".to_string(),
                    );
                }
                Err(error) => {
                    mark_slack_later_hydration_error(&mut self.slack_later_rows[row_index], error);
                }
            }
        }
        self.sync_slack_later_selected_detail(cx);
        self.dispatch_slack_later_hydrations(cx);
        cx.notify();
    }

    fn slack_later_hydration_request_is_current(
        &self,
        request: &SlackLaterHydrationRequest,
    ) -> bool {
        self.slack_later_generation == request.generation
            && self.slack_active_rail_view == SlackRailView::Later
            && self.slack_later_team_id.as_deref() == Some(request.team_id.as_str())
            && self.slack_later_filter == request.filter
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.team_id == request.team_id)
    }

    pub(in crate::ui::surface::state) fn sync_slack_later_selected_detail(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .selected_slack_later_row()
            .and_then(SlackLaterRow::thread_target);
        match target {
            Some(target)
                if self.slack_thread_panel.as_ref().is_some_and(|panel| {
                    panel.origin.later_item_key() == Some(&target.item_key)
                        && panel.conversation_id == target.conversation_id
                        && panel.parent_message_id == target.thread_timestamp
                        && panel.origin.later_selected_message_id()
                            == Some(target.selected_message_id.as_ref())
                }) => {}
            Some(target) => {
                let target = target.clone();
                self.open_slack_later_thread_detail(target, cx);
            }
            None => self.reset_slack_later_thread_context(),
        }
    }

    pub(in crate::ui::surface::state) fn queue_slack_later_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.saturating_sub(SLACK_LATER_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_LATER_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_later_rows.len());
        let urls = collect_slack_later_row_urls(&self.slack_later_rows[start..end]);
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub(in crate::ui::surface::state) fn queue_selected_slack_later_images(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let urls = self
            .selected_slack_later_row()
            .map(|row| collect_slack_later_row_urls(std::slice::from_ref(row)))
            .unwrap_or_default();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
