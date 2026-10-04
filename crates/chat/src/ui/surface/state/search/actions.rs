use super::{
    Arc, Context, SlackHistoryDestination, SlackSearchRequest, SurfaceState,
    SLACK_SEARCH_PAGINATION_THRESHOLD, SLACK_SEARCH_RECENT_PLACE_LIMIT,
};
use crate::ui::surface::{
    SlackComposerDestination, SlackMainComposerDraftOwner, SlackRemoteDraftState,
    SlackSearchUnreadDraftPosition,
};
use std::collections::HashSet;

const SLACK_SEARCH_UNREAD_DRAFT_LIMIT: usize = 4;
const SLACK_SEARCH_RECENT_CHANNEL_LIMIT: usize = 20;

impl SurfaceState {
    pub(crate) fn handle_slack_search_list_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        row_count: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_search_results_open {
            return;
        }
        self.prefetch_slack_search_images_for_range(
            visible_start.saturating_sub(2)
                ..visible_end
                    .saturating_add(4)
                    .min(self.slack_search_rows.len()),
            cx,
        );
        if self.slack_search_loading
            || visible_end.saturating_add(SLACK_SEARCH_PAGINATION_THRESHOLD) < row_count
        {
            return;
        }
        let Some(cursor) = self.slack_search_next_cursor.clone() else {
            return;
        };
        self.begin_slack_search(
            SlackSearchRequest {
                generation: self.slack_search_generation,
                query: self.slack_search_request_query.clone(),
                display_query: self.slack_search_committed_query.clone(),
                cursor: Some(cursor),
                options: self.slack_search_options.clone(),
            },
            cx,
        );
    }

    pub(crate) fn activate_slack_search_query(&mut self, cx: &mut Context<Self>) {
        let display_query = self.slack_search_query.trim().to_string();
        let Some(query) = self.slack_search_api_query(&display_query) else {
            return;
        };
        self.leave_slack_directory(cx);
        self.slack_search_request_query.clone_from(&query);
        self.slack_search_committed_query.clone_from(&display_query);
        self.slack_search_results_open = true;
        self.slack_search_open = false;
        self.record_slack_search_history(&display_query);
        if self.slack_search_rows.is_empty() && !self.slack_search_loading {
            self.begin_slack_search(
                SlackSearchRequest {
                    generation: self.slack_search_generation,
                    query,
                    display_query: self.slack_search_committed_query.clone(),
                    cursor: None,
                    options: self.slack_search_options.clone(),
                },
                cx,
            );
        }
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_search_option(&mut self, cx: &mut Context<Self>) {
        let selected_option = self.slack_search_selected_option;
        if !self.slack_search_has_api_query() {
            let unread_drafts = self.slack_search_unreads_and_drafts();
            if let Some(position) = unread_drafts.get(selected_option) {
                let conversation_id = self
                    .slack_workspace()
                    .and_then(|workspace| workspace.sections.get(position.section_index))
                    .and_then(|section| section.items.get(position.item_index))
                    .map(|item| item.target_id.clone());
                if let Some(conversation_id) = conversation_id {
                    self.select_slack_conversation(&conversation_id, cx);
                }
                return;
            }
            let Some(history_index) = self
                .slack_search_recent_history_indices()
                .get(selected_option.saturating_sub(unread_drafts.len()))
                .copied()
            else {
                return;
            };
            self.activate_slack_history_menu_index(history_index, cx);
            return;
        }
        let action_count = self.slack_search_action_count();
        if selected_option < action_count {
            if selected_option == 0 {
                self.activate_slack_search_query(cx);
            } else {
                self.activate_slack_search_in_current_conversation(cx);
            }
            return;
        }
        let result_index = selected_option - action_count;
        if result_index < self.slack_quick_search_rows.len() {
            self.activate_slack_quick_search_row(result_index, cx);
            return;
        }
        self.activate_slack_quick_search_message(
            result_index - self.slack_quick_search_rows.len(),
            cx,
        );
    }

    pub(crate) fn select_slack_search_option(
        &mut self,
        option_index: usize,
        cx: &mut Context<Self>,
    ) {
        let option_index = option_index.min(self.slack_search_option_count().saturating_sub(1));
        if self.slack_search_selected_option == option_index {
            return;
        }
        self.slack_search_selected_option = option_index;
        let quick_row_start = self.slack_search_action_count();
        if option_index < quick_row_start {
            self.slack_quick_search_options_scroll_handle
                .scroll_to_item(option_index);
        } else if (quick_row_start..quick_row_start + self.slack_quick_search_rows.len())
            .contains(&option_index)
        {
            self.slack_quick_search_options_scroll_handle
                .scroll_to_item(quick_row_start);
            self.slack_quick_search_scroll_handle.scroll_to_item(
                option_index - quick_row_start,
                super::ScrollStrategy::Nearest,
            );
        } else {
            self.slack_quick_search_options_scroll_handle
                .scroll_to_item(quick_row_start + 1);
        }
        self.notify_slack_search_overlay(cx);
    }

    pub(crate) fn move_slack_search_selection(&mut self, direction: i32, cx: &mut Context<Self>) {
        let option_count = self.slack_search_option_count();
        if option_count == 0 {
            return;
        }
        let selected_option = self.slack_search_selected_option.min(option_count - 1);
        let next_option = if direction < 0 {
            selected_option.checked_sub(1).unwrap_or(option_count - 1)
        } else {
            (selected_option + 1) % option_count
        };
        self.select_slack_search_option(next_option, cx);
    }

    pub(crate) fn slack_search_option_count(&self) -> usize {
        if !self.slack_search_has_api_query() {
            return self.slack_search_unreads_and_drafts().len()
                + self.slack_search_recent_history_indices().len();
        }
        self.slack_search_action_count()
            + self.slack_quick_search_rows.len()
            + self.slack_quick_search_message_rows.len()
    }

    pub(in crate::ui::surface::state) fn slack_search_recent_channel_ids(&self) -> Vec<String> {
        let mut recent_channels = Vec::with_capacity(SLACK_SEARCH_RECENT_CHANNEL_LIMIT);
        let mut seen = HashSet::with_capacity(SLACK_SEARCH_RECENT_CHANNEL_LIMIT);
        if let Some(conversation_id) = self
            .slack_workspace()
            .map(|workspace| workspace.conversation_id.as_str())
            .filter(|conversation_id| !conversation_id.is_empty())
        {
            seen.insert(conversation_id.to_string());
            recent_channels.push(conversation_id.to_string());
        }
        for entry in self.slack_conversation_history.iter().rev() {
            let SlackHistoryDestination::Conversation {
                conversation_id, ..
            } = &entry.destination
            else {
                continue;
            };
            if conversation_id.is_empty() || !seen.insert(conversation_id.to_string()) {
                continue;
            }
            recent_channels.push(conversation_id.to_string());
            if recent_channels.len() == SLACK_SEARCH_RECENT_CHANNEL_LIMIT {
                break;
            }
        }
        recent_channels
    }

    pub(crate) fn slack_search_recent_history_indices(&self) -> Vec<usize> {
        let mut recent_indices: Vec<usize> = Vec::with_capacity(SLACK_SEARCH_RECENT_PLACE_LIMIT);
        for (history_index, entry) in self.slack_conversation_history.iter().enumerate().rev() {
            let SlackHistoryDestination::Conversation {
                conversation_id, ..
            } = &entry.destination
            else {
                continue;
            };
            if recent_indices.iter().any(|existing_index| {
                matches!(
                    &self.slack_conversation_history[*existing_index].destination,
                    SlackHistoryDestination::Conversation {
                        conversation_id: existing_id,
                        ..
                    } if existing_id.as_ref() == conversation_id.as_ref()
                )
            }) {
                continue;
            }
            recent_indices.push(history_index);
            if recent_indices.len() == SLACK_SEARCH_RECENT_PLACE_LIMIT {
                break;
            }
        }
        recent_indices
    }

    pub(crate) fn slack_search_unreads_and_drafts(&self) -> Vec<SlackSearchUnreadDraftPosition> {
        let Some(workspace) = self.slack_workspace() else {
            return Vec::new();
        };
        let Some(self_user_id) = workspace.self_user_id.as_deref() else {
            return Vec::new();
        };
        let draft_conversation_ids =
            self.slack_search_draft_conversation_ids(&workspace.team_id, self_user_id);

        let mut positions = Vec::with_capacity(SLACK_SEARCH_UNREAD_DRAFT_LIMIT);
        let mut seen = HashSet::new();
        for (section_index, section) in workspace.sections.iter().enumerate() {
            for (item_index, item) in section.items.iter().enumerate() {
                let has_draft = draft_conversation_ids.contains(&item.target_id);
                if item.target_id.is_empty()
                    || (!item.unread && !has_draft)
                    || !seen.insert(item.target_id.as_str())
                {
                    continue;
                }
                positions.push(SlackSearchUnreadDraftPosition {
                    section_index,
                    item_index,
                    has_draft,
                });
                if positions.len() == SLACK_SEARCH_UNREAD_DRAFT_LIMIT {
                    return positions;
                }
            }
        }
        positions
    }

    fn slack_search_draft_conversation_ids(
        &self,
        team_id: &str,
        self_user_id: &str,
    ) -> HashSet<String> {
        let mut conversation_ids = self
            .slack_composer_drafts
            .iter()
            .filter(|(key, draft)| {
                key.team_id == team_id && key.self_user_id == self_user_id && !draft.is_empty()
            })
            .map(|(key, _)| slack_search_draft_conversation_id(&key.destination).to_string())
            .collect::<HashSet<_>>();
        conversation_ids.extend(
            self.slack_remote_drafts
                .iter()
                .filter(|(key, state)| {
                    key.team_id == team_id
                        && key.self_user_id == self_user_id
                        && matches!(state, SlackRemoteDraftState::Present(_))
                })
                .map(|(key, _)| slack_search_draft_conversation_id(&key.destination).to_string()),
        );
        if !self.slack_composer_text.trim().is_empty() || !self.slack_composer_files.is_empty() {
            if let Some(SlackMainComposerDraftOwner::Conversation(key)) = self
                .slack_active_main_composer_context
                .as_ref()
                .map(|context| &context.owner)
            {
                if key.team_id == team_id && key.self_user_id == self_user_id {
                    conversation_ids
                        .insert(slack_search_draft_conversation_id(&key.destination).to_string());
                }
            }
        }
        conversation_ids
    }

    pub(crate) fn slack_search_has_current_conversation_action(&self) -> bool {
        self.slack_search_scope_modifier.is_some()
            && !self
                .slack_search_current_conversation_display_modifier()
                .is_some_and(|display| self.slack_search_query.contains(&display))
    }

    pub(crate) fn activate_slack_search_in_current_conversation(&mut self, cx: &mut Context<Self>) {
        let display_query = self.slack_search_query.trim().to_string();
        let Some(modifier) = self.slack_search_scope_modifier.clone() else {
            return;
        };
        let Some(query) = self.slack_search_api_query(&display_query) else {
            return;
        };
        self.leave_slack_directory(cx);
        let has_display_scope = self
            .slack_search_current_conversation_display_modifier()
            .is_some_and(|display| display_query.starts_with(&display));
        self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
        self.slack_search_request_query = if has_display_scope {
            query
        } else {
            format!("{query} {modifier}")
        };
        self.slack_search_committed_query = display_query.clone();
        self.slack_search_results_open = true;
        self.slack_search_open = false;
        self.record_slack_search_history(&display_query);
        self.slack_search_rows = Arc::default();
        self.slack_search_list_state.reset(0);
        self.slack_search_total = 0;
        self.slack_search_next_cursor = None;
        self.slack_search_loading = false;
        self.slack_search_error = None;
        self.begin_slack_search(
            SlackSearchRequest {
                generation: self.slack_search_generation,
                query: self.slack_search_request_query.clone(),
                display_query,
                cursor: None,
                options: self.slack_search_options.clone(),
            },
            cx,
        );
    }

    pub(crate) fn slack_search_has_api_query(&self) -> bool {
        self.slack_search_api_query(&self.slack_search_query)
            .is_some()
    }
}

fn slack_search_draft_conversation_id(destination: &SlackComposerDestination) -> &str {
    match destination {
        SlackComposerDestination::Conversation { conversation_id }
        | SlackComposerDestination::Thread {
            conversation_id, ..
        } => conversation_id,
    }
}
