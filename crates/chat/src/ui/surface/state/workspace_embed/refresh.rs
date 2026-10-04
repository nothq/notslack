mod cache;
mod conversation;
mod live;

use super::{
    report_chat_connection_error, Arc, ChatStartup, Context, SlackInitialWorkspaceRefresh,
    SlackMessageNavigationRequest, SurfaceState, WorkspaceApi,
};
use crate::ui::surface::{SlackConversationRefreshRequest, SlackInitialRefreshIdentity};

#[derive(Clone)]
struct SlackInitialStageContext {
    identity: SlackInitialRefreshIdentity,
    workspace_api: Arc<dyn WorkspaceApi>,
    profile_enabled: bool,
}

struct SlackInitialSidebarStage {
    context: SlackInitialStageContext,
    collapsed_sections: std::collections::HashSet<String>,
    muted_conversations: std::collections::HashSet<String>,
}

struct SlackInitialConversationStage {
    context: SlackInitialStageContext,
    initial_message_navigation: Option<SlackMessageNavigationRequest>,
    refresh_request: SlackConversationRefreshRequest,
}

struct SlackInitialConversationCompletion {
    identity: SlackInitialRefreshIdentity,
    refresh_request: SlackConversationRefreshRequest,
    navigation: Option<SlackMessageNavigationRequest>,
}

impl SurfaceState {
    pub(super) fn refresh_initial_slack_workspace(
        &mut self,
        refresh: SlackInitialWorkspaceRefresh,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialWorkspaceRefresh {
            team_id,
            conversation_id,
            workspace_api,
            initial_message_navigation,
        } = refresh;
        let identity = self.slack_initial_refresh.begin(team_id, conversation_id);
        let conversation_refresh_request = self.begin_initial_slack_conversation_refresh(
            identity.team_id(),
            identity.conversation_id(),
        );
        self.record_slack_activation_live_workspace_preparation_started(identity.conversation_id());
        let profile_enabled = self.slack_activation_load_profile.is_some();
        let collapsed_sections = self.slack_collapsed_sections.clone();
        let muted_conversations = self.slack_muted_conversations.clone();
        let context = SlackInitialStageContext {
            identity,
            workspace_api,
            profile_enabled,
        };
        self.record_slack_activation_initial_refresh_started(&context.identity);
        self.spawn_initial_slack_cached_workspace_stage(
            context.clone(),
            collapsed_sections.clone(),
            muted_conversations.clone(),
            cx,
        );
        self.spawn_initial_slack_cached_dm_stage(context.clone(), cx);
        self.spawn_initial_slack_shell_stage(context.clone(), cx);
        self.spawn_initial_slack_sidebar_stage(
            SlackInitialSidebarStage {
                context: context.clone(),
                collapsed_sections,
                muted_conversations,
            },
            cx,
        );
        self.spawn_initial_slack_dm_stage(context.clone(), cx);
        self.spawn_initial_slack_conversation_stage(
            SlackInitialConversationStage {
                context,
                initial_message_navigation,
                refresh_request: conversation_refresh_request,
            },
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn initial_slack_refresh_is_current(
        &self,
        identity: &SlackInitialRefreshIdentity,
    ) -> bool {
        self.slack_initial_refresh.is_current(identity)
            && self
                .slack_pending_conversation_id
                .as_deref()
                .is_none_or(|pending| pending == identity.conversation_id())
            && self
                .slack_conversation_id()
                .is_none_or(|current| current == identity.conversation_id())
    }

    pub(in crate::ui::surface::state) fn finish_initial_slack_activation_if_visible(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
    ) {
        if self.initial_slack_refresh_is_current(identity)
            && self.slack_conversation_id() == Some(identity.conversation_id())
        {
            self.record_slack_activation_live_workspace_applied(identity.conversation_id());
        }
    }

    pub(in crate::ui::surface::state) fn apply_chat_connection_error(
        &mut self,
        request: &crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let (connection_api, previous_connection) = match &self.chat_startup {
            ChatStartup::Loading {
                connection_api,
                request: pending_request,
                generation: pending_generation,
                previous_connection,
                ..
            } if pending_request == request && *pending_generation == generation => {
                (connection_api.clone(), previous_connection.clone())
            }
            _ => return,
        };
        report_chat_connection_error(&error);
        self.record_slack_activation_connection_failed("connection_failed");
        self.chat_startup = ChatStartup::Error {
            connection_api,
            request: request.clone(),
            generation,
            previous_connection,
            error,
        };
        cx.notify();
    }
}
