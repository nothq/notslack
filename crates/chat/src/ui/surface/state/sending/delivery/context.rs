use std::collections::HashSet;

use crate::ui::surface::{
    SlackActiveMainComposerContext, SlackMainRoute, SlackMainTab, SlackRailView,
    SlackSendDraftSource, SlackSendRequest, SurfaceState,
};
use crate::ui::Context;

use super::super::super::SlackVisiblePersonRow;

impl SurfaceState {
    pub(crate) fn has_current_slack_send_target(&self) -> bool {
        self.slack_workspace_api_capabilities.send_message
            && self.current_slack_send_target_id().is_some()
    }

    pub(crate) fn can_mutate_current_slack_send_draft(&self) -> bool {
        !self.slack_schedule_blocks_current_composer_mutation()
            && self.has_current_slack_send_target()
    }

    pub(crate) fn slack_schedule_blocks_current_composer_mutation(&self) -> bool {
        self.slack_schedule_pending.is_some()
            || self.has_slack_composer_schedule_recovery_for_active_owner()
    }

    pub(super) fn current_slack_send_target_id(&self) -> Option<&str> {
        let context = self.slack_active_main_composer_context.as_ref()?;
        if !self.slack_main_composer_context_is_visible(context) {
            return None;
        }
        match (
            &context.source,
            self.slack_pending_conversation_id.as_deref(),
        ) {
            (SlackSendDraftSource::Activity { .. }, _) => Some(&context.target.conversation_id),
            (_, Some(requested_conversation_id))
                if requested_conversation_id != context.target.conversation_id =>
            {
                None
            }
            _ => Some(&context.target.conversation_id),
        }
    }

    fn slack_main_composer_context_is_visible(
        &self,
        context: &SlackActiveMainComposerContext,
    ) -> bool {
        match &context.source {
            SlackSendDraftSource::Conversation { .. } => {
                self.slack_main_route == SlackMainRoute::Conversation
                    && matches!(
                        self.slack_active_rail_view,
                        SlackRailView::Home | SlackRailView::Dms
                    )
                    && self.slack_active_tab == SlackMainTab::Messages
                    && !self.slack_search_results_open
            }
            SlackSendDraftSource::NewMessage { .. } => {
                self.slack_main_route == SlackMainRoute::NewMessage
                    && self.slack_active_rail_view == SlackRailView::Home
            }
            SlackSendDraftSource::Activity { .. } => {
                self.slack_active_rail_view == SlackRailView::Activity
                    && self.slack_activity_detail.composer() == Some(context)
            }
        }
    }

    pub(super) fn slack_send_request_identity_is_current(
        &self,
        request: &SlackSendRequest,
    ) -> bool {
        self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == request.team_id
                && workspace.self_user_id.as_deref() == Some(request.self_user_id.as_str())
        })
    }

    pub(super) fn slack_send_request_workspace_is_current(
        &self,
        request: &SlackSendRequest,
    ) -> bool {
        self.slack_send_request_identity_is_current(request)
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.conversation_id == request.conversation_id)
    }

    fn slack_send_request_source_is_current(&self, request: &SlackSendRequest) -> bool {
        self.slack_active_main_composer_context
            .as_ref()
            .is_some_and(|context| context.source == request.draft_source)
    }

    pub(super) fn slack_send_request_target_is_current(&self, request: &SlackSendRequest) -> bool {
        self.slack_send_request_identity_is_current(request)
            && self.slack_send_request_source_is_current(request)
            && self.current_slack_send_target_id() == Some(request.conversation_id.as_str())
    }

    pub(super) fn fail_slack_send<T>(
        &mut self,
        message: &str,
        focus_composer: bool,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        self.slack_error = Some(message.to_string());
        self.slack_composer_focused = focus_composer;
        cx.notify();
        None
    }

    pub(in crate::ui::surface::state) fn slack_visible_people_rows(
        &self,
    ) -> Vec<SlackVisiblePersonRow> {
        let Some(workspace) = self.slack_workspace() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        let mut seen = HashSet::new();
        let mut push_row = |label: String,
                            user_id: Option<&str>,
                            accessory: Option<String>,
                            detail: Option<String>| {
            if label.trim().is_empty() {
                return;
            }
            let Some(user_id) = user_id.filter(|user_id| !user_id.trim().is_empty()) else {
                return;
            };
            if seen.insert(user_id.to_string()) {
                rows.push(SlackVisiblePersonRow {
                    user_id: user_id.to_string(),
                    label,
                    accessory,
                    detail,
                });
            }
        };

        let (display_name, user_id, avatar_label, _) = self.slack_self_identity();
        push_row(
            display_name,
            user_id.as_deref(),
            avatar_label,
            Some("Signed in as you.".to_string()),
        );

        for item in workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
        {
            if item.user_id.is_some() {
                push_row(
                    item.label.clone(),
                    item.user_id.as_deref(),
                    item.icon.clone(),
                    Some("Visible in the current workspace sidebar.".to_string()),
                );
            }
        }
        for message in &workspace.messages {
            push_row(
                message.author.clone(),
                message.user_id.as_deref(),
                message.avatar_label.clone(),
                Some("Visible in the active conversation.".to_string()),
            );
        }
        rows
    }
}
