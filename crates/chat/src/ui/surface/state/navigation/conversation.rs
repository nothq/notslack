use super::{
    finish_slack_conversation_load, load_slack_conversation_snapshot, Arc, Context,
    SlackConversationLoadRoute, SlackMainRoute, SlackMainTab, SlackRailView, SlackSidebarRow,
    SlackSidebarRowKind, SurfaceState, WorkspaceApi, SLACK_CONVERSATION_LOAD_CONCURRENCY,
};

impl SurfaceState {
    pub(crate) fn select_slack_conversation(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.leave_slack_new_message(cx);
        self.cancel_slack_message_navigation();
        self.select_slack_conversation_inner(
            conversation_id,
            SlackConversationLoadRoute::Conversation,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn select_slack_conversation_for_message(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.leave_slack_new_message(cx);
        self.select_slack_conversation_inner(
            conversation_id,
            SlackConversationLoadRoute::Conversation,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn select_slack_conversation_for_new_message(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.select_slack_conversation_inner(
            conversation_id,
            SlackConversationLoadRoute::NewMessage,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn select_slack_conversation_inner(
        &mut self,
        conversation_id: &str,
        route: SlackConversationLoadRoute,
        cx: &mut Context<Self>,
    ) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_conversation {
            return;
        }
        self.prepare_slack_conversation_route(conversation_id, route, cx);
        if self.slack_conversation_id() == Some(conversation_id)
            && !self
                .slack_active_loading_conversation_ids
                .contains(conversation_id)
            && !self.slack_message_navigation_requires_conversation_load(conversation_id)
        {
            self.clear_slack_pending_conversation_load();
            let _ = self.restore_matching_slack_composer_schedule_recovery(cx);
            self.restore_matching_slack_scheduled_edit_recovery(cx);
            cx.notify();
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.clear_slack_conversation_read_overlay_for_id(conversation_id);
            self.clear_slack_conversation_load_profile();
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };

        let requested_conversation_id = conversation_id.to_string();
        self.start_slack_conversation_load_profile(&requested_conversation_id);
        self.slack_pending_conversation_id = Some(requested_conversation_id.clone());
        self.slack_pending_conversation_route = route;
        self.mark_slack_pending_sidebar_selection(&requested_conversation_id);
        self.slack_error = None;
        self.slack_profile_panel = None;
        self.slack_composer_focused = false;
        cx.notify();
        self.start_slack_conversation_load(workspace_api, cx);
    }

    fn prepare_slack_conversation_route(
        &mut self,
        conversation_id: &str,
        route: SlackConversationLoadRoute,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_id() != Some(conversation_id) {
            self.cancel_slack_composer_capture_for_owner_change(cx);
        }
        self.prepare_slack_conversation_selection(route, cx);
        if self.slack_active_rail_view != SlackRailView::Dms {
            self.slack_active_rail_view = SlackRailView::Home;
        }
        self.slack_active_tab = SlackMainTab::Messages;
        self.close_slack_search_results(cx);
        self.close_slack_search(cx);
        self.reset_slack_schedule_context(cx);
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        if self.slack_conversation_id() != Some(conversation_id) {
            self.reset_slack_thread_context();
        }
        self.slack_aux_panel = None;
        self.slack_expanded_attachment = None;
        self.queue_slack_sidebar_active_row_reveal(conversation_id);
        if route == SlackConversationLoadRoute::Conversation
            && self.install_slack_conversation_read_overlay(conversation_id)
        {
            cx.notify();
        }
    }

    fn prepare_slack_conversation_selection(
        &mut self,
        route: SlackConversationLoadRoute,
        cx: &mut Context<Self>,
    ) {
        self.leave_slack_all_threads();
        self.leave_slack_directory(cx);
        self.slack_main_route = route.main_route();
        self.slack_pending_draft_restore = None;
        self.slack_dms_peek_visible = false;
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_bookmark_folder();
        self.leave_slack_drafts_sent();
    }

    pub(in crate::ui::surface::state) fn mark_slack_pending_sidebar_selection(
        &mut self,
        conversation_id: &str,
    ) {
        let mut rows = self.slack_sidebar_rows.to_vec();
        let mut changed = false;
        for row in &mut rows {
            let SlackSidebarRowKind::Item { item, .. } = &mut row.kind else {
                continue;
            };
            let active = item.target_id == conversation_id;
            if item.active != active {
                item.active = active;
                changed = true;
            }
        }
        if changed {
            SlackSidebarRow::refresh_boundary_state(&mut rows);
            let mut rows = self.project_slack_conversation_read_sidebar_rows(Arc::from(rows));
            self.slack_presence_authority
                .overlay_sidebar_rows(&mut rows);
            self.slack_sidebar_rows = rows;
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_sidebar(data);
        }
    }

    pub(in crate::ui::surface::state) fn start_slack_conversation_load(
        &mut self,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        let Some(requested_conversation_id) = self.slack_pending_conversation_id.clone() else {
            cx.notify();
            return;
        };
        let route = self.slack_pending_conversation_route;
        if !self.can_start_slack_conversation_load(&requested_conversation_id, cx) {
            return;
        }
        let anchor_timestamp = self.slack_message_navigation_anchor(&requested_conversation_id);
        self.slack_active_loading_conversation_ids
            .insert(requested_conversation_id.clone());
        self.spawn_background_task(
            requested_conversation_id.clone(),
            cx,
            move |requested_conversation_id| {
                load_slack_conversation_snapshot(
                    workspace_api,
                    requested_conversation_id,
                    anchor_timestamp,
                    route,
                )
            },
            move |this, result, cx| {
                finish_slack_conversation_load(this, &requested_conversation_id, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn can_start_slack_conversation_load(
        &mut self,
        requested_conversation_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_conversation_id() == Some(requested_conversation_id)
            && !self.slack_message_navigation_requires_conversation_load(requested_conversation_id)
        {
            self.clear_slack_pending_conversation_load();
            cx.notify();
            return false;
        }
        if self
            .slack_active_loading_conversation_ids
            .contains(requested_conversation_id)
            || self.slack_active_loading_conversation_ids.len()
                >= SLACK_CONVERSATION_LOAD_CONCURRENCY
        {
            cx.notify();
            return false;
        }
        true
    }

    pub(in crate::ui::surface::state) fn cancel_pending_slack_new_message_conversation_load(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_conversation_id.is_none()
            || self.slack_pending_conversation_route != SlackConversationLoadRoute::NewMessage
        {
            return;
        }
        self.clear_slack_conversation_load_profile();
        self.clear_slack_pending_conversation_load();
        self.refresh_slack_sidebar_rows();
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn clear_slack_pending_conversation_load(&mut self) {
        self.slack_pending_conversation_id = None;
        self.slack_pending_conversation_route = SlackConversationLoadRoute::Conversation;
    }

    pub(in crate::ui::surface::state) fn slack_conversation_load_route_is_current(
        &self,
        requested_conversation_id: &str,
        route: SlackConversationLoadRoute,
    ) -> bool {
        match route {
            SlackConversationLoadRoute::Conversation => {
                self.slack_main_route == SlackMainRoute::Conversation
            }
            SlackConversationLoadRoute::NewMessage => {
                self.slack_main_route == SlackMainRoute::NewMessage
                    && self
                        .slack_new_message_destination
                        .as_ref()
                        .is_some_and(|destination| {
                            destination.conversation_id.as_ref() == requested_conversation_id
                        })
            }
        }
    }
}
