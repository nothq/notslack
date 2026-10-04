use super::{
    next_slack_files_generation, Arc, Context, ScrollStrategy, SharedString, SlackRailView,
    SurfaceState, SLACK_FILES_PAGINATION_THRESHOLD,
};

impl SurfaceState {
    pub(crate) fn handle_slack_files_list_scroll(
        &mut self,
        visible_end: usize,
        row_count: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_rail_view != SlackRailView::Files
            || self.slack_files_loading
            || visible_end.saturating_add(SLACK_FILES_PAGINATION_THRESHOLD) < row_count
        {
            return;
        }
        let Some(page) = self
            .slack_files_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.pagination.next_page())
        else {
            return;
        };
        self.begin_slack_files_page(page, cx);
    }

    pub(crate) fn select_slack_file_row(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.slack_files_rows.iter().any(|row| row.id == id) {
            self.slack_files_selected_id = Some(id);
            cx.notify();
        }
    }

    pub(crate) fn move_slack_file_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        if self.slack_files_rows.is_empty() {
            return;
        }
        let selected_index = self
            .slack_files_selected_id
            .as_ref()
            .and_then(|id| self.slack_files_rows.iter().position(|row| &row.id == id));
        let next_index = match (selected_index, direction < 0) {
            (Some(selected_index), true) => selected_index
                .checked_sub(1)
                .unwrap_or(self.slack_files_rows.len() - 1),
            (Some(selected_index), false) => (selected_index + 1) % self.slack_files_rows.len(),
            (None, true) => self.slack_files_rows.len() - 1,
            (None, false) => 0,
        };
        self.slack_files_selected_id = Some(self.slack_files_rows[next_index].id.clone());
        self.slack_files_scroll_handle
            .scroll_to_item(next_index, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub(crate) fn open_selected_slack_file(&mut self, cx: &mut Context<Self>) {
        let Some(selected_id) = self.slack_files_selected_id.as_ref() else {
            return;
        };
        let Some(link_url) = self
            .slack_files_rows
            .iter()
            .find(|row| &row.id == selected_id)
            .map(|row| row.link_url.to_string())
        else {
            return;
        };
        self.open_slack_link(&link_url, cx);
    }

    pub(crate) fn open_slack_file(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some((selected_id, link_url)) = self
            .slack_files_rows
            .iter()
            .find(|row| row.id.as_ref() == id)
            .map(|row| (row.id.clone(), row.link_url.to_string()))
        else {
            return;
        };
        self.slack_files_selected_id = Some(selected_id);
        cx.notify();
        self.open_slack_link(&link_url, cx);
    }

    pub(in crate::ui::surface::state) fn reload_slack_files(&mut self, cx: &mut Context<Self>) {
        self.slack_files_generation = next_slack_files_generation(self.slack_files_generation);
        self.slack_files_snapshot = None;
        self.slack_files_rows = Arc::default();
        self.slack_files_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_files_error = None;
        self.slack_files_selected_id = None;
        if self.slack_files_loading {
            cx.notify();
            return;
        }
        self.begin_slack_files_page(1, cx);
    }
}
