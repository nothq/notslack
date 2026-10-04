use std::sync::Arc;

use gpui::{Context, ScrollStrategy};

use super::SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS;
use crate::ui::surface::{normalize_slack_dm_finder_text, SlackMainRoute, SurfaceState};
use crate::ui::SlackDestinationTarget;

impl SurfaceState {
    pub(super) fn set_slack_directory_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::Directory || self.slack_directory_query == query
        {
            return;
        }
        self.slack_directory_query = query;
        self.slack_directory_normalized_query =
            normalize_slack_dm_finder_text(&self.slack_directory_query).into();
        self.rebuild_slack_directory_visible_rows();
        self.slack_directory_prefetched_range = None;
        self.queue_slack_directory_visible_images(0, SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(super) fn clear_slack_directory_query(&mut self, cx: &mut Context<Self>) {
        if self.slack_directory_query.is_empty() {
            return;
        }
        self.slack_directory_query.clear();
        self.slack_directory_normalized_query = Default::default();
        self.rebuild_slack_directory_visible_rows();
        self.slack_directory_prefetched_range = None;
        self.queue_slack_directory_visible_images(0, SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(super) fn rebuild_slack_directory_visible_rows(&mut self) {
        if self.slack_main_route != SlackMainRoute::Directory {
            self.slack_directory_visible_row_indices = Arc::default();
            self.slack_directory_selected_index = None;
            return;
        }
        let query = self.slack_directory_normalized_query.as_ref();
        self.slack_directory_visible_row_indices = self
            .slack_directory_rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (query.is_empty() || row.search_key.contains(query)).then_some(index)
            })
            .collect::<Vec<_>>()
            .into();
        self.slack_directory_selected_index =
            (!self.slack_directory_visible_row_indices.is_empty()).then_some(0);
        if self.slack_directory_selected_index.is_some() {
            self.slack_directory_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }
    }

    pub(crate) fn move_slack_directory_selection(
        &mut self,
        direction: i32,
        cx: &mut Context<Self>,
    ) {
        let count = self.slack_directory_visible_row_indices.len();
        if self.slack_main_route != SlackMainRoute::Directory || count == 0 {
            return;
        }
        let next = match self.slack_directory_selected_index {
            None if direction < 0 => count - 1,
            None => 0,
            Some(current) if direction < 0 => current.saturating_sub(1),
            Some(current) => current.saturating_add(1).min(count - 1),
        };
        self.slack_directory_selected_index = Some(next);
        self.slack_directory_scroll_handle
            .scroll_to_item(next, ScrollStrategy::Nearest);
        self.queue_slack_directory_visible_images(next, next.saturating_add(1), cx);
        cx.notify();
    }

    pub(super) fn open_selected_slack_directory_person(&mut self, cx: &mut Context<Self>) {
        let Some(visible_index) = self
            .slack_directory_selected_index
            .or_else(|| (!self.slack_directory_visible_row_indices.is_empty()).then_some(0))
        else {
            return;
        };
        self.open_slack_directory_person(visible_index, cx);
    }

    pub(crate) fn open_slack_directory_person(
        &mut self,
        visible_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::Directory {
            return;
        }
        let Some(row) = self
            .slack_directory_visible_row_indices
            .get(visible_index)
            .and_then(|row_index| self.slack_directory_rows.get(*row_index))
        else {
            return;
        };
        let SlackDestinationTarget::Person { user_id } = &row.target else {
            panic!("Slack People directory rows must target people");
        };
        let user_id = user_id.clone();
        self.slack_directory_selected_index = Some(visible_index);
        self.slack_directory_scroll_handle
            .scroll_to_item(visible_index, ScrollStrategy::Nearest);
        self.slack_directory_focus_pending = false;
        self.open_slack_profile(&user_id, cx);
        cx.notify();
    }

    pub(crate) fn queue_slack_directory_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::Directory {
            return;
        }
        let range = (
            visible_start.min(self.slack_directory_visible_row_indices.len()),
            visible_end.min(self.slack_directory_visible_row_indices.len()),
        );
        if self.slack_directory_prefetched_range == Some(range) {
            return;
        }
        self.slack_directory_prefetched_range = Some(range);
        let urls = self.slack_directory_visible_row_indices[range.0..range.1]
            .iter()
            .filter_map(|row_index| self.slack_directory_rows.get(*row_index))
            .filter_map(|row| row.avatar_image_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
