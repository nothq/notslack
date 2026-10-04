use std::sync::Arc;

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    prepare_slack_channel_creator_hydration, prepare_slack_channel_details,
    PreparedSlackChannelCreatorHydration, PreparedSlackChannelDetails,
    SlackChannelCreatorLoadRequest, SlackChannelDetailsLoadRequest,
};

type SlackChannelDetailsLoadResult = Result<PreparedSlackChannelDetails, String>;
type SlackChannelCreatorLoadResult = Result<PreparedSlackChannelCreatorHydration, String>;

struct SlackChannelDetailsTarget {
    workspace_api: Arc<dyn WorkspaceApi>,
    team_id: String,
    conversation_id: String,
}

struct SlackChannelCreatorTarget {
    workspace_api: Arc<dyn WorkspaceApi>,
    team_id: String,
    conversation_id: String,
    user_id: String,
    created_date_label: Option<gpui::SharedString>,
}

impl SurfaceState {
    pub(crate) fn open_slack_channel_details(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.slack_channel_details_target() else {
            return;
        };
        let SlackChannelDetailsTarget {
            workspace_api,
            team_id,
            conversation_id,
        } = target;
        self.slack_channel_details_open = true;
        self.slack_channel_details_error = None;
        let key = (team_id.clone(), conversation_id.clone());
        if let Some(details) = self.slack_channel_details_cache.get(&key).cloned() {
            self.slack_channel_details = Some(details);
            self.slack_channel_details_request = None;
            self.slack_channel_details_loading = false;
            cx.notify();
            self.begin_slack_channel_creator_hydration(cx);
            return;
        }
        self.slack_channel_details = None;
        if let Some(generation) = self.slack_channel_details_inflight.get(&key).copied() {
            self.slack_channel_details_request = Some(SlackChannelDetailsLoadRequest {
                generation,
                team_id,
                conversation_id,
            });
            self.slack_channel_details_loading = true;
            cx.notify();
            return;
        }
        self.slack_channel_details_generation = self
            .slack_channel_details_generation
            .checked_add(1)
            .expect("Slack channel details request generation overflowed");
        let request = SlackChannelDetailsLoadRequest {
            generation: self.slack_channel_details_generation,
            team_id,
            conversation_id,
        };
        self.slack_channel_details_inflight
            .insert(key, request.generation);
        self.slack_channel_details_request = Some(request.clone());
        self.slack_channel_details_loading = true;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackChannelDetailsLoadRequest)| {
                let result = workspace_api
                    .load_slack_channel_details(&request.conversation_id)
                    .and_then(prepare_slack_channel_details);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_channel_details_load(request, result, cx);
            },
        );
    }

    fn slack_channel_details_target(&self) -> Option<SlackChannelDetailsTarget> {
        let workspace = self.slack_workspace()?;
        if !self.slack_workspace_api_capabilities.load_channel_details
            || !workspace.channel_kind.is_channel()
            || workspace.team_id.is_empty()
            || workspace.conversation_id.is_empty()
        {
            return None;
        }
        Some(SlackChannelDetailsTarget {
            workspace_api: self.active_slack_workspace_api()?,
            team_id: workspace.team_id.clone(),
            conversation_id: workspace.conversation_id.clone(),
        })
    }

    pub(crate) fn clear_slack_channel_details_view(&mut self) {
        self.slack_channel_details = None;
        self.slack_channel_details_request = None;
        self.slack_channel_details_loading = false;
        self.slack_channel_details_error = None;
        self.slack_channel_creator_request = None;
    }

    fn finish_slack_channel_details_load(
        &mut self,
        request: SlackChannelDetailsLoadRequest,
        result: SlackChannelDetailsLoadResult,
        cx: &mut Context<Self>,
    ) {
        let key = (request.team_id.clone(), request.conversation_id.clone());
        if self.slack_channel_details_inflight.get(&key) != Some(&request.generation) {
            return;
        }
        self.slack_channel_details_inflight.remove(&key);
        if self.slack_channel_details_request.as_ref() != Some(&request)
            || !self.slack_channel_details_open
            || !self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == request.team_id
                    && workspace.conversation_id == request.conversation_id
            })
        {
            return;
        }
        self.slack_channel_details_request = None;
        self.slack_channel_details_loading = false;
        let details = match result {
            Ok(details) => details,
            Err(error) => {
                self.slack_channel_details_error = Some(error.into());
                cx.notify();
                return;
            }
        };
        if details.team_id.as_ref() != request.team_id
            || details.conversation_id.as_ref() != request.conversation_id
        {
            self.slack_channel_details_error =
                Some("Slack returned details for a different channel.".into());
            cx.notify();
            return;
        }
        self.slack_channel_details_cache
            .insert(key, details.clone());
        self.slack_channel_details = Some(details);
        self.slack_channel_details_error = None;
        cx.notify();
        self.begin_slack_channel_creator_hydration(cx);
    }

    fn begin_slack_channel_creator_hydration(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.load_profile {
            return;
        }
        let Some(target) = self.slack_channel_creator_target() else {
            return;
        };
        let SlackChannelCreatorTarget {
            workspace_api,
            team_id,
            conversation_id,
            user_id,
            created_date_label,
        } = target;
        let key = (team_id.clone(), conversation_id.clone(), user_id.clone());
        if let Some(generation) = self.slack_channel_creator_inflight.get(&key).copied() {
            self.slack_channel_creator_request = Some(SlackChannelCreatorLoadRequest {
                generation,
                team_id,
                conversation_id,
                user_id,
                created_date_label,
            });
            return;
        }
        self.slack_channel_creator_generation = self
            .slack_channel_creator_generation
            .checked_add(1)
            .expect("Slack channel creator request generation overflowed");
        let request = SlackChannelCreatorLoadRequest {
            generation: self.slack_channel_creator_generation,
            team_id,
            conversation_id,
            user_id,
            created_date_label,
        };
        self.slack_channel_creator_inflight
            .insert(key, request.generation);
        self.slack_channel_creator_request = Some(request.clone());
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackChannelCreatorLoadRequest)| {
                let result =
                    workspace_api
                        .load_slack_profile(&request.user_id)
                        .and_then(|profile| {
                            prepare_slack_channel_creator_hydration(
                                &request.user_id,
                                profile,
                                request.created_date_label.clone(),
                            )
                        });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_channel_creator_hydration(request, result, cx);
            },
        );
    }

    fn slack_channel_creator_target(&self) -> Option<SlackChannelCreatorTarget> {
        let details = self
            .slack_channel_details
            .as_ref()
            .filter(|details| details.creator_needs_hydration)?;
        let team_id = details.team_id.to_string();
        let conversation_id = details.conversation_id.to_string();
        let user_id = details.creator_user_id.as_ref()?.to_string();
        if !self.slack_channel_details_open
            || !self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == team_id && workspace.conversation_id == conversation_id
            })
        {
            return None;
        }
        Some(SlackChannelCreatorTarget {
            workspace_api: self.active_slack_workspace_api()?,
            team_id,
            conversation_id,
            user_id,
            created_date_label: details.created_date_label.clone(),
        })
    }

    fn finish_slack_channel_creator_hydration(
        &mut self,
        request: SlackChannelCreatorLoadRequest,
        result: SlackChannelCreatorLoadResult,
        cx: &mut Context<Self>,
    ) {
        let key = (
            request.team_id.clone(),
            request.conversation_id.clone(),
            request.user_id.clone(),
        );
        if self.slack_channel_creator_inflight.get(&key) != Some(&request.generation) {
            return;
        }
        self.slack_channel_creator_inflight.remove(&key);
        if self.slack_channel_creator_request.as_ref() != Some(&request)
            || !self.slack_channel_details_open
            || !self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == request.team_id
                    && workspace.conversation_id == request.conversation_id
            })
        {
            return;
        }
        self.slack_channel_creator_request = None;
        let hydration = match result {
            Ok(hydration) => hydration,
            Err(error) => {
                eprintln!(
                    "[notslack-slack-channel-details-creator] team={} conversation={} creator={} error={error}",
                    request.team_id, request.conversation_id, request.user_id
                );
                return;
            }
        };
        if hydration.user_id.as_ref() != request.user_id {
            return;
        }
        let Some(details) = self.slack_channel_details.as_mut() else {
            return;
        };
        if details.team_id.as_ref() != request.team_id
            || details.conversation_id.as_ref() != request.conversation_id
            || details.creator_user_id.as_deref() != Some(request.user_id.as_str())
            || !details.creator_needs_hydration
        {
            return;
        }
        details.creation = Some(hydration.creation);
        details.creator_needs_hydration = false;
        let details = details.clone();
        self.slack_channel_details_cache
            .insert((request.team_id, request.conversation_id), details);
        cx.notify();
    }
}
