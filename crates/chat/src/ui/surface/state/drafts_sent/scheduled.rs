use std::sync::Arc;

use super::{Context, SlackDraftsSentTab, SurfaceState};
use crate::ui::surface::{
    prepare_slack_drafts_sent_snapshot, PreparedSlackDraftsSentSnapshot,
    SlackScheduledDeleteIdentity, SlackScheduledEdit,
};
use crate::ui::{SlackDraftsSentRequest, WorkspaceApi};

struct SlackScheduledDeleteRequest {
    identity: SlackScheduledDeleteIdentity,
    workspace_api: Arc<dyn WorkspaceApi>,
    list_request: SlackDraftsSentRequest,
    timezone: chrono_tz::Tz,
    authenticated_self_user_id: String,
}

struct SlackScheduledDeleteResult {
    identity: SlackScheduledDeleteIdentity,
    deletion: Result<(), String>,
    refreshed: Option<Result<PreparedSlackDraftsSentSnapshot, String>>,
}

struct SlackScheduledDeleteWorkspace {
    workspace_api: Arc<dyn WorkspaceApi>,
    team_id: String,
    timezone: chrono_tz::Tz,
    authenticated_self_user_id: String,
}

impl SurfaceState {
    pub(crate) fn delete_slack_scheduled_draft(
        &mut self,
        edit: SlackScheduledEdit,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.prepare_slack_scheduled_delete(edit, cx) else {
            return;
        };
        self.spawn_background_task(
            request,
            cx,
            |request: SlackScheduledDeleteRequest| {
                let deletion = request
                    .workspace_api
                    .delete_slack_scheduled_draft(&request.identity.target);
                let refreshed = deletion.as_ref().ok().map(|_| {
                    request
                        .workspace_api
                        .load_slack_drafts_sent(request.list_request)
                        .and_then(|snapshot| {
                            prepare_slack_drafts_sent_snapshot(
                                snapshot,
                                request.timezone,
                                &request.authenticated_self_user_id,
                            )
                        })
                });
                SlackScheduledDeleteResult {
                    identity: request.identity,
                    deletion,
                    refreshed,
                }
            },
            |this, result, cx| {
                this.finish_slack_scheduled_delete(result, cx);
            },
        );
    }

    fn prepare_slack_scheduled_delete(
        &mut self,
        edit: SlackScheduledEdit,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduledDeleteRequest> {
        if !self.slack_scheduled_delete_is_available(edit.target.draft_id()) {
            return None;
        }
        let workspace = self.slack_scheduled_delete_workspace(cx)?;
        self.slack_scheduled_mutation_generation = self
            .slack_scheduled_mutation_generation
            .checked_add(1)
            .expect("Slack scheduled mutation generation overflowed");
        let identity = SlackScheduledDeleteIdentity::new(
            self.slack_scheduled_mutation_generation,
            workspace.team_id.clone(),
            workspace.authenticated_self_user_id.clone(),
            edit.conversation_id,
            edit.target,
        );
        self.slack_scheduled_delete_pending = Some(identity.clone());
        self.slack_drafts_sent_error = None;
        cx.notify();
        Some(SlackScheduledDeleteRequest {
            identity,
            workspace_api: workspace.workspace_api,
            list_request: SlackDraftsSentRequest {
                team_id: workspace.team_id,
                tab: SlackDraftsSentTab::Scheduled,
                cursor: None,
            },
            timezone: workspace.timezone,
            authenticated_self_user_id: workspace.authenticated_self_user_id,
        })
    }

    fn slack_scheduled_delete_is_available(
        &self,
        edit_draft_id: &crate::model::SlackDraftId,
    ) -> bool {
        let conflicting_edit_owner = self
            .slack_active_scheduled_edit
            .as_ref()
            .is_some_and(|active| active.edit.target.draft_id() == edit_draft_id)
            || self
                .slack_pending_draft_restore
                .as_ref()
                .and_then(|restore| restore.scheduled_edit.as_ref())
                .is_some_and(|pending_edit| pending_edit.target.draft_id() == edit_draft_id)
            || self
                .slack_scheduled_edit_recovery
                .as_ref()
                .is_some_and(|recovery| recovery.edit.target.draft_id() == edit_draft_id);
        self.slack_workspace_api_capabilities
            .delete_scheduled_message
            && self.slack_scheduled_delete_pending.is_none()
            && self.slack_schedule_pending.is_none()
            && self.slack_composer_schedule_recovery.is_none()
            && !conflicting_edit_owner
    }

    fn slack_scheduled_delete_workspace(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduledDeleteWorkspace> {
        let workspace_api = self.active_slack_workspace_api()?;
        let team_id = self.slack_drafts_sent_team_id.clone()?;
        let drafts_sent_self_user_id = self.slack_drafts_sent_self_user_id.clone()?;
        let Some((workspace_team_id, timezone, authenticated_self_user_id)) =
            self.slack_workspace().and_then(|workspace| {
                Some((
                    workspace.team_id.clone(),
                    workspace
                        .self_timezone_id
                        .as_deref()?
                        .parse::<chrono_tz::Tz>()
                        .ok()?,
                    workspace.self_user_id.clone()?,
                ))
            })
        else {
            self.slack_drafts_sent_error =
                Some("Slack scheduled messages require the workspace time zone.".to_string());
            cx.notify();
            return None;
        };
        if workspace_team_id != team_id || authenticated_self_user_id != drafts_sent_self_user_id {
            return None;
        }
        Some(SlackScheduledDeleteWorkspace {
            workspace_api,
            team_id,
            timezone,
            authenticated_self_user_id,
        })
    }

    fn finish_slack_scheduled_delete(
        &mut self,
        result: SlackScheduledDeleteResult,
        cx: &mut Context<Self>,
    ) {
        if self.slack_scheduled_delete_pending.as_ref() != Some(&result.identity)
            || self.slack_scheduled_mutation_generation != result.identity.mutation.generation
            || self.slack_drafts_sent_team_id.as_deref()
                != Some(result.identity.mutation.team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref()
                != Some(result.identity.mutation.self_user_id.as_str())
            || self.slack_workspace().is_none_or(|workspace| {
                workspace.team_id.as_str() != result.identity.mutation.team_id.as_str()
                    || workspace.self_user_id.as_deref()
                        != Some(result.identity.mutation.self_user_id.as_str())
            })
        {
            return;
        }
        self.slack_scheduled_delete_pending = None;
        if let Err(error) = result.deletion {
            self.slack_drafts_sent_error = Some(error);
            cx.notify();
            return;
        }
        if self
            .slack_active_scheduled_edit
            .as_ref()
            .is_some_and(|active| active.edit.target == result.identity.target)
        {
            self.restore_slack_scheduled_edit_prior_draft(cx);
        }
        match result.refreshed {
            Some(Ok(prepared)) => {
                self.slack_drafts_sent_error = self
                    .apply_authoritative_slack_scheduled_snapshot(prepared)
                    .err();
            }
            Some(Err(error)) => {
                self.invalidate_slack_scheduled_snapshot();
                self.slack_drafts_sent_error = Some(format!(
                    "Scheduled message canceled, but Slack could not refresh the scheduled list: {error}"
                ));
            }
            None => unreachable!("successful Slack scheduled deletion must refresh the list"),
        }
        cx.notify();
    }
}
