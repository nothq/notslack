mod actions;
mod control;
mod input;
mod query;
mod quick;

use std::{ops::Range, rc::Rc, sync::Arc};

use gpui::ScrollStrategy;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputHighlight, TextInputProps, TextInputStyle,
};

use super::{
    prepare_slack_search_snapshot, Context, PreparedSlackSearchSnapshot, SurfaceState, WorkspaceApi,
};
use crate::ui::surface::slack_message_timezone;
use crate::ui::surface::{slack_palette, SlackHistoryDestination, SLACK_SEARCH_RECENT_PLACE_LIMIT};
use crate::ui::{
    alpha, px, rgb, spawn_background_task_for_entity, Entity, SlackConversationKind,
    SlackMessageSearchOptions, SlackMessageSearchRequest as SlackApiSearchRequest,
    SlackMessageSearchSort,
};

const SLACK_SEARCH_PAGINATION_THRESHOLD: usize = 4;

#[derive(Clone)]
struct SlackSearchRequest {
    generation: u64,
    query: String,
    display_query: String,
    cursor: Option<String>,
    options: SlackMessageSearchOptions,
}

impl SurfaceState {
    fn notify_slack_search_overlay(&self, cx: &mut Context<Self>) {
        self.slack_search_overlay.update(cx, |_, cx| cx.notify());
    }

    pub(crate) fn open_slack_search(&mut self, cx: &mut Context<Self>) {
        let reaction_picker_closed = self.slack_reaction_picker.take().is_some();
        if self.slack_search_open {
            if reaction_picker_closed {
                cx.notify();
            }
            return;
        }
        self.slack_search_open = true;
        self.slack_search_scope_modifier = self.slack_search_current_conversation_modifier();
        self.slack_search_selected_option = 0;
        self.schedule_slack_quick_search(cx);
        cx.notify();
    }

    pub(crate) fn open_slack_conversation_search(&mut self, cx: &mut Context<Self>) {
        let Some(display_modifier) = self.slack_search_current_conversation_display_modifier()
        else {
            self.open_slack_search(cx);
            return;
        };
        let Some(request_modifier) = self.slack_search_current_conversation_modifier() else {
            self.open_slack_search(cx);
            return;
        };
        self.reset_slack_search_state();
        self.slack_search_open = true;
        self.slack_search_scope_modifier = Some(request_modifier);
        self.slack_search_query = format!("{display_modifier}  ");
        self.slack_search_selected_option = 0;
        let query = self.slack_search_query.clone();
        self.slack_search_input.update(cx, |input, cx| {
            input.set_text_and_move_cursor_to_end(query, cx);
        });
        self.schedule_slack_quick_search(cx);
        cx.notify();
    }

    pub(crate) fn close_slack_search(&mut self, cx: &mut Context<Self>) {
        if !self.slack_search_open {
            return;
        }
        self.slack_search_open = false;
        self.slack_quick_search_loading = false;
        self.slack_quick_message_last_started_at = None;
        if !self.slack_search_results_open {
            self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
            self.slack_search_loading = false;
        }
        cx.notify();
    }

