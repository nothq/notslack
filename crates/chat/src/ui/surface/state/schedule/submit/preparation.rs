use std::sync::Arc;

use chrono_tz::Tz;

use super::super::{Context, SlackScheduleDraftIdentity, SlackSchedulePostAt, SurfaceState};
use super::{
    SlackPreparedScheduleMutation, SlackPreparedScheduledDraft, SlackScheduleMutationInput,
    SlackScheduledListRefresh,
};
use crate::ui::surface::{
    SlackComposerDraft, SlackScheduleDraftOwner, SlackSchedulePendingSubmission,
    SlackScheduledMutationIdentity, SlackScheduledPendingOrigin, SlackScheduledPendingPhase,
};
use crate::ui::{SlackDraftsSentRequest, SlackDraftsSentTab, WorkspaceApi};

mod mutation;

pub(super) type SlackScheduleSubmission = (
    SlackSchedulePendingSubmission,
    SlackPreparedScheduledDraft,
    SlackScheduledListRefresh,
);

struct SlackScheduleDraftContext {
    identity: SlackScheduleDraftIdentity,
    draft: SlackComposerDraft,
    prepared: SlackPreparedScheduledDraft,
}

struct SlackScheduleRemoteContext {
    post_at_unix_seconds: i64,
    workspace_api: Arc<dyn WorkspaceApi>,
    timezone: Tz,
}

struct SlackScheduleSubmissionContext {
    owner: SlackScheduleDraftOwner,
    draft_identity: SlackScheduleDraftIdentity,
    draft: SlackComposerDraft,
    prepared: SlackPreparedScheduledDraft,
    post_at_unix_seconds: i64,
    workspace_api: Arc<dyn WorkspaceApi>,
    timezone: Tz,
}

