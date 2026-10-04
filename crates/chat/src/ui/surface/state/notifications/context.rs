use crate::ui::surface::{
    SlackActivityDetailState, SlackActivityDetailTarget, SlackMainRoute, SlackMainTab,
    SlackRailView, SurfaceState,
};
use crate::ui::{
    SlackMessageTimestamp, SlackNotificationTeamBadge, SlackNotificationVisibleTarget,
    SlackNotificationWindowContext,
};

impl SurfaceState {
    pub fn slack_notification_team_badge(&self) -> Option<SlackNotificationTeamBadge> {
        self.slack_workspace()
            .map(|workspace| SlackNotificationTeamBadge {
                team_id: workspace.team_id.clone(),
                count: workspace.rail_badges.activity.unwrap_or_default(),
            })
    }

    pub fn slack_notification_window_context(&self) -> Option<SlackNotificationWindowContext> {
        let workspace = self.slack_workspace()?;
        let visible_targets = if self.active && !self.slack_notification_content_is_obscured() {
            self.slack_notification_visible_targets(workspace)
        } else {
            Vec::new()
        };
        Some(SlackNotificationWindowContext {
            team_id: workspace.team_id.clone(),
            active_surface: self.active,
            visible_targets,
        })
    }

    fn slack_notification_visible_targets(
        &self,
        workspace: &crate::ui::SlackWorkspace,
    ) -> Vec<SlackNotificationVisibleTarget> {
        if self.slack_main_route == SlackMainRoute::AllThreads {
            if self.slack_all_threads_team_id.as_deref() != Some(workspace.team_id.as_str()) {
                return Vec::new();
            }
            let (start, end) = self.slack_all_threads_visible_range;
            return self
                .slack_all_threads_rows
                .get(
                    start.min(self.slack_all_threads_rows.len())
                        ..end.min(self.slack_all_threads_rows.len()),
                )
                .unwrap_or_default()
                .iter()
                .map(|row| SlackNotificationVisibleTarget::Thread {
                    conversation_id: row.conversation_id.to_string(),
                    thread_timestamp: SlackMessageTimestamp::parse(&row.parent.id)
                        .expect("All Threads rows must retain a valid parent timestamp"),
                })
                .collect();
        }

        match self.slack_active_rail_view {
            SlackRailView::Home | SlackRailView::Dms
                if self.slack_main_route == SlackMainRoute::Conversation
                    && self.slack_active_tab == SlackMainTab::Messages =>
            {
                let mut targets = vec![SlackNotificationVisibleTarget::Conversation {
                    conversation_id: workspace.conversation_id.clone(),
                }];
                if let Some(panel) = self.slack_thread_panel.as_ref().filter(|panel| {
                    panel.origin.is_conversation()
                        && panel.conversation_id == workspace.conversation_id
                }) {
                    targets.push(SlackNotificationVisibleTarget::Thread {
                        conversation_id: panel.conversation_id.clone(),
                        thread_timestamp: SlackMessageTimestamp::parse(&panel.parent_message_id)
                            .expect(
                                "mounted Slack thread panel must retain a valid parent timestamp",
                            ),
                    });
                }
                targets
            }
            SlackRailView::Activity => self.slack_activity_notification_visible_targets(workspace),
            SlackRailView::Later => self.slack_later_notification_visible_targets(workspace),
            SlackRailView::Home
            | SlackRailView::Dms
            | SlackRailView::Files
            | SlackRailView::DraftsSent
            | SlackRailView::More
            | SlackRailView::Admin => Vec::new(),
        }
    }

    fn slack_activity_notification_visible_targets(
        &self,
        workspace: &crate::ui::SlackWorkspace,
    ) -> Vec<SlackNotificationVisibleTarget> {
        let SlackActivityDetailState::Loaded {
            message_timestamp,
            target,
            ..
        } = &self.slack_activity_detail
        else {
            return Vec::new();
        };
        let (team_id, conversation_id, thread_timestamp) = match target {
            SlackActivityDetailTarget::Conversation { composer } => (
                composer.target.team_id.as_str(),
                composer.target.conversation_id.as_str(),
                None,
            ),
            SlackActivityDetailTarget::Thread {
                team_id,
                conversation_id,
                thread_timestamp,
                ..
            } => (
                team_id.as_str(),
                conversation_id.as_str(),
                Some(thread_timestamp),
            ),
        };
        if team_id != workspace.team_id {
            return Vec::new();
        }
        let mut targets = vec![SlackNotificationVisibleTarget::Message {
            conversation_id: conversation_id.to_string(),
            message_timestamp: message_timestamp.clone(),
        }];
        if let Some(thread_timestamp) = thread_timestamp {
            targets.push(SlackNotificationVisibleTarget::Thread {
                conversation_id: conversation_id.to_string(),
                thread_timestamp: thread_timestamp.clone(),
            });
        }
        targets
    }

    fn slack_later_notification_visible_targets(
        &self,
        workspace: &crate::ui::SlackWorkspace,
    ) -> Vec<SlackNotificationVisibleTarget> {
        if self.slack_later_team_id.as_deref() != Some(workspace.team_id.as_str()) {
            return Vec::new();
        }
        let Some(target) = self
            .selected_slack_later_row()
            .and_then(|row| row.thread_target())
        else {
            return Vec::new();
        };
        let Some(panel) = self.slack_thread_panel.as_ref().filter(|panel| {
            !panel.origin.is_conversation()
                && panel.origin.later_item_key() == Some(&target.item_key)
                && panel.conversation_id == target.conversation_id
                && panel.parent_message_id == target.thread_timestamp
        }) else {
            return Vec::new();
        };
        vec![SlackNotificationVisibleTarget::Thread {
            conversation_id: panel.conversation_id.clone(),
            thread_timestamp: SlackMessageTimestamp::parse(&panel.parent_message_id)
                .expect("Later thread panel must retain a valid parent timestamp"),
        }]
    }

    fn slack_notification_content_is_obscured(&self) -> bool {
        self.slack_search_open
            || self.slack_search_results_open
            || self.slack_profile_panel.is_some()
            || self.slack_dms_peek_visible
            || self.slack_expanded_attachment.is_some()
            || self.slack_rail_menu.is_some()
            || self.slack_history_menu_open
            || self.slack_conversation_tabs_overflow_open
            || self.slack_channel_move_menu_open
            || self.slack_channel_notifications_menu_open
            || self.slack_channel_menu_open
            || self.slack_message_menu.is_some()
            || self.slack_date_jump_overlay.is_some()
            || self.slack_message_forward_modal.is_some()
            || self.slack_message_delete_modal.is_some()
            || self.slack_channel_details_open
            || self.slack_schedule_overlay.is_some()
            || self.slack_members_panel_open
            || self.slack_composer_link_dialog.is_some()
            || self.slack_video_clip_modal.is_some()
    }
}