    pub(super) fn set_slack_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        let was_open = self.slack_search_open;
        self.slack_reaction_picker = None;
        self.slack_search_open = true;
        self.slack_search_query = query;
        self.slack_search_request_query.clear();
        self.slack_search_selected_option = 0;
        self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
        self.slack_quick_search_rows = Arc::default();
        self.slack_quick_search_message_rows = Arc::default();
        self.slack_quick_search_options_scroll_handle = gpui::ScrollHandle::new();
        self.slack_quick_search_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_quick_search_prefetched_range = None;
        self.slack_quick_search_loading = false;
        self.slack_quick_search_error = None;
        self.slack_search_rows = Arc::default();
        self.slack_search_list_state.reset(0);
        self.slack_search_total = 0;
        self.slack_search_next_cursor = None;
        self.slack_search_loading = false;
        self.slack_search_error = None;
        self.schedule_slack_quick_search(cx);
        if was_open {
            self.notify_slack_search_overlay(cx);
        } else {
            cx.notify();
        }
    }

    fn begin_slack_search(&mut self, request: SlackSearchRequest, cx: &mut Context<Self>) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_search_error =
                Some("Slack message search requires a connected workspace".to_string());
            self.slack_search_loading = false;
            cx.notify();
            return;
        };
        let timezone = slack_message_timezone(
            self.slack_workspace()
                .and_then(|workspace| workspace.self_timezone_id.as_deref()),
        );
        self.slack_search_loading = true;
        self.slack_search_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            move |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackSearchRequest)| {
                let api_request = SlackApiSearchRequest {
                    query: request.query.clone(),
                    cursor: request.cursor.clone(),
                    options: request.options.clone(),
                };
                let result = workspace_api
                    .search_slack_messages_with_options(&api_request)
                    .map(|snapshot| {
                        prepare_slack_search_snapshot(
                            snapshot,
                            &request.display_query,
                            request.generation,
                            timezone,
                        )
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_search(request, result, cx);
            },
        );
    }

    fn finish_slack_search(
        &mut self,
        request: SlackSearchRequest,
        result: Result<PreparedSlackSearchSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if (!self.slack_search_open && !self.slack_search_results_open)
            || self.slack_search_generation != request.generation
            || self.slack_search_query.trim() != request.display_query
        {
            return;
        }
        self.slack_search_loading = false;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_search_error = Some(error);
                cx.notify();
                return;
            }
        };
        let old_row_count = self.slack_search_rows.len();
        if request.cursor.is_some() {
            let mut rows = Vec::with_capacity(old_row_count + prepared.rows.len());
            rows.extend(self.slack_search_rows.iter().cloned());
            rows.extend(prepared.rows.iter().cloned());
            self.slack_search_rows = rows.into();
            self.slack_search_list_state.splice(
                old_row_count..old_row_count,
                self.slack_search_rows.len() - old_row_count,
            );
        } else {
            self.slack_search_rows = prepared.rows;
            self.slack_search_list_state
                .reset(self.slack_search_rows.len());
        }
        self.slack_search_total = prepared.snapshot.total;
        self.slack_search_next_cursor = prepared.snapshot.next_cursor;
        self.slack_search_error = None;
        self.slack_search_selected_option = self
            .slack_search_selected_option
            .min(self.slack_search_option_count().saturating_sub(1));
        self.prefetch_slack_search_images_for_range(0..self.slack_search_rows.len().min(8), cx);
        cx.notify();
    }

    pub(crate) fn toggle_slack_search_from_self(&mut self, cx: &mut Context<Self>) {
        self.slack_search_options.from = if self.slack_search_options.from.is_some() {
            None
        } else {
            self.slack_workspace()
                .and_then(|workspace| workspace.self_user_id.as_deref())
                .and_then(|user_id| slack_search_reference_id(user_id, &['U', 'W']))
                .map(|user_id| format!("<@{user_id}>"))
        };
        self.restart_slack_search_with_options(cx);
    }

    pub(crate) fn toggle_slack_search_in_current(&mut self, cx: &mut Context<Self>) {
        self.slack_search_options.in_conversation =
            if self.slack_search_options.in_conversation.is_some() {
                None
            } else {
                self.slack_search_current_conversation_modifier()
                    .and_then(|modifier| {
                        modifier.split_once(':').map(|(_, value)| value.to_string())
                    })
            };
        self.restart_slack_search_with_options(cx);
    }

    pub(crate) fn toggle_slack_search_only_my_channels(&mut self, cx: &mut Context<Self>) {
        self.slack_search_options.only_my_channels = !self.slack_search_options.only_my_channels;
        self.restart_slack_search_with_options(cx);
    }

    pub(crate) fn toggle_slack_search_automations(&mut self, cx: &mut Context<Self>) {
        self.slack_search_options.include_automations =
            !self.slack_search_options.include_automations;
        self.restart_slack_search_with_options(cx);
    }

    pub(crate) fn toggle_slack_search_sort(&mut self, cx: &mut Context<Self>) {
        self.slack_search_options.sort = match self.slack_search_options.sort {
            SlackMessageSearchSort::Relevant => SlackMessageSearchSort::Recent,
            SlackMessageSearchSort::Recent => SlackMessageSearchSort::Relevant,
        };
        self.restart_slack_search_with_options(cx);
    }

    fn restart_slack_search_with_options(&mut self, cx: &mut Context<Self>) {
        if !self.slack_search_results_open || self.slack_search_committed_query.is_empty() {
            cx.notify();
            return;
        }
        self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
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
                display_query: self.slack_search_committed_query.clone(),
                cursor: None,
                options: self.slack_search_options.clone(),
            },
            cx,
        );
    }
}

fn replace_slack_search_display_token(query: String, display: &str, request: &str) -> String {
    query.replace(display, request)
}

fn slack_search_reference_id<'a>(value: &'a str, prefixes: &[char]) -> Option<&'a str> {
    let value = value.trim();
    (value.len() > 1
        && value
            .chars()
            .next()
            .is_some_and(|prefix| prefixes.contains(&prefix))
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    .then_some(value)
}

fn slack_search_reference_label(value: &str) -> Option<String> {
    let label = value
        .trim()
        .chars()
        .filter(|character| !matches!(character, '<' | '>' | '|'))
        .collect::<String>();
    (!label.is_empty()).then_some(label)
}