impl SurfaceState {
    pub(super) fn prepare_slack_schedule_submission(
        &mut self,
        owner: SlackScheduleDraftOwner,
        post_at: SlackSchedulePostAt,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduleSubmission> {
        let context = self.prepare_slack_schedule_submission_context(owner, post_at, cx)?;
        let identity = self.next_slack_schedule_mutation_identity(&context.draft_identity);
        let mutation = self.prepare_slack_schedule_mutation(
            SlackScheduleMutationInput {
                identity: &identity,
                draft_identity: &context.draft_identity,
                owner: &context.owner,
                draft: &context.draft,
            },
            cx,
        )?;
        let accepted_draft =
            self.accept_slack_schedule_draft(&context.owner, context.draft.token, &mutation);
        Some(build_slack_schedule_submission(
            context,
            identity,
            mutation,
            accepted_draft,
        ))
    }

    fn prepare_slack_schedule_submission_context(
        &mut self,
        owner: SlackScheduleDraftOwner,
        post_at: SlackSchedulePostAt,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduleSubmissionContext> {
        let draft = self.prepare_slack_schedule_draft_context(&owner, cx)?;
        let remote =
            self.prepare_slack_schedule_remote_context(&owner, &draft.identity, post_at, cx)?;
        Some(SlackScheduleSubmissionContext {
            owner,
            draft_identity: draft.identity,
            draft: draft.draft,
            prepared: draft.prepared,
            post_at_unix_seconds: remote.post_at_unix_seconds,
            workspace_api: remote.workspace_api,
            timezone: remote.timezone,
        })
    }

    fn prepare_slack_schedule_draft_context(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduleDraftContext> {
        if !self.slack_workspace_api_capabilities.schedule_message {
            self.show_slack_schedule_error(
                owner,
                "Wait for the selected Slack conversation to finish loading before scheduling.",
                cx,
            );
            return None;
        }
        if matches!(owner, SlackScheduleDraftOwner::Main { .. })
            && self.slack_composer_capture_blocks_current_draft()
        {
            self.show_slack_schedule_error(
                owner,
                "Stop or cancel the media clip recording before scheduling this message.",
                cx,
            );
            return None;
        }
        let Some(identity) = self.slack_schedule_draft_identity(owner) else {
            self.show_slack_schedule_error(
                owner,
                "The selected Slack composer changed before scheduling could begin.",
                cx,
            );
            return None;
        };
        if let Some(diagnostic) = self.slack_schedule_blocking_owner_diagnostic() {
            self.show_slack_schedule_error(owner, &diagnostic, cx);
            return None;
        }
        if self.slack_schedule_create_or_update_is_blocked() {
            return None;
        }
        let Some(draft) = self.snapshot_slack_schedule_draft(owner) else {
            self.show_slack_schedule_error(
                owner,
                "The selected Slack composer changed before scheduling could begin.",
                cx,
            );
            return None;
        };
        if owner
            .draft_key()
            .is_some_and(|key| self.slack_thread_reply_is_pending(key))
        {
            return None;
        }
        let prepared = self.prepare_slack_scheduled_draft(owner, &draft, cx)?;
        Some(SlackScheduleDraftContext {
            identity,
            draft,
            prepared,
        })
    }

    fn prepare_slack_schedule_remote_context(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        draft_identity: &SlackScheduleDraftIdentity,
        post_at: SlackSchedulePostAt,
        cx: &mut Context<Self>,
    ) -> Option<SlackScheduleRemoteContext> {
        let post_at_unix_seconds = match post_at.future_unix_seconds() {
            Ok(post_at_unix_seconds) => post_at_unix_seconds,
            Err(message) => {
                self.show_slack_schedule_error(owner, &message, cx);
                return None;
            }
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.show_slack_schedule_error(owner, "missing Slack workspace api", cx);
            return None;
        };
        let Some(timezone) = self.slack_workspace().and_then(|workspace| {
            (workspace.team_id == draft_identity.team_id
                && workspace.self_user_id.as_deref() == Some(draft_identity.self_user_id.as_str()))
            .then_some(())?;
            workspace
                .self_timezone_id
                .as_deref()?
                .parse::<chrono_tz::Tz>()
                .ok()
        }) else {
            self.show_slack_schedule_error(
                owner,
                "Slack scheduled messages require an authenticated identity and workspace time zone.",
                cx,
            );
            return None;
        };
        Some(SlackScheduleRemoteContext {
            post_at_unix_seconds,
            workspace_api,
            timezone,
        })
    }

    fn next_slack_schedule_mutation_identity(
        &mut self,
        draft_identity: &SlackScheduleDraftIdentity,
    ) -> SlackScheduledMutationIdentity {
        let team_id = draft_identity.team_id.clone();
        let self_user_id = draft_identity.self_user_id.clone();
        if self.slack_drafts_sent_team_id.as_deref() != Some(team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref() != Some(self_user_id.as_str())
        {
            self.reset_slack_drafts_sent_context();
            self.slack_drafts_sent_team_id = Some(team_id.clone());
            self.slack_drafts_sent_self_user_id = Some(self_user_id.clone());
        }
        self.slack_schedule_generation = self
            .slack_schedule_generation
            .checked_add(1)
            .expect("Slack schedule request generation overflowed");
        SlackScheduledMutationIdentity {
            generation: self.slack_schedule_generation,
            team_id,
            self_user_id,
            conversation_id: draft_identity.conversation_id.clone(),
        }
    }

    fn accept_slack_schedule_draft(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        draft_token: u64,
        mutation: &SlackPreparedScheduleMutation,
    ) -> SlackComposerDraft {
        let mut accepted_draft = self
            .take_slack_schedule_draft(owner)
            .expect("validated Slack schedule owner must retain its exact draft");
        accepted_draft.client_message_id = Some(mutation.client_message_id.clone());
        assert_eq!(
            accepted_draft.id,
            owner.draft_id(),
            "Slack scheduling must retain the exact accepted draft identity"
        );
        assert_eq!(
            accepted_draft.token, draft_token,
            "Slack scheduling must claim the exact accepted draft token"
        );
        if let SlackScheduledPendingOrigin::ScheduledEdit {
            prior_draft_handle, ..
        } = &mutation.origin
        {
            let active_edit = self
                .slack_active_scheduled_edit
                .take()
                .expect("scheduled-edit submission requires its active edit owner");
            assert_eq!(
                active_edit.edit_draft_handle.draft_id,
                owner.draft_id(),
                "scheduled-edit submission must detach the exact edit draft"
            );
            assert_eq!(
                &active_edit.prior_draft_handle, prior_draft_handle,
                "scheduled-edit submission must retain its exact prior draft"
            );
            self.restore_slack_send_draft(Some(active_edit.prior_draft));
        }
        accepted_draft
    }

    fn prepare_slack_scheduled_draft(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        draft: &crate::ui::surface::SlackComposerDraft,
        cx: &mut Context<Self>,
    ) -> Option<SlackPreparedScheduledDraft> {
        let text_is_empty = draft.document.text().trim().is_empty();
        if text_is_empty && draft.files.is_empty() {
            self.show_slack_schedule_error(
                owner,
                "Add a message or attachment before scheduling.",
                cx,
            );
            return None;
        }
        let file_ids = match draft.files.projected_slack_file_ids() {
            Ok(file_ids) => file_ids,
            Err(message) => {
                self.show_slack_schedule_error(owner, &message, cx);
                return None;
            }
        };
        let message_draft = if text_is_empty {
            None
        } else {
            match draft.document.export_message_draft() {
                Ok(message_draft) => Some(message_draft),
                Err(message) => {
                    self.show_slack_schedule_error(owner, &message, cx);
                    return None;
                }
            }
        };
        Some(SlackPreparedScheduledDraft {
            message_draft,
            local_files: draft.files.scheduled_local_files(),
            file_ids,
        })
    }
}

fn build_slack_schedule_submission(
    context: SlackScheduleSubmissionContext,
    identity: SlackScheduledMutationIdentity,
    mutation: SlackPreparedScheduleMutation,
    accepted_draft: SlackComposerDraft,
) -> SlackScheduleSubmission {
    let SlackScheduleSubmissionContext {
        owner,
        prepared,
        post_at_unix_seconds,
        workspace_api,
        timezone,
        ..
    } = context;
    let SlackPreparedScheduleMutation {
        mutation,
        origin,
        client_message_id: _,
    } = mutation;
    let team_id = identity.team_id.clone();
    let self_user_id = identity.self_user_id.clone();
    (
        SlackSchedulePendingSubmission {
            identity,
            owner,
            accepted_draft,
            mutation,
            origin,
            post_at_unix_seconds,
            file_ids: prepared.file_ids.clone(),
            local_files: prepared.local_files.clone(),
            workspace_api,
            timezone,
            phase: SlackScheduledPendingPhase::Submitting,
        },
        prepared,
        SlackScheduledListRefresh {
            request: SlackDraftsSentRequest {
                team_id,
                tab: SlackDraftsSentTab::Scheduled,
                cursor: None,
            },
            timezone,
            authenticated_self_user_id: self_user_id,
        },
    )
}
