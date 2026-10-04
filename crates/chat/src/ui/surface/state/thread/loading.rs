use super::{
    merge_slack_thread_rows, normalize_slack_thread_parent_row, prepare_slack_thread_snapshot, px,
    refresh_slack_thread_list_rows, slack_thread_row_remote_image_urls, Arc, Context, ListOffset,
    PreparedSlackThreadSnapshot, Range, SlackMessageTimestamp, SlackRailView, SlackThreadListRow,
    SlackThreadPanelOrigin, SlackThreadPanelState, SlackThreadRequest, SurfaceState, WorkspaceApi,
    SLACK_THREAD_IMAGE_LEADING_ROWS, SLACK_THREAD_IMAGE_PREFETCH_URL_LIMIT,
    SLACK_THREAD_IMAGE_TRAILING_ROWS, SLACK_THREAD_INITIAL_IMAGE_ROW_LIMIT,
    SLACK_THREAD_PAGINATION_THRESHOLD,
};
use crate::ui::{SlackThreadLoad, SlackThreadReadMetadata};

type PreparedSlackThreadLoad = (PreparedSlackThreadSnapshot, Option<SlackThreadReadMetadata>);

impl SurfaceState {
    pub(crate) fn retry_slack_thread_load(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.slack_thread_panel.as_ref() else {
            return;
        };
        if panel.loading || panel.error.is_none() {
            return;
        }
        self.begin_slack_thread_load(panel.next_cursor.clone(), cx);
    }

