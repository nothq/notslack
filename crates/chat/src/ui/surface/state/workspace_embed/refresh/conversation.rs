use super::super::{
    load_initial_slack_conversation, prepare_initial_stage, Context,
    PreparedSlackConversationSnapshot, SlackInitialStageLoadResult, SurfaceState,
};
use super::{
    SlackInitialConversationCompletion, SlackInitialConversationStage, SlackInitialStageContext,
};
use crate::ui::surface::{
    SlackConversationRefreshRequest, SlackInitialRefreshIdentity, SlackMessageNavigationRequest,
};

impl SurfaceState {
    pub(super) fn spawn_initial_slack_conversation_stage(
        &mut self,
        stage: SlackInitialConversationStage,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialConversationStage {
            context:
                SlackInitialStageContext {
                    identity,
                    workspace_api,
                    profile_enabled,
                },
            initial_message_navigation,
            refresh_request,
        } = stage;
        let conversation_anchor = initial_message_navigation
            .as_ref()
            .map(|request| request.target.anchor_timestamp().clone());
        let completion = SlackInitialConversationCompletion {
            identity: identity.clone(),
            refresh_request,
            navigation: initial_message_navigation,
        };
        let load_identity = identity;
        self.spawn_background_task(
            load_identity.conversation_id().to_string(),
            cx,
            move |conversation_id| {
                prepare_initial_stage(profile_enabled, || {
                    load_initial_slack_conversation(
                        workspace_api.as_ref(),
                        load_identity.team_id(),
                        &conversation_id,
                        conversation_anchor.as_ref(),
                    )
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_conversation_stage(completion, load, cx);
            },
        );
    }

    fn finish_initial_slack_conversation_stage(
        &mut self,
        completion: SlackInitialConversationCompletion,
        load: SlackInitialStageLoadResult<PreparedSlackConversationSnapshot>,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialConversationCompletion {
            identity,
            refresh_request,
            navigation,
        } = completion;
        if !self.initial_slack_conversation_stage_is_current(
            &identity,
            &refresh_request,
            navigation.as_ref(),
            cx,
        ) {
            return;
        }
        if let Some(prepared_at) = load.prepared_at {
            self.record_slack_activation_live_workspace_prepared(
                identity.conversation_id(),
                prepared_at,
            );
        }
        self.record_slack_activation_live_stage_prepared(
            identity.conversation_id(),
            "conversation",
            load.prepared_at,
        );
        match load.result {
            Ok(prepared) => {
                if !self.apply_initial_slack_conversation(
                    &identity,
                    navigation.as_ref(),
                    prepared,
                    cx,
                ) {
                    return;
                }
            }
            Err(error) => {
                self.fail_initial_slack_conversation(&identity, navigation.as_ref(), error, cx);
            }
        }
        self.finish_initial_slack_conversation_refresh(&refresh_request, cx);
        self.resume_slack_members_after_initial_conversation(identity.conversation_id(), cx);
    }

    fn apply_initial_slack_conversation(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
        navigation: Option<&SlackMessageNavigationRequest>,
        prepared: PreparedSlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .slack_initial_refresh
            .record_live_conversation(identity)
        {
            return false;
        }
        self.apply_prepared_slack_conversation_snapshot(prepared, cx);
        if navigation.is_some() {
            self.continue_slack_message_navigation(cx);
        }
        self.record_slack_activation_live_stage_applied(identity.conversation_id(), "conversation");
        self.finish_initial_slack_activation_if_visible(identity);
        true
    }

    fn fail_initial_slack_conversation(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
        navigation: Option<&SlackMessageNavigationRequest>,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.record_slack_activation_live_stage_failed(identity.conversation_id(), "conversation");
        self.record_slack_activation_live_workspace_failed(identity.conversation_id());
        if navigation.is_some() {
            self.cancel_slack_message_navigation();
        }
        self.slack_error = Some(error);
        cx.notify();
    }

    fn initial_slack_conversation_stage_is_current(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
        refresh_request: &SlackConversationRefreshRequest,
        navigation: Option<&SlackMessageNavigationRequest>,
        cx: &mut Context<Self>,
    ) -> bool {
        let current = self.initial_slack_refresh_is_current(identity)
            && navigation
                .is_none_or(|request| self.slack_message_navigation.as_ref() == Some(request));
        if current {
            return true;
        }
        self.finish_initial_slack_conversation_refresh(refresh_request, cx);
        self.cancel_deferred_slack_members(identity.conversation_id());
        self.report_stale_initial_slack_refresh(identity, "live_conversation");
        false
    }
}
