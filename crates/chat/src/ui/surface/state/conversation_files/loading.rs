use super::{
    prepare_slack_conversation_files_snapshot, spawn_background_task_for_entity, Arc, Context,
    HashSet, PreparedSlackConversationFilesSnapshot, SlackConversationFilesExplorerRow,
    SlackConversationFilesLoad, SlackConversationFilesRequest, SlackMainTab, SurfaceState,
    WorkspaceApi, SLACK_CONVERSATION_FILES_INITIAL_VISIBLE_HEIGHT,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_conversation_files_load(
        &mut self,
        files_page: Option<u32>,
        media_page: Option<u32>,
        links_page: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_conversation_files_error =
                Some("Files & links requires a connected Slack workspace.".to_string());
            cx.notify();
            return;
        };
        if self.slack_conversation_files_loading {
            return;
        }
        let Some(load) = self.slack_conversation_files_load(files_page, media_page, links_page)
        else {
            return;
        };
        self.slack_conversation_files_loading = true;
        self.slack_conversation_files_error = None;
        let reset_list = self.slack_conversation_files_snapshot.is_none()
            || !self
                .slack_conversation_files_cache
                .loaded(self.slack_conversation_files_filter);
        self.rebuild_slack_conversation_files_explorer(reset_list);
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, load),
            cx,
            |(workspace_api, load): (Arc<dyn WorkspaceApi>, SlackConversationFilesLoad)| {
                let result = workspace_api
                    .load_slack_conversation_files(load.request.clone())
                    .and_then(|snapshot| {
                        prepare_slack_conversation_files_snapshot(
                            snapshot,
                            load.cache.clone(),
                            load.timezone,
                        )
                    });
                (load, result)
            },
            |this, (load, result), cx| {
                this.finish_slack_conversation_files_load(load, result, cx);
            },
        );
    }

    fn slack_conversation_files_load(
        &self,
        files_page: Option<u32>,
        media_page: Option<u32>,
        links_page: Option<u32>,
    ) -> Option<SlackConversationFilesLoad> {
        let timezone = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_timezone_id.as_deref())
            .and_then(|timezone| timezone.parse::<chrono_tz::Tz>().ok())
            .unwrap_or(chrono_tz::UTC);
        Some(SlackConversationFilesLoad {
            generation: self.slack_conversation_files_generation,
            request: SlackConversationFilesRequest {
                team_id: self.slack_conversation_files_team_id.clone()?,
                conversation_id: self.slack_conversation_files_conversation_id.clone()?,
                browser_session_id: self.slack_conversation_files_browser_session_id.clone()?,
                search_query: self.slack_conversation_files_committed_query.clone(),
                sort: self.slack_conversation_files_sort,
                files_page,
                media_page,
                links_page,
            },
            timezone,
            cache: self.slack_conversation_files_cache.clone(),
        })
    }

    fn finish_slack_conversation_files_load(
        &mut self,
        load: SlackConversationFilesLoad,
        result: Result<PreparedSlackConversationFilesSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        self.slack_conversation_files_loading = false;
        let current = self.slack_conversation_files_generation == load.generation
            && self.slack_conversation_files_team_id.as_deref()
                == Some(load.request.team_id.as_str())
            && self.slack_conversation_files_conversation_id.as_deref()
                == Some(load.request.conversation_id.as_str())
            && self.slack_conversation_files_committed_query == load.request.search_query
            && self.slack_conversation_files_sort == load.request.sort;
        if !current {
            if self.slack_conversation_files_reload_pending
                && self.slack_active_tab == SlackMainTab::FilesLinks
            {
                self.start_pending_slack_conversation_files_reload(cx);
            } else {
                cx.notify();
            }
            return;
        }
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_conversation_files_error = Some(error);
                self.rebuild_slack_conversation_files_explorer(false);
                cx.notify();
                return;
            }
        };
        if prepared.snapshot.team_id != load.request.team_id
            || prepared.snapshot.conversation_id != load.request.conversation_id
            || prepared.snapshot.search_query != load.request.search_query
            || prepared.snapshot.sort != load.request.sort
        {
            self.slack_conversation_files_error =
                Some("Slack returned files for a different conversation query.".to_string());
            self.rebuild_slack_conversation_files_explorer(false);
            cx.notify();
            return;
        }
        self.slack_conversation_files_snapshot = Some(prepared.snapshot);
        self.slack_conversation_files_cache = prepared.cache;
        self.slack_conversation_files_error = None;
        self.rebuild_slack_conversation_files_explorer(false);
        self.queue_initial_slack_conversation_files_images(cx);
        if self.slack_conversation_files_reload_pending {
            self.start_pending_slack_conversation_files_reload(cx);
        } else {
            self.ensure_slack_conversation_files_filter(cx);
            cx.notify();
        }
    }

    pub(in crate::ui::surface::state) fn rebuild_slack_conversation_files_explorer(
        &mut self,
        reset_list: bool,
    ) {
        let rows = self.slack_conversation_files_explorer_rows();
        if self.slack_conversation_files_explorer_rows == rows {
            if reset_list {
                self.slack_conversation_files_list_state.reset(rows.len());
            }
            return;
        }
        if reset_list {
            self.slack_conversation_files_explorer_rows = rows;
            self.slack_conversation_files_list_state
                .reset(self.slack_conversation_files_explorer_rows.len());
            return;
        }
        let old_len = self.slack_conversation_files_explorer_rows.len();
        let new_len = rows.len();
        let common_prefix = self
            .slack_conversation_files_explorer_rows
            .iter()
            .zip(rows.iter())
            .take_while(|(old, new)| old == new)
            .count();
        let common_suffix = self.slack_conversation_files_explorer_rows[common_prefix..]
            .iter()
            .rev()
            .zip(rows[common_prefix..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        self.slack_conversation_files_explorer_rows = rows;
        self.slack_conversation_files_list_state.splice(
            common_prefix..old_len - common_suffix,
            new_len - common_prefix - common_suffix,
        );
    }

    fn slack_conversation_files_explorer_rows(&self) -> Arc<[SlackConversationFilesExplorerRow]> {
        let filter = self.slack_conversation_files_filter;
        let cache = &self.slack_conversation_files_cache;
        let terminal_rows =
            |terminal: SlackConversationFilesExplorerRow|
             -> Arc<[SlackConversationFilesExplorerRow]> {
            let mut rows = cache.explorer_rows(filter).to_vec();
            if rows.iter().any(|row| {
                matches!(
                    row,
                    SlackConversationFilesExplorerRow::MediaGrid { preview: true, .. }
                )
            }) {
                rows.retain(|row| *row != SlackConversationFilesExplorerRow::Empty);
                rows.push(terminal);
                rows.into()
            } else {
                Arc::from([SlackConversationFilesExplorerRow::Controls, terminal])
            }
        };
        if self.slack_conversation_files_loading
            && !cache.loaded(filter)
            && cache.rows(filter).is_empty()
        {
            terminal_rows(SlackConversationFilesExplorerRow::Loading)
        } else if self.slack_conversation_files_error.is_some() && cache.rows(filter).is_empty() {
            terminal_rows(SlackConversationFilesExplorerRow::Error)
        } else if cache.explorer_rows(filter).is_empty() {
            Arc::from([
                SlackConversationFilesExplorerRow::Controls,
                SlackConversationFilesExplorerRow::Empty,
            ])
        } else {
            cache.explorer_rows(filter).clone()
        }
    }

    pub(in crate::ui::surface::state) fn queue_initial_slack_conversation_files_images(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_tab != SlackMainTab::FilesLinks {
            return;
        }
        let visible_start = self
            .slack_conversation_files_list_state
            .logical_scroll_top()
            .item_ix
            .min(self.slack_conversation_files_explorer_rows.len());
        let mut visible_height = 0.0;
        let mut visible_end = visible_start;
        for row in self.slack_conversation_files_explorer_rows[visible_start..]
            .iter()
            .copied()
        {
            if visible_height >= SLACK_CONVERSATION_FILES_INITIAL_VISIBLE_HEIGHT {
                break;
            }
            visible_height += row.height();
            visible_end += 1;
        }
        self.queue_slack_conversation_files_explorer_images(visible_start, visible_end, cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_conversation_files_explorer_images(
        &mut self,
        start: usize,
        end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = start.min(self.slack_conversation_files_explorer_rows.len());
        let end = end.min(self.slack_conversation_files_explorer_rows.len());
        let mut urls = HashSet::new();
        for explorer_row in &self.slack_conversation_files_explorer_rows[start..end] {
            match *explorer_row {
                SlackConversationFilesExplorerRow::MediaGrid { start, end, .. } => {
                    for row in self
                        .slack_conversation_files_cache
                        .media
                        .get(start..end)
                        .expect("conversation Media image range must exist")
                    {
                        if let Some(url) = row.thumbnail_url.as_ref() {
                            urls.insert(url.to_string());
                        }
                    }
                }
                SlackConversationFilesExplorerRow::Result { source, index } => {
                    let row = self
                        .slack_conversation_files_cache
                        .rows(source)
                        .get(index)
                        .expect("conversation Files image row index must exist");
                    if let Some(url) = row.thumbnail_url.as_ref() {
                        urls.insert(url.to_string());
                    }
                }
                SlackConversationFilesExplorerRow::Controls
                | SlackConversationFilesExplorerRow::MediaHeader
                | SlackConversationFilesExplorerRow::Spacer
                | SlackConversationFilesExplorerRow::Loading
                | SlackConversationFilesExplorerRow::Empty
                | SlackConversationFilesExplorerRow::Error => {}
            }
        }
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