    pub(crate) fn handle_slack_thread_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        row_count: usize,
        cx: &mut Context<Self>,
    ) {
        let list_row_count = self
            .slack_thread_panel
            .as_ref()
            .map_or(0, |panel| panel.list_rows.len());
        self.prefetch_slack_thread_images_for_range(
            visible_start.saturating_sub(SLACK_THREAD_IMAGE_LEADING_ROWS)
                ..visible_end
                    .saturating_add(SLACK_THREAD_IMAGE_TRAILING_ROWS)
                    .min(list_row_count),
            cx,
        );
        self.queue_slack_thread_read_for_visible_range(visible_start, visible_end, cx);
        let next_cursor = self.slack_thread_panel.as_ref().and_then(|panel| {
            (!panel.loading
                && visible_end.saturating_add(SLACK_THREAD_PAGINATION_THRESHOLD) >= row_count)
                .then(|| panel.next_cursor.clone())
                .flatten()
        });
        if let Some(cursor) = next_cursor {
            self.begin_slack_thread_load(Some(cursor), cx);
        }
    }

    pub(in crate::ui::surface::state) fn begin_slack_thread_load(
        &mut self,
        cursor: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.begin_slack_thread_request(cursor, cx);
    }

    pub(in crate::ui::surface::state) fn begin_slack_thread_request(
        &mut self,
        cursor: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.begin_slack_thread_request_with_mode(cursor, false, cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_thread_realtime_refresh(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.slack_thread_panel.as_ref() else {
            self.slack_realtime_thread_refresh_pending = false;
            return;
        };
        if panel.loading {
            self.slack_realtime_thread_refresh_pending = true;
            return;
        }
        self.slack_realtime_thread_refresh_pending = false;
        self.begin_slack_thread_request_with_mode(None, true, cx);
    }

    fn begin_slack_thread_request_with_mode(
        &mut self,
        cursor: Option<String>,
        replace_existing: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.slack_thread_panel.as_ref() else {
            return;
        };
        if panel.loading {
            return;
        }
        let thread_timestamp = match SlackMessageTimestamp::parse(&panel.parent_message_id) {
            Ok(thread_timestamp) => thread_timestamp,
            Err(error) => {
                self.fail_slack_thread_load(error, cx);
                return;
            }
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.fail_slack_thread_load(
                "Slack thread loading requires a connected workspace".to_string(),
                cx,
            );
            return;
        };
        let request = SlackThreadRequest {
            generation: panel.generation,
            team_id: self
                .slack_workspace()
                .expect("Slack thread panel requires a workspace")
                .team_id
                .clone(),
            conversation_id: panel.conversation_id.clone(),
            parent_message_id: panel.parent_message_id.clone(),
            thread_timestamp,
            cursor,
            replace_existing,
        };
        let show_composer = self.slack_workspace_api_capabilities.send_thread_reply;
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("Slack thread panel disappeared before its request started");
        panel.loading = true;
        panel.error = None;
        refresh_slack_thread_list_rows(panel, show_composer);
        cx.notify();
        self.spawn_slack_thread_request(workspace_api, request, cx);
    }

    fn spawn_slack_thread_request(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        request: SlackThreadRequest,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackThreadRequest)| {
                let result = workspace_api
                    .load_slack_thread_with_read_metadata(
                        &request.conversation_id,
                        &request.thread_timestamp,
                        request.cursor.as_deref(),
                    )
                    .map(
                        |SlackThreadLoad {
                             snapshot,
                             read_metadata,
                         }| {
                            (prepare_slack_thread_snapshot(snapshot), read_metadata)
                        },
                    );
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_thread_load(request, result, cx);
            },
        );
    }

    fn finish_slack_thread_load(
        &mut self,
        request: SlackThreadRequest,
        result: Result<PreparedSlackThreadLoad, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_thread_request_is_current(&request) {
            return;
        }
        let (prepared, read_metadata) = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_slack_thread_load(error, cx);
                return;
            }
        };
        if prepared.snapshot.team_id != request.team_id
            || prepared.snapshot.conversation_id != request.conversation_id
            || prepared.snapshot.thread_timestamp != request.parent_message_id
        {
            self.fail_slack_thread_load(
                "Slack thread response did not match the requested conversation and parent"
                    .to_string(),
                cx,
            );
            return;
        }
        let prefetch_range = self.install_prepared_slack_thread_load(&request, prepared);
        self.install_slack_thread_read_load(&request, read_metadata);
        self.prefetch_slack_thread_images_for_range(prefetch_range, cx);
        self.continue_slack_message_navigation(cx);
        cx.notify();
        if self.slack_realtime_thread_refresh_pending {
            self.queue_slack_thread_realtime_refresh(cx);
        }
    }

    fn install_prepared_slack_thread_load(
        &mut self,
        request: &SlackThreadRequest,
        prepared: PreparedSlackThreadSnapshot,
    ) -> Range<usize> {
        let expected_reply_count = prepared.parent_row.as_ref().and_then(|row| row.reply_count);
        let PreparedSlackThreadSnapshot {
            snapshot,
            timezone,
            parent_row,
            reply_rows,
        } = prepared;
        let show_composer = self.slack_workspace_api_capabilities.send_thread_reply;
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("current Slack thread panel disappeared while applying a response");
        panel.loading = false;
        panel.error = None;
        panel.conversation_name = snapshot.conversation_name;
        panel.timezone = timezone;
        panel.next_cursor = snapshot.next_cursor;
        panel.pagination_initialized = true;
        if let Some(mut parent_row) = parent_row {
            normalize_slack_thread_parent_row(&mut parent_row);
            panel.parent_row = parent_row;
            panel.parent_hydrated = true;
        }
        if let Some(expected_reply_count) = expected_reply_count {
            panel.expected_reply_count = expected_reply_count.max(panel.expected_reply_count);
        }
        let old_rows = panel.reply_rows.clone();
        let merged_rows = if request.replace_existing {
            merge_slack_thread_rows(&[], &reply_rows, panel.timezone)
        } else {
            merge_slack_thread_rows(&old_rows, &reply_rows, panel.timezone)
        };
        panel.reply_rows = merged_rows.into();
        panel.expected_reply_count = panel
            .expected_reply_count
            .max(u32::try_from(panel.reply_rows.len()).unwrap_or(u32::MAX));
        refresh_slack_thread_list_rows(panel, show_composer);
        slack_thread_install_prefetch_range(panel, request)
    }

    fn slack_thread_request_is_current(&self, request: &SlackThreadRequest) -> bool {
        self.slack_workspace()
            .is_some_and(|workspace| workspace.team_id == request.team_id)
            && self.slack_thread_panel.as_ref().is_some_and(|panel| {
                panel.generation == request.generation
                    && panel.conversation_id == request.conversation_id
                    && panel.parent_message_id == request.parent_message_id
                    && self.slack_thread_panel_origin_is_current(panel)
            })
    }

    pub(in crate::ui::surface::state) fn slack_thread_panel_origin_is_current(
        &self,
        panel: &SlackThreadPanelState,
    ) -> bool {
        match &panel.origin {
            SlackThreadPanelOrigin::Conversation => {
                self.slack_workspace().is_some_and(|workspace| {
                    workspace.conversation_id == panel.conversation_id
                        && self
                            .slack_pending_conversation_id
                            .as_deref()
                            .is_none_or(|pending| pending == panel.conversation_id)
                })
            }
            SlackThreadPanelOrigin::Later { item_key, .. } => {
                self.slack_active_rail_view == SlackRailView::Later
                    && self.slack_later_selected_key.as_ref() == Some(item_key)
            }
        }
    }

    pub(in crate::ui::surface::state) fn fail_slack_thread_load(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let show_composer = self.slack_workspace_api_capabilities.send_thread_reply;
        let Some(panel) = self.slack_thread_panel.as_mut() else {
            return;
        };
        panel.loading = false;
        panel.error = Some(error);
        refresh_slack_thread_list_rows(panel, show_composer);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn prefetch_slack_thread_images_for_range(
        &mut self,
        row_range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        let mut urls = Vec::new();
        if let Some(panel) = self.slack_thread_panel.as_ref() {
            for list_row in panel.list_rows.get(row_range).unwrap_or_default().iter() {
                let message_row = match list_row {
                    SlackThreadListRow::Parent => Some(&panel.parent_row),
                    SlackThreadListRow::Reply(reply_index) => panel.reply_rows.get(*reply_index),
                    SlackThreadListRow::ReplyDivider
                    | SlackThreadListRow::Loading
                    | SlackThreadListRow::Error
                    | SlackThreadListRow::Composer => None,
                };
                if let Some(message_row) = message_row {
                    for url in slack_thread_row_remote_image_urls(message_row) {
                        if urls.len() == SLACK_THREAD_IMAGE_PREFETCH_URL_LIMIT {
                            break;
                        }
                        urls.push(url);
                    }
                }
                if urls.len() == SLACK_THREAD_IMAGE_PREFETCH_URL_LIMIT {
                    break;
                }
            }
        }
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}

fn slack_thread_install_prefetch_range(
    panel: &SlackThreadPanelState,
    request: &SlackThreadRequest,
) -> Range<usize> {
    let selected_index = request
        .cursor
        .is_none()
        .then(|| {
            panel
                .origin
                .later_selected_message_id()
                .and_then(|selected_message_id| {
                    panel
                        .reply_rows
                        .iter()
                        .position(|row| row.id == selected_message_id)
                })
        })
        .flatten();
    if let Some(selected_index) = selected_index {
        panel.list_state.scroll_to(ListOffset {
            item_ix: selected_index,
            offset_in_item: px(0.0),
        });
    }
    let prefetch_start = selected_index
        .unwrap_or_else(|| panel.list_state.logical_scroll_top().item_ix)
        .min(panel.list_rows.len());
    prefetch_start
        ..prefetch_start
            .saturating_add(SLACK_THREAD_INITIAL_IMAGE_ROW_LIMIT)
            .min(panel.list_rows.len())
}
