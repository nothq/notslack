use super::{
    slack_sidebar_layout_changed, Arc, Context, Image, PreparedSlackWorkspace, SlackMainRoute,
    SlackRailView, SlackWorkspace, SlackWorkspaceUiIdentity, SlackWorkspaceViewReactivation,
    SurfaceState,
};

struct PreparedSlackWorkspaceActivation {
    team_id: String,
    self_user_id: Option<String>,
    conversation_id: String,
    composer_draft_text: String,
    dm_item_count: usize,
    sidebar_layout_changed: bool,
    reveal_active_sidebar_row: bool,
    view_reactivation: SlackWorkspaceViewReactivation,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_prepared_slack_workspace(
        &mut self,
        prepared: PreparedSlackWorkspace,
        cx: &mut Context<Self>,
    ) {
        self.slack_initial_refresh.invalidate();
        self.apply_prepared_slack_workspace_state(prepared, cx);
    }

    fn apply_prepared_slack_workspace_state(
        &mut self,
        mut prepared: PreparedSlackWorkspace,
        cx: &mut Context<Self>,
    ) {
        self.slack_presence_authority
            .overlay_prepared_workspace(&mut prepared);
        let reveal_active_sidebar_row =
            self.slack_workspace_conversation_changed(&prepared.workspace.conversation_id);
        if self.slack_workspace.as_deref() == Some(&prepared.workspace) {
            return;
        }
        let activation =
            self.install_prepared_slack_workspace_state(prepared, reveal_active_sidebar_row, cx);
        self.activate_prepared_slack_workspace(activation, cx);
    }

