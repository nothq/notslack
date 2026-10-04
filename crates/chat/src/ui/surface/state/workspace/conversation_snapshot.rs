use super::{
    Context, PreparedSlackConversationSnapshot, SlackConversationLiveTarget,
    SlackMessageListPosition, SlackWorkspaceUiIdentity, SurfaceState,
};

struct SlackConversationSnapshotActivation {
    same_conversation: bool,
    message_list_position: SlackMessageListPosition,
    team_id: String,
    self_user_id: Option<String>,
    conversation_id: String,
    composer_draft_text: String,
    read_target: SlackConversationLiveTarget,
    last_read: Option<crate::ui::SlackLastReadTimestamp>,
    last_read_boundary_loaded: bool,
    matching_sidebar: Option<crate::ui::SlackSidebarSnapshot>,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_prepared_slack_conversation_snapshot(
        &mut self,
        prepared: PreparedSlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) {
        let PreparedSlackConversationSnapshot {
            snapshot,
            message_rows,
            message_rows_local_today,
            message_chunks,
            remote_images,
            authored_client_message_ids,
        } = prepared;
        if self
            .slack_workspace
            .as_ref()
            .is_some_and(|workspace| workspace.team_id != snapshot.team_id)
        {
            return;
        }
        let activation = self.prepare_slack_conversation_snapshot_activation(&snapshot, cx);
        let observed_team_id = activation.team_id.clone();
        let observed_conversation_id = activation.conversation_id.clone();
        self.slack_remote_images.extend(remote_images);
        self.slack_message_rows = message_rows;
        self.slack_message_rows_local_today = Some(message_rows_local_today);
        self.slack_message_chunks = message_chunks;
        if activation.same_conversation {
            self.reconcile_slack_thread_parent();
        }
        self.slack_conversation_snapshot = Some(snapshot.clone());
        if !self.apply_slack_conversation_snapshot_to_workspace(snapshot, activation, cx) {
            self.install_staged_slack_workspace(cx);
        }
        self.reconcile_slack_observed_client_messages(
            &observed_team_id,
            &observed_conversation_id,
            &authored_client_message_ids,
            cx,
        );
    }

    fn prepare_slack_conversation_snapshot_activation(
        &mut self,
        snapshot: &crate::ui::SlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) -> SlackConversationSnapshotActivation {
        let read_target = SlackConversationLiveTarget {
            team_id: snapshot.team_id.clone(),
            conversation_id: snapshot.conversation_id.clone(),
        };
        let last_read = snapshot.last_read.clone();
        let last_read_boundary_loaded = snapshot.last_read_boundary_loaded;
        self.pause_slack_conversation_read_for_live_positioning();
        self.advance_slack_conversation_revision();
        self.invalidate_slack_message_history_requests();
        let same_conversation = self
            .slack_workspace
            .as_ref()
            .is_some_and(|workspace| workspace.conversation_id == snapshot.conversation_id);
        if !same_conversation {
            self.store_current_slack_conversation_draft(cx);
        }
        let message_list_position = self.capture_slack_message_list_position();
        let team_id = snapshot.team_id.clone();
        let self_user_id = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_user_id.clone());
        let conversation_id = snapshot.conversation_id.clone();
        let composer_draft_text = snapshot.composer_draft_text.clone().unwrap_or_default();
        let matching_sidebar = self
            .slack_sidebar_snapshot
            .as_ref()
            .filter(|sidebar| sidebar.conversation_id == conversation_id)
            .cloned();
        SlackConversationSnapshotActivation {
            same_conversation,
            message_list_position,
            team_id,
            self_user_id,
            conversation_id,
            composer_draft_text,
            read_target,
            last_read,
            last_read_boundary_loaded,
            matching_sidebar,
        }
    }

    fn apply_slack_conversation_snapshot_to_workspace(
        &mut self,
        snapshot: crate::ui::SlackConversationSnapshot,
        activation: SlackConversationSnapshotActivation,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(workspace) = self.slack_workspace_mut() else {
            return false;
        };
        if !activation.same_conversation {
            for item in workspace
                .sections
                .iter_mut()
                .flat_map(|section| section.items.iter_mut())
            {
                item.active = item.target_id == activation.conversation_id;
            }
        }
        let mut resolved_attachment_channel_labels =
            workspace.apply_conversation_snapshot(snapshot);
        if let Some(sidebar) = activation.matching_sidebar {
            resolved_attachment_channel_labels |= workspace.apply_sidebar_snapshot(sidebar);
        }
        if resolved_attachment_channel_labels {
            self.refresh_slack_message_rows();
        }
        self.rebuild_slack_local_delivery_rows();
        self.sync_slack_conversation_metadata(&activation.conversation_id, false);
        self.sync_current_slack_conversation_history();
        self.record_slack_conversation_applied(&activation.conversation_id);
        self.sync_slack_message_list_state(
            activation.same_conversation,
            activation.message_list_position,
        );
        self.install_slack_conversation_read_after_live_positioning(
            activation.read_target,
            activation.last_read.as_ref(),
            activation.last_read_boundary_loaded,
            cx,
        );
        self.rebuild_slack_remote_image_queue();
        self.queue_slack_dm_visible_images_for_current_view(cx);
        self.prefetch_slack_visible_images(cx);
        self.activate_current_slack_conversation_history_tab(cx);
        if activation.same_conversation {
            self.slack_error = None;
            cx.notify();
        } else {
            self.reset_slack_workspace_ui_state(
                SlackWorkspaceUiIdentity {
                    team_id: &activation.team_id,
                    self_user_id: activation.self_user_id.as_deref(),
                    conversation_id: &activation.conversation_id,
                },
                activation.composer_draft_text,
                cx,
            );
        }
        true
    }
}
