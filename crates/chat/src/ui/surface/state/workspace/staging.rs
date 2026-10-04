use super::{
    Arc, Context, SlackConversationLiveTarget, SlackWorkspace, SlackWorkspaceUiIdentity,
    SurfaceState,
};

struct StagedSlackWorkspace {
    workspace: SlackWorkspace,
    awaiting_live_sidebar: bool,
    resolved_attachment_channel_labels: bool,
    read_target: SlackConversationLiveTarget,
    last_read: Option<crate::ui::SlackLastReadTimestamp>,
    last_read_boundary_loaded: bool,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn install_staged_slack_workspace(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(staged) = self.prepare_staged_slack_workspace() else {
            return false;
        };
        let StagedSlackWorkspace {
            mut workspace,
            awaiting_live_sidebar,
            resolved_attachment_channel_labels,
            read_target,
            last_read,
            last_read_boundary_loaded,
        } = staged;
        self.slack_presence_authority
            .overlay_workspace(&mut workspace);
        let conversation_id = workspace.conversation_id.clone();
        let team_id = workspace.team_id.clone();
        let self_user_id = workspace.self_user_id.clone();
        let composer_draft_text = workspace.composer_draft_text.clone().unwrap_or_default();
        let message_list_position = self.capture_slack_message_list_position();
        self.prepare_slack_drafts_for_workspace(&workspace, cx);
        self.advance_slack_conversation_revision();
        self.sync_slack_bookmark_folder_context(&workspace);
        self.sync_slack_workspace_metadata(&workspace, false);
        self.record_slack_conversation_applied(&conversation_id);
        self.slack_workspace = Some(Arc::new(workspace));
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_workspace(data);
        }
        if resolved_attachment_channel_labels {
            self.refresh_slack_message_rows();
        }
        self.sync_slack_message_list_state(false, message_list_position);
        self.install_slack_conversation_read_after_live_positioning(
            read_target,
            last_read.as_ref(),
            last_read_boundary_loaded,
            cx,
        );
        self.queue_slack_sidebar_active_row_reveal_for_stage(
            &conversation_id,
            awaiting_live_sidebar,
        );
        self.refresh_staged_slack_workspace_images(cx);
        self.reset_slack_workspace_ui_state(
            SlackWorkspaceUiIdentity {
                team_id: &team_id,
                self_user_id: self_user_id.as_deref(),
                conversation_id: &conversation_id,
            },
            composer_draft_text,
            cx,
        );
        self.sync_slack_members_context(cx);
        self.activate_current_slack_conversation_history_tab(cx);
        self.emit_slack_dock_badge(cx);
        true
    }

    fn refresh_staged_slack_workspace_images(&mut self, cx: &mut Context<Self>) {
        self.rebuild_slack_remote_image_queue();
        self.queue_slack_dm_visible_images_for_current_view(cx);
        self.prefetch_slack_visible_images(cx);
    }

    fn prepare_staged_slack_workspace(&self) -> Option<StagedSlackWorkspace> {
        let shell = self.slack_shell_snapshot.clone()?;
        let conversation = self.slack_conversation_snapshot.clone()?;
        if shell.team_id != conversation.team_id {
            return None;
        }
        let sidebar = self.slack_sidebar_snapshot.clone().filter(|sidebar| {
            sidebar.team_id == shell.team_id
                && sidebar.conversation_id == conversation.conversation_id
        });
        let awaiting_live_sidebar = sidebar.is_none();
        let mut workspace = SlackWorkspace::from_snapshots(shell, None, conversation);
        let read_target = SlackConversationLiveTarget {
            team_id: workspace.team_id.clone(),
            conversation_id: workspace.conversation_id.clone(),
        };
        let last_read = workspace.last_read.clone();
        let last_read_boundary_loaded = workspace.last_read_boundary_loaded;
        let resolved_attachment_channel_labels = sidebar
            .map(|sidebar| workspace.apply_sidebar_snapshot(sidebar))
            .unwrap_or(false);
        Some(StagedSlackWorkspace {
            workspace,
            awaiting_live_sidebar,
            resolved_attachment_channel_labels,
            read_target,
            last_read,
            last_read_boundary_loaded,
        })
    }
}
