use std::sync::Arc;

use super::{Context, SurfaceState, WorkspaceApi};
use crate::model::ChatSurfaceEvent;
use crate::ui::surface::{
    SlackChannelNotificationLoadRequest, SlackChannelNotificationMutationRequest,
};
use crate::ui::{
    SlackChannelNotificationMode, SlackChannelNotificationMutation,
    SlackChannelNotificationPreference, SlackNotificationTeamBadge,
};

mod context;

impl SurfaceState {
    pub(crate) fn emit_slack_dock_badge(&self, cx: &mut Context<Self>) {
        if let Some(badge) = self.slack_notification_team_badge() {
            cx.emit(ChatSurfaceEvent::DockBadgeChanged(badge));
        }
    }

    pub(crate) fn clear_slack_dock_badge(&self, team_id: String, cx: &mut Context<Self>) {
        cx.emit(ChatSurfaceEvent::DockBadgeChanged(
            SlackNotificationTeamBadge { team_id, count: 0 },
        ));
    }

    pub(crate) fn sync_slack_channel_notification_preference(&mut self, cx: &mut Context<Self>) {
        let target = self.slack_workspace().and_then(|workspace| {
            (self
                .slack_workspace_api_capabilities
                .load_channel_notification_preferences
                && !workspace.team_id.is_empty()
                && !workspace.conversation_id.is_empty())
            .then(|| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        });
        let Some((team_id, conversation_id)) = target else {
            self.clear_slack_channel_notification_context();
            return;
        };
        if self
            .slack_channel_notification_preference
            .as_ref()
            .is_some_and(|preference| {
                preference.team_id == team_id && preference.conversation_id == conversation_id
            })
            || self
                .slack_channel_notification_load_request
                .as_ref()
                .is_some_and(|request| {
                    request.team_id == team_id && request.conversation_id == conversation_id
                })
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.clear_slack_channel_notification_context();
            return;
        };

        self.slack_channel_notification_generation = self
            .slack_channel_notification_generation
            .checked_add(1)
            .expect("Slack channel notification request generation overflowed");
        let request = SlackChannelNotificationLoadRequest {
            generation: self.slack_channel_notification_generation,
            team_id,
            conversation_id,
        };
        self.mark_slack_channel_notification_loading(&request);
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (
                Arc<dyn WorkspaceApi>,
                SlackChannelNotificationLoadRequest,
            )| {
                let result = workspace_api
                    .load_slack_channel_notification_preference(&request.conversation_id);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_channel_notification_load(request, result, cx);
            },
        );
    }

    fn mark_slack_channel_notification_loading(
        &mut self,
        request: &SlackChannelNotificationLoadRequest,
    ) {
        self.slack_channel_notification_preference = None;
        self.slack_channel_notification_load_request = Some(request.clone());
        self.slack_channel_notification_mutation_request = None;
        self.slack_channel_notification_loading = true;
        self.slack_channel_notification_error = None;
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.slack_channel_notifications_menu_keyboard_highlighted = false;
    }

    pub(crate) fn slack_channel_notifications_control_available(&self) -> bool {
        self.slack_workspace_api_capabilities
            .load_channel_notification_preferences
            && self
                .slack_workspace_api_capabilities
                .mutate_channel_notification_preferences
            && self
                .slack_workspace()
                .zip(self.slack_channel_notification_preference.as_ref())
                .is_some_and(|(workspace, preference)| {
                    workspace.team_id == preference.team_id
                        && workspace.conversation_id == preference.conversation_id
                })
    }

    pub(crate) fn close_slack_channel_notifications_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_notifications_menu_open {
            return;
        }
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.slack_channel_notifications_menu_keyboard_highlighted = false;
        self.slack_channel_notifications_menu_focus_pending = false;
        self.slack_header_notifications_focus_pending = true;
        cx.notify();
    }

    pub(crate) fn move_slack_channel_notifications_menu_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        let option_count = if self.slack_channel_notifications_advanced_open {
            1
        } else {
            4
        };
        let current = if self.slack_channel_notifications_advanced_open {
            0
        } else {
            self.slack_channel_notifications_menu_selected_index
        };
        let next = (current as isize + direction).rem_euclid(option_count) as usize;
        self.slack_channel_notifications_menu_selected_index =
            if self.slack_channel_notifications_advanced_open {
                4
            } else {
                next
            };
        self.slack_channel_notifications_menu_keyboard_highlighted = true;
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_channel_notification(&mut self, cx: &mut Context<Self>) {
        match self.slack_channel_notifications_menu_selected_index {
            0 => self.start_slack_channel_notification_mutation(
                SlackChannelNotificationMutation::SetMode(SlackChannelNotificationMode::Everything),
                cx,
            ),
            1 => self.start_slack_channel_notification_mutation(
                SlackChannelNotificationMutation::SetMode(SlackChannelNotificationMode::Mentions),
                cx,
            ),
            2 => {
                let muted = self
                    .slack_channel_notification_preference
                    .as_ref()
                    .is_some_and(|preference| preference.muted);
                self.start_slack_channel_notification_mutation(
                    SlackChannelNotificationMutation::SetMuted(!muted),
                    cx,
                );
            }
            3 => {
                self.slack_channel_notifications_advanced_open = true;
                self.slack_channel_notifications_menu_selected_index = 4;
                cx.notify();
            }
            4 => self.start_slack_channel_notification_mutation(
                SlackChannelNotificationMutation::SetMode(SlackChannelNotificationMode::Nothing),
                cx,
            ),
            _ => {}
        }
    }

    pub(crate) fn return_to_slack_channel_notifications_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_notifications_advanced_open {
            return;
        }
        self.slack_channel_notifications_advanced_open = false;
        self.slack_channel_notifications_menu_selected_index = 3;
        self.slack_channel_notifications_menu_keyboard_highlighted = true;
        cx.notify();
    }

    pub(crate) fn start_slack_channel_notification_mutation(
        &mut self,
        mutation: SlackChannelNotificationMutation,
        cx: &mut Context<Self>,
    ) {
        if self.slack_channel_notification_mutation_request.is_some()
            || !self.slack_channel_notifications_control_available()
        {
            return;
        }
        let (team_id, conversation_id) = {
            let Some(preference) = self.slack_channel_notification_preference.as_ref() else {
                return;
            };
            if slack_notification_mutation_is_noop(preference, mutation) {
                self.close_slack_channel_notifications_menu(cx);
                return;
            }
            (
                preference.team_id.clone(),
                preference.conversation_id.clone(),
            )
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_channel_notification_generation = self
            .slack_channel_notification_generation
            .checked_add(1)
            .expect("Slack channel notification request generation overflowed");
        let request = SlackChannelNotificationMutationRequest {
            generation: self.slack_channel_notification_generation,
            team_id,
            conversation_id,
            mutation,
        };
        self.slack_channel_notification_mutation_request = Some(request.clone());
        self.slack_channel_notification_error = None;
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.slack_header_notifications_focus_pending = true;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (
                Arc<dyn WorkspaceApi>,
                SlackChannelNotificationMutationRequest,
            )| {
                let result = workspace_api.mutate_slack_channel_notification_preference(
                    &request.conversation_id,
                    request.mutation,
                );
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_channel_notification_mutation(request, result, cx);
            },
        );
    }

    fn finish_slack_channel_notification_load(
        &mut self,
        request: SlackChannelNotificationLoadRequest,
        result: Result<SlackChannelNotificationPreference, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_channel_notification_load_request.as_ref() != Some(&request) {
            return;
        }
        self.slack_channel_notification_load_request = None;
        self.slack_channel_notification_loading = false;
        match result {
            Ok(preference)
                if preference.team_id == request.team_id
                    && preference.conversation_id == request.conversation_id =>
            {
                self.slack_channel_notification_preference = Some(preference);
                self.slack_channel_notification_error = None;
            }
            Ok(_) => {
                self.slack_channel_notification_preference = None;
                self.slack_channel_notification_error =
                    Some("Slack returned notification preferences for a different channel.".into());
            }
            Err(error) => {
                self.slack_channel_notification_preference = None;
                self.slack_channel_notification_error = Some(error.into());
            }
        }
        cx.notify();
    }

    fn finish_slack_channel_notification_mutation(
        &mut self,
        request: SlackChannelNotificationMutationRequest,
        result: Result<SlackChannelNotificationPreference, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_channel_notification_mutation_request.as_ref() != Some(&request) {
            return;
        }
        self.slack_channel_notification_mutation_request = None;
        let still_active = self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == request.team_id
                && workspace.conversation_id == request.conversation_id
        });
        if !still_active {
            return;
        }
        match result {
            Ok(preference)
                if preference.team_id == request.team_id
                    && preference.conversation_id == request.conversation_id =>
            {
                self.slack_channel_notification_preference = Some(preference);
                self.slack_channel_notification_error = None;
            }
            Ok(_) => {
                self.slack_channel_notification_error =
                    Some("Slack returned notification preferences for a different channel.".into());
            }
            Err(error) => {
                self.slack_channel_notification_error = Some(error.into());
            }
        }
        cx.notify();
    }

    fn clear_slack_channel_notification_context(&mut self) {
        self.slack_channel_notification_preference = None;
        self.slack_channel_notification_load_request = None;
        self.slack_channel_notification_mutation_request = None;
        self.slack_channel_notification_loading = false;
        self.slack_channel_notification_error = None;
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.slack_channel_notifications_menu_keyboard_highlighted = false;
        self.slack_channel_notifications_menu_focus_pending = false;
        self.slack_header_notifications_focus_pending = false;
    }
}

fn slack_notification_mutation_is_noop(
    preference: &SlackChannelNotificationPreference,
    mutation: SlackChannelNotificationMutation,
) -> bool {
    match mutation {
        SlackChannelNotificationMutation::SetMode(mode) => preference.desktop_mode == mode,
        SlackChannelNotificationMutation::SetMuted(muted) => preference.muted == muted,
    }
}