    fn install_prepared_slack_workspace_state(
        &mut self,
        prepared: PreparedSlackWorkspace,
        reveal_active_sidebar_row: bool,
        cx: &mut Context<Self>,
    ) -> PreparedSlackWorkspaceActivation {
        let PreparedSlackWorkspace {
            workspace,
            message_rows,
            message_rows_local_today,
            message_chunks,
            sidebar_rows,
            remote_images,
            active_conversation_was_muted,
        } = prepared;
        self.prepare_slack_drafts_for_workspace(&workspace, cx);
        let message_list_position = self.capture_slack_message_list_position();
        self.advance_slack_conversation_revision();
        self.invalidate_slack_message_history_requests();
        let view_reactivation = self.prepare_slack_workspace_view_reactivation(&workspace, cx);
        self.install_slack_workspace_snapshots(&workspace);
        let same_conversation = self
            .slack_workspace
            .as_ref()
            .is_some_and(|previous| previous.conversation_id == workspace.conversation_id);
        let sidebar_layout_changed =
            slack_sidebar_layout_changed(&self.slack_sidebar_rows, &sidebar_rows);
        self.sync_slack_bookmark_folder_context(&workspace);
        self.sync_slack_workspace_metadata(&workspace, active_conversation_was_muted);
        self.merge_slack_workspace_remote_images(remote_images);
        let team_id = workspace.team_id.clone();
        let self_user_id = workspace.self_user_id.clone();
        let conversation_id = workspace.conversation_id.clone();
        self.sync_slack_reaction_catalog_team(&team_id);
        self.record_slack_conversation_applied(&conversation_id);
        let composer_draft_text = workspace.composer_draft_text.clone().unwrap_or_default();
        let dm_item_count = self.slack_dm_inbox_item_count(&workspace);
        self.slack_workspace = Some(Arc::new(workspace));
        self.slack_message_rows = message_rows;
        self.slack_message_rows_local_today = Some(message_rows_local_today);
        self.slack_message_chunks = message_chunks;
        self.rebuild_slack_local_delivery_rows();
        self.slack_sidebar_rows = self.project_slack_conversation_read_sidebar_rows(sidebar_rows);
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_workspace(data);
        }
        self.sync_slack_message_list_state(same_conversation, message_list_position);
        PreparedSlackWorkspaceActivation {
            team_id,
            self_user_id,
            conversation_id,
            composer_draft_text,
            dm_item_count,
            sidebar_layout_changed,
            reveal_active_sidebar_row,
            view_reactivation,
        }
    }

    fn install_slack_workspace_snapshots(&mut self, workspace: &SlackWorkspace) {
        self.slack_shell_snapshot = Some(workspace.shell_snapshot());
        self.slack_sidebar_snapshot = Some(workspace.sidebar_snapshot());
        self.slack_conversation_snapshot = Some(workspace.conversation_snapshot());
    }

    fn activate_prepared_slack_workspace(
        &mut self,
        activation: PreparedSlackWorkspaceActivation,
        cx: &mut Context<Self>,
    ) {
        self.sync_applied_slack_sidebar_rows(activation.sidebar_layout_changed);
        if activation.reveal_active_sidebar_row {
            self.queue_slack_sidebar_active_row_reveal(&activation.conversation_id);
        }
        if self.slack_dm_list_state.item_count() != activation.dm_item_count {
            self.slack_dm_list_state.reset(activation.dm_item_count);
        }
        self.rebuild_slack_remote_image_queue();
        self.queue_slack_dm_visible_images_for_current_view(cx);
        self.prefetch_slack_visible_images(cx);
        self.reset_slack_workspace_ui_state(
            SlackWorkspaceUiIdentity {
                team_id: &activation.team_id,
                self_user_id: activation.self_user_id.as_deref(),
                conversation_id: &activation.conversation_id,
            },
            activation.composer_draft_text,
            cx,
        );
        self.reconcile_slack_current_conversation_outbound_deliveries(cx);
        self.sync_slack_members_context(cx);
        self.reactivate_slack_workspace_view(activation.view_reactivation, cx);
        self.activate_current_slack_conversation_history_tab(cx);
        self.emit_slack_dock_badge(cx);
    }

    fn slack_workspace_conversation_changed(&self, conversation_id: &str) -> bool {
        self.slack_workspace
            .as_ref()
            .is_none_or(|current| current.conversation_id != conversation_id)
    }

    fn merge_slack_workspace_remote_images(
        &mut self,
        remote_images: std::collections::HashMap<String, Arc<Image>>,
    ) {
        let mut merged = std::mem::take(&mut self.slack_remote_images);
        merged.extend(remote_images);
        self.slack_remote_images = merged;
    }

    fn prepare_slack_workspace_view_reactivation(
        &mut self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> SlackWorkspaceViewReactivation {
        let all_threads_team_changed = self
            .slack_all_threads_team_id
            .as_deref()
            .is_some_and(|team_id| team_id != workspace.team_id.as_str());
        let reactivate_all_threads =
            self.slack_main_route == SlackMainRoute::AllThreads && all_threads_team_changed;
        if all_threads_team_changed {
            self.reset_slack_all_threads_context();
        }
        let activity_team_changed = self
            .slack_activity_team_id
            .as_deref()
            .is_some_and(|team_id| team_id != workspace.team_id.as_str());
        let reactivate_activity =
            self.slack_active_rail_view == SlackRailView::Activity && activity_team_changed;
        if activity_team_changed {
            self.reset_slack_activity_context(cx);
        }
        let later_team_changed = self
            .slack_later_team_id
            .as_deref()
            .is_some_and(|team_id| team_id != workspace.team_id.as_str());
        let reactivate_later =
            self.slack_active_rail_view == SlackRailView::Later && later_team_changed;
        if later_team_changed {
            self.reset_slack_later_context();
        }
        let (
            reactivate_files,
            reactivate_drafts_sent,
            directory_team_changed,
            new_message_team_changed,
        ) = self.prepare_slack_secondary_workspace_view_reactivation(workspace, cx);
        SlackWorkspaceViewReactivation {
            all_threads: reactivate_all_threads,
            activity: reactivate_activity,
            later: reactivate_later,
            files: reactivate_files,
            drafts_sent: reactivate_drafts_sent,
            directory: directory_team_changed,
            new_message: new_message_team_changed,
        }
    }

    fn prepare_slack_secondary_workspace_view_reactivation(
        &mut self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> (bool, bool, bool, bool) {
        let files_context_changed = self.slack_files_team_id.as_deref().is_some_and(|team_id| {
            team_id != workspace.team_id.as_str()
                || self.slack_files_self_user_id.as_deref() != workspace.self_user_id.as_deref()
        });
        let reactivate_files =
            self.slack_active_rail_view == SlackRailView::Files && files_context_changed;
        if files_context_changed {
            self.reset_slack_files_context();
        }
        let drafts_sent_context_changed =
            self.slack_drafts_sent_team_id
                .as_deref()
                .is_some_and(|team_id| {
                    team_id != workspace.team_id.as_str()
                        || self.slack_drafts_sent_self_user_id.as_deref()
                            != workspace.self_user_id.as_deref()
                });
        let reactivate_drafts_sent =
            self.slack_active_rail_view == SlackRailView::DraftsSent && drafts_sent_context_changed;
        if drafts_sent_context_changed {
            self.reset_slack_drafts_sent_context();
        }
        let directory_team_changed = self.slack_main_route == SlackMainRoute::Directory
            && self.slack_directory_team_id.as_deref() != Some(workspace.team_id.as_str());
        if directory_team_changed {
            self.reset_slack_directory_context();
        }
        let new_message_team_changed = self.slack_main_route == SlackMainRoute::NewMessage
            && self
                .slack_new_message_team_id
                .as_deref()
                .is_some_and(|team_id| team_id != workspace.team_id.as_str());
        if new_message_team_changed {
            self.reset_slack_new_message_after_workspace_change(cx);
        }
        (
            reactivate_files,
            reactivate_drafts_sent,
            directory_team_changed,
            new_message_team_changed,
        )
    }

    fn reset_slack_new_message_after_workspace_change(&mut self, cx: &mut Context<Self>) {
        self.reset_slack_new_message_team_context(cx);
        self.replace_slack_send_draft_document(Default::default());
        assert!(
            self.slack_composer_files.is_empty(),
            "workspace preparation must move or discard every active Slack composer file"
        );
    }

    pub(in crate::ui::surface::state) fn sync_applied_slack_sidebar_rows(
        &mut self,
        sidebar_layout_changed: bool,
    ) {
        if self.slack_sidebar_list_state.item_count() != self.slack_sidebar_rows.len() {
            self.slack_sidebar_list_state
                .reset(self.slack_sidebar_rows.len());
        } else if sidebar_layout_changed {
            self.slack_sidebar_list_state.remeasure();
        }
        if self.slack_home_finder_active() {
            self.rebuild_slack_home_finder_results();
        }
    }

    fn reactivate_slack_workspace_view(
        &mut self,
        reactivation: SlackWorkspaceViewReactivation,
        cx: &mut Context<Self>,
    ) {
        if reactivation.all_threads {
            if self.slack_workspace_api_capabilities.load_all_threads {
                self.activate_slack_all_threads(cx);
            } else {
                self.fallback_to_slack_home_after_workspace_change(cx);
            }
        }
        if reactivation.activity {
            self.reactivate_slack_activity_after_workspace_change(cx);
        }
        if reactivation.later {
            self.reactivate_slack_later_after_workspace_change(cx);
        }
        if reactivation.files {
            self.reactivate_slack_files_after_workspace_change(cx);
        }
        if reactivation.drafts_sent {
            self.reactivate_slack_drafts_sent_after_workspace_change(cx);
        }
        if reactivation.directory {
            if self
                .slack_workspace_api_capabilities
                .load_destination_directory
            {
                self.activate_slack_directory(cx);
            } else {
                self.leave_slack_directory(cx);
                self.fallback_to_slack_home_after_workspace_change(cx);
            }
        }
        if reactivation.new_message {
            self.activate_slack_new_message(cx);
        }
    }

    pub(in crate::ui::surface::state) fn reactivate_slack_activity_after_workspace_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_workspace_api_capabilities.load_activity {
            self.activate_slack_activity(cx);
        } else {
            self.fallback_to_slack_home_after_workspace_change(cx);
        }
    }

    pub(in crate::ui::surface::state) fn reactivate_slack_later_after_workspace_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_workspace_api_capabilities.load_later {
            self.activate_slack_later(cx);
        } else {
            self.fallback_to_slack_home_after_workspace_change(cx);
        }
    }

    pub(in crate::ui::surface::state) fn reactivate_slack_files_after_workspace_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_workspace_api_capabilities.load_files {
            self.activate_slack_files(cx);
        } else {
            self.fallback_to_slack_home_after_workspace_change(cx);
        }
    }

    pub(in crate::ui::surface::state) fn reactivate_slack_drafts_sent_after_workspace_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_workspace_api_capabilities.load_drafts_sent {
            self.activate_slack_drafts_sent(cx);
        } else {
            self.fallback_to_slack_home_after_workspace_change(cx);
        }
    }

    pub(in crate::ui::surface::state) fn fallback_to_slack_home_after_workspace_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_active_rail_view = SlackRailView::Home;
        cx.notify();
    }
}
