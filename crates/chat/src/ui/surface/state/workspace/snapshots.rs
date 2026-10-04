use super::{
    slack_sidebar_layout_changed, Context, PreparedSlackShellSnapshot,
    PreparedSlackSidebarSnapshot, SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_prepared_slack_shell_snapshot(
        &mut self,
        prepared: PreparedSlackShellSnapshot,
        cx: &mut Context<Self>,
    ) {
        let PreparedSlackShellSnapshot {
            snapshot,
            remote_images,
        } = prepared;
        if self
            .slack_workspace
            .as_ref()
            .is_some_and(|workspace| workspace.team_id != snapshot.team_id)
        {
            return;
        }
        self.slack_presence_authority
            .sync_identity(&snapshot.team_id, snapshot.self_user_id.as_deref());
        let timezone_changed = self
            .slack_workspace
            .as_ref()
            .is_some_and(|workspace| workspace.self_timezone_id != snapshot.self_timezone_id);
        let message_list_position =
            timezone_changed.then(|| self.capture_slack_message_list_position());
        let timezone_id = snapshot.self_timezone_id.clone();
        self.slack_remote_images.extend(remote_images);
        self.slack_shell_snapshot = Some(snapshot.clone());
        if timezone_changed {
            if let Some(conversation) = self.slack_conversation_snapshot.as_mut() {
                conversation.self_timezone_id = timezone_id;
            }
        }
        if let Some(workspace) = self.slack_workspace_mut() {
            workspace.apply_shell_snapshot(snapshot);
            if let Some(message_list_position) = message_list_position {
                self.refresh_slack_message_rows();
                self.reconcile_slack_thread_parent();
                self.sync_slack_message_list_state(true, message_list_position);
            }
            self.rebuild_slack_remote_image_queue();
            self.queue_slack_dm_visible_images_for_current_view(cx);
            self.emit_slack_dock_badge(cx);
            cx.notify();
            return;
        }
        if !self.install_staged_slack_workspace(cx) {
            cx.notify();
        }
    }

    pub(in crate::ui::surface::state) fn apply_prepared_slack_sidebar_snapshot(
        &mut self,
        mut prepared: PreparedSlackSidebarSnapshot,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_sidebar_snapshot_applies(&prepared.snapshot) {
            return;
        }
        self.slack_presence_authority
            .overlay_prepared_sidebar(&mut prepared);
        let PreparedSlackSidebarSnapshot {
            snapshot,
            rows,
            remote_images,
            active_conversation_was_muted,
        } = prepared;
        let rows = self.project_slack_conversation_read_sidebar_rows(rows);
        self.slack_remote_images.extend(remote_images);
        self.install_prepared_slack_sidebar_rows(rows);
        self.refresh_slack_home_finder_after_sidebar_snapshot();
        self.install_slack_sidebar_snapshot_metadata(&snapshot, active_conversation_was_muted);
        if let Some(workspace) = self
            .slack_workspace_mut()
            .filter(|workspace| workspace.conversation_id == snapshot.conversation_id)
        {
            let resolved_attachment_channel_labels = workspace.apply_sidebar_snapshot(snapshot);
            if resolved_attachment_channel_labels {
                self.refresh_slack_message_rows();
            }
            {
                let (authority, data) = (&mut self.slack_presence_authority, &self.data);
                authority.reindex_sidebar(data);
            }
            self.rebuild_slack_remote_image_queue();
            self.queue_slack_dm_visible_images_for_current_view(cx);
            self.emit_slack_dock_badge(cx);
            cx.notify();
            return;
        }
        if self.slack_workspace.is_some() {
            {
                let (authority, data) = (&mut self.slack_presence_authority, &self.data);
                authority.reindex_sidebar(data);
            }
            cx.notify();
            return;
        }
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_sidebar(data);
        }
        if !self.install_staged_slack_workspace(cx) {
            cx.notify();
        }
    }

    fn refresh_slack_home_finder_after_sidebar_snapshot(&mut self) {
        if self.slack_home_finder_active() {
            self.rebuild_slack_home_finder_results();
        }
    }

    fn install_prepared_slack_sidebar_rows(
        &mut self,
        rows: std::sync::Arc<[crate::ui::surface::SlackSidebarRow]>,
    ) {
        let layout_changed = slack_sidebar_layout_changed(&self.slack_sidebar_rows, &rows);
        self.slack_sidebar_rows = rows;
        if self.slack_sidebar_list_state.item_count() != self.slack_sidebar_rows.len() {
            self.slack_sidebar_list_state
                .reset(self.slack_sidebar_rows.len());
        } else if layout_changed {
            self.slack_sidebar_list_state.remeasure();
        }
    }

    fn install_slack_sidebar_snapshot_metadata(
        &mut self,
        snapshot: &crate::ui::SlackSidebarSnapshot,
        active_conversation_was_muted: bool,
    ) {
        if let Some(reveal) = self
            .slack_sidebar_reveal
            .as_mut()
            .filter(|reveal| reveal.conversation_id == snapshot.conversation_id)
        {
            reveal.awaiting_live_sidebar = false;
            reveal.scheduled = false;
        }
        if active_conversation_was_muted {
            self.slack_muted_conversations
                .insert(snapshot.conversation_id.clone());
        }
        self.slack_sidebar_snapshot = Some(snapshot.clone());
        self.merge_slack_dm_sidebar_state();
    }

    fn slack_sidebar_snapshot_applies(&self, snapshot: &crate::ui::SlackSidebarSnapshot) -> bool {
        let team_matches = self
            .slack_workspace
            .as_ref()
            .is_none_or(|workspace| workspace.team_id == snapshot.team_id);
        let conversation_matches = self
            .slack_workspace
            .as_ref()
            .is_none_or(|workspace| workspace.conversation_id == snapshot.conversation_id)
            || self.slack_pending_conversation_id.as_deref()
                == Some(snapshot.conversation_id.as_str());
        team_matches && conversation_matches
    }
}
