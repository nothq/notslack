use super::{
    slack_workspace_changed, ChatStartup, Context, SlackMainRoute, SlackMainTab, SlackRailView,
    SlackWorkspaceApiCapabilities, SurfaceInput, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn set_input(
        &mut self,
        input: SurfaceInput,
        startup: ChatStartup,
        cx: &mut Context<Self>,
    ) {
        self.embedded_shell = input.embedded_shell;
        self.chat_startup = startup;
        self.slack_workspace_api_capabilities = input
            .workspace_api
            .as_ref()
            .map_or(SlackWorkspaceApiCapabilities::NONE, |api| {
                api.capabilities()
            });
        let workspace_api_changed = match (&self.workspace_api, &input.workspace_api) {
            (Some(current), Some(next)) => !std::sync::Arc::ptr_eq(current, next),
            (None, None) => false,
            _ => true,
        };
        self.apply_slack_message_capability_downgrades(cx);
        self.apply_slack_rail_capability_downgrades(cx);
        self.apply_slack_tab_capability_downgrades(cx);
        self.workspace_api = input.workspace_api;
        if workspace_api_changed {
            self.slack_presence_authority.rebase_source();
            self.reset_slack_realtime_subscription();
            self.reset_slack_reaction_catalog();
            self.reset_slack_preferred_skin_tone();
        }
        let workspace_changed = match (&input.workspace, &self.slack_workspace) {
            (None, None) => false,
            (Some(next), Some(current)) => slack_workspace_changed(next, current),
            _ => true,
        };
        if workspace_changed {
            if let Some(workspace) = input.workspace {
                self.apply_slack_workspace(workspace, cx);
            } else {
                self.clear_slack_workspace_input(cx);
            }
        }
        self.sync_slack_members_context(cx);
        self.sync_slack_channel_notification_preference(cx);
        self.sync_slack_preferred_skin_tone(cx);
        self.sync_slack_remote_draft_hydration(cx);
        self.ensure_slack_realtime_subscription(cx);
    }

    fn apply_slack_message_capability_downgrades(&mut self, cx: &mut Context<Self>) {
        if self.slack_message_forward_modal.is_some()
            && (!self.slack_workspace_api_capabilities.forward_message
                || !self
                    .slack_workspace_api_capabilities
                    .load_destination_directory)
        {
            self.reset_slack_message_forward_context();
            cx.notify();
        }
        if (self.slack_message_edit.is_some()
            && !self.slack_workspace_api_capabilities.update_message)
            || (self.slack_message_delete_modal.is_some()
                && !self.slack_workspace_api_capabilities.delete_message)
        {
            self.reset_slack_message_mutation_context();
            self.slack_message_list_state.remeasure();
            cx.notify();
        }
    }

    fn apply_slack_rail_capability_downgrades(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route == SlackMainRoute::Directory
            && !self
                .slack_workspace_api_capabilities
                .load_destination_directory
        {
            self.leave_slack_directory(cx);
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
        if self.slack_main_route == SlackMainRoute::AllThreads
            && !self.slack_workspace_api_capabilities.load_all_threads
        {
            self.leave_slack_all_threads();
            self.slack_main_route = SlackMainRoute::Conversation;
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
        if self.slack_active_rail_view == SlackRailView::Activity
            && !self.slack_workspace_api_capabilities.load_activity
        {
            self.leave_slack_activity(cx);
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
        if self.slack_active_rail_view == SlackRailView::Later
            && !self.slack_workspace_api_capabilities.load_later
        {
            self.leave_slack_later();
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
        if self.slack_active_rail_view == SlackRailView::Files
            && !self.slack_workspace_api_capabilities.load_files
        {
            self.leave_slack_files();
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
        if self.slack_active_rail_view == SlackRailView::DraftsSent
            && !self.slack_workspace_api_capabilities.load_drafts_sent
        {
            self.leave_slack_drafts_sent();
            self.slack_active_rail_view = SlackRailView::Home;
            cx.notify();
        }
    }

    fn apply_slack_tab_capability_downgrades(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_tab == SlackMainTab::Canvas
            && !self.slack_workspace_api_capabilities.load_canvas
        {
            self.slack_active_tab = SlackMainTab::Messages;
            cx.notify();
        }
        if self.slack_active_tab == SlackMainTab::Pins
            && !self.slack_workspace_api_capabilities.load_pins
        {
            self.slack_active_tab = SlackMainTab::Messages;
            cx.notify();
        }
        if self.slack_active_tab == SlackMainTab::FilesLinks
            && !self
                .slack_workspace_api_capabilities
                .load_conversation_files
        {
            self.leave_slack_conversation_files();
            self.slack_active_tab = SlackMainTab::Messages;
            cx.notify();
        }
        if self.slack_active_tab == SlackMainTab::BookmarkFolder
            && !self.slack_workspace_api_capabilities.load_bookmark_folder
        {
            self.reset_slack_bookmark_folder_context();
            cx.notify();
        }
    }

    fn clear_slack_workspace_input(&mut self, cx: &mut Context<Self>) {
        let cleared_team_id = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone());
        self.leave_slack_directory(cx);
        self.reset_slack_realtime_subscription();
        self.slack_presence_authority.clear();
        self.slack_initial_refresh.invalidate();
        self.invalidate_slack_message_history_requests();
        self.discard_slack_drafts_for_workspace_identity_change(cx);
        self.slack_shell_snapshot = None;
        self.slack_sidebar_snapshot = None;
        self.slack_sidebar_reveal = None;
        self.reset_slack_home_finder();
        self.clear_slack_dm_inbox();
        self.reset_slack_all_threads_context();
        self.reset_slack_directory_context();
        self.reset_slack_activity_context(cx);
        self.reset_slack_later_context();
        self.reset_slack_files_context();
        self.reset_slack_conversation_files_context();
        self.reset_slack_bookmark_folder_context();
        self.reset_slack_drafts_sent_context();
        self.slack_members_deferred_conversation_id = None;
        self.reset_slack_members_context();
        self.slack_conversation_snapshot = None;
        self.slack_workspace = None;
        if let Some(team_id) = cleared_team_id {
            self.clear_slack_dock_badge(team_id, cx);
        }
        self.reset_slack_remote_draft_hydration();
        self.reset_slack_reaction_context();
        self.reset_slack_reaction_catalog();
        self.reset_slack_preferred_skin_tone();
        self.reset_slack_message_action_context();
        self.reset_slack_thread_context();
        self.refresh_slack_message_rows();
        self.refresh_slack_sidebar_rows();
        self.slack_remote_images.clear();
    }
}
