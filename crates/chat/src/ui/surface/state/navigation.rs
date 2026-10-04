mod conversation;
mod keys;
mod menus;
mod profile;

use super::{
    keystroke_input_text, prepare_slack_conversation_snapshot, Arc, ChatStartup, Context,
    KeyDownEvent, SlackConversationLoadResult, SlackConversationLoadRoute, SlackMainTab,
    SlackProfilePanelState, SlackRailView, SlackSidebarRow, SlackSidebarRowKind, SurfaceState,
    WorkspaceApi, SLACK_CONVERSATION_LOAD_CONCURRENCY,
};
use crate::ui::surface::SlackMainRoute;

impl SurfaceState {
    pub(crate) fn handle_slack_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_slack_workspace() {
            if event.keystroke.key.eq_ignore_ascii_case("r")
                && !event.keystroke.modifiers.modified()
                && matches!(self.chat_startup, ChatStartup::Error { .. })
            {
                self.reconnect_chat_workspace(cx);
                return true;
            }
            return false;
        }
        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform
            && modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.function
            && event.keystroke.key.eq_ignore_ascii_case("s")
        {
            self.toggle_slack_workspace_switcher(cx);
            return true;
        }
        if self.handle_slack_all_threads_shortcut(event, cx) {
            return true;
        }
        self.handle_slack_workspace_key_down(event, cx)
    }
}

fn load_slack_conversation_snapshot(
    workspace_api: Arc<dyn WorkspaceApi>,
    requested_conversation_id: String,
    anchor_timestamp: Option<crate::ui::SlackMessageTimestamp>,
    route: SlackConversationLoadRoute,
) -> SlackConversationLoadResult {
    let worker_started_at = std::time::Instant::now();
    let result = match anchor_timestamp.as_ref() {
        Some(anchor_timestamp) => {
            workspace_api.load_slack_conversation_at(&requested_conversation_id, anchor_timestamp)
        }
        None => workspace_api.load_slack_conversation(&requested_conversation_id),
    };
    let workspace_loaded_at = std::time::Instant::now();
    let result = result.map(|conversation| {
        let prepared = prepare_slack_conversation_snapshot(conversation);
        (prepared, std::time::Instant::now())
    });
    match result {
        Ok((prepared, prepared_at)) => SlackConversationLoadResult {
            result: Ok(prepared),
            anchor_timestamp,
            route,
            worker_started_at,
            workspace_loaded_at,
            prepared_at: Some(prepared_at),
        },
        Err(message) => SlackConversationLoadResult {
            result: Err(message),
            anchor_timestamp,
            route,
            worker_started_at,
            workspace_loaded_at,
            prepared_at: None,
        },
    }
}

fn finish_slack_conversation_load(
    this: &mut SurfaceState,
    requested_conversation_id: &str,
    result: SlackConversationLoadResult,
    cx: &mut Context<SurfaceState>,
) {
    this.record_slack_conversation_load_result(requested_conversation_id, &result);
    this.slack_active_loading_conversation_ids
        .remove(requested_conversation_id);
    let pending_load_matches = this.slack_pending_conversation_id.as_deref()
        == Some(requested_conversation_id)
        && this.slack_pending_conversation_route == result.route;
    if pending_load_matches
        && this.slack_conversation_load_route_is_current(requested_conversation_id, result.route)
    {
        if result.result.is_err() {
            this.clear_slack_conversation_read_overlay_for_id(requested_conversation_id);
        }
        apply_slack_conversation_load_result(this, result, cx);
        if this.slack_pending_conversation_id.as_deref() == Some(requested_conversation_id) {
            this.clear_slack_pending_conversation_load();
        }
    } else if pending_load_matches {
        this.clear_slack_conversation_read_overlay_for_id(requested_conversation_id);
        this.clear_slack_conversation_load_profile();
        this.clear_slack_pending_conversation_load();
        this.refresh_slack_sidebar_rows();
        cx.notify();
    }
    let Some(workspace_api) = this.active_slack_workspace_api() else {
        return;
    };
    this.start_slack_conversation_load(workspace_api, cx);
}

fn apply_slack_conversation_load_result(
    this: &mut SurfaceState,
    result: SlackConversationLoadResult,
    cx: &mut Context<SurfaceState>,
) {
    let navigation_load_is_current = result.anchor_timestamp.as_ref().is_some_and(|anchor| {
        this.slack_message_navigation
            .as_ref()
            .is_some_and(|request| request.target.anchor_timestamp() == anchor)
    });
    match result.result {
        Ok(prepared) => {
            this.apply_prepared_slack_conversation_snapshot(prepared, cx);
            if navigation_load_is_current {
                this.continue_slack_message_navigation(cx);
            }
        }
        Err(message) => {
            this.clear_slack_conversation_load_profile();
            this.clear_slack_pending_conversation_load();
            this.slack_history_target_index = None;
            this.slack_history_menu_target = None;
            this.cancel_slack_message_navigation();
            this.refresh_slack_sidebar_rows();
            this.slack_error = Some(message);
            cx.notify();
        }
    }
}
