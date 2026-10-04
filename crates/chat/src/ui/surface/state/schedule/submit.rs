mod completion;
mod execution;
mod preparation;
mod reconciliation;
mod refresh;

use std::{sync::Arc, time::Duration};

use super::{
    Context, SlackScheduleDraftIdentity, SlackScheduleOverlay, SlackSchedulePostAt, SurfaceState,
};
use crate::ui::surface::{
    PreparedSlackDraftsSentSnapshot, SlackScheduleDraftOwner, SlackScheduledMutationIdentity,
    SlackScheduledPendingMutation, SlackScheduledPendingOrigin,
};
use crate::ui::{SlackDraftsSentRequest, SlackMessageDraft, WorkspaceApi};
use execution::{execute_slack_schedule_work, schedule_remote_request};

const SLACK_SCHEDULE_RECONCILE_MAX_ATTEMPTS: u8 = 4;
const SLACK_SCHEDULE_RECONCILE_BASE_DELAY: Duration = Duration::from_millis(500);

struct SlackPreparedScheduledDraft {
    message_draft: Option<SlackMessageDraft>,
    file_ids: Arc<[crate::model::SlackFileId]>,
    local_files: Arc<[crate::model::SlackScheduledDraftLocalFile]>,
}

struct SlackPreparedScheduleMutation {
    mutation: SlackScheduledPendingMutation,
    origin: SlackScheduledPendingOrigin,
    client_message_id: crate::model::SlackMessageClientId,
}

struct SlackScheduleMutationInput<'a> {
    identity: &'a SlackScheduledMutationIdentity,
    draft_identity: &'a SlackScheduleDraftIdentity,
    owner: &'a SlackScheduleDraftOwner,
    draft: &'a crate::ui::surface::SlackComposerDraft,
}

struct SlackScheduledListRefresh {
    request: SlackDraftsSentRequest,
    timezone: chrono_tz::Tz,
    authenticated_self_user_id: String,
}

struct SlackScheduleWork {
    request: SlackScheduleRemoteRequest,
    message_draft: Option<SlackMessageDraft>,
    refresh: SlackScheduledListRefresh,
}

#[derive(Clone)]
struct SlackScheduleRemoteRequest {
    identity: SlackScheduledMutationIdentity,
    mutation: SlackScheduledPendingMutation,
    post_at_unix_seconds: i64,
    file_ids: Arc<[crate::model::SlackFileId]>,
    local_files: Arc<[crate::model::SlackScheduledDraftLocalFile]>,
    workspace_api: Arc<dyn WorkspaceApi>,
}

struct SlackScheduleTaskResult {
    identity: SlackScheduledMutationIdentity,
    outcome: Result<
        crate::ui::SlackScheduledDraftReceipt,
        crate::model::SlackScheduledDraftMutationFailure,
    >,
    refreshed: Result<PreparedSlackDraftsSentSnapshot, String>,
}

struct SlackScheduleReconcileWork {
    request: SlackScheduleRemoteRequest,
    refresh: SlackScheduledListRefresh,
    attempt: u8,
}

struct SlackScheduleReconcileResult {
    identity: SlackScheduledMutationIdentity,
    attempt: u8,
    outcome: Result<Option<crate::ui::SlackScheduledDraftReceipt>, String>,
    refreshed: Result<PreparedSlackDraftsSentSnapshot, String>,
}

impl SurfaceState {
    pub(crate) fn schedule_slack_preset(&mut self, preset_index: usize, cx: &mut Context<Self>) {
        let Some(overlay) = self.slack_schedule_overlay.as_ref() else {
            return;
        };
        let SlackScheduleOverlay::Menu(menu) = &overlay.phase else {
            return;
        };
        let Some(preset) = menu
            .presets
            .get(preset_index)
            .and_then(Option::as_ref)
            .cloned()
        else {
            return;
        };
        self.submit_slack_schedule(overlay.owner.clone(), preset.post_at, cx);
    }

    pub(crate) fn submit_slack_custom_schedule(&mut self, cx: &mut Context<Self>) {
        let Some((owner, post_at)) = self.slack_schedule_overlay.as_ref().and_then(|overlay| {
            let SlackScheduleOverlay::Custom(custom) = &overlay.phase else {
                return None;
            };
            Some((overlay.owner.clone(), custom.post_at()))
        }) else {
            return;
        };
        let post_at = match post_at {
            Ok(post_at) => post_at,
            Err(message) => {
                self.show_slack_schedule_error(&owner, &message, cx);
                return;
            }
        };
        self.submit_slack_schedule(owner, post_at, cx);
    }

    pub(in crate::ui::surface::state) fn submit_slack_schedule(
        &mut self,
        owner: SlackScheduleDraftOwner,
        post_at: SlackSchedulePostAt,
        cx: &mut Context<Self>,
    ) {
        let Some((pending, prepared, refresh)) =
            self.prepare_slack_schedule_submission(owner, post_at, cx)
        else {
            return;
        };
        let work = SlackScheduleWork {
            request: schedule_remote_request(&pending),
            message_draft: prepared.message_draft,
            refresh,
        };
        let owner = pending.owner.clone();
        self.slack_schedule_pending = Some(pending);
        self.slack_schedule_overlay = None;
        self.slack_schedule_submission_error = None;
        self.clear_slack_schedule_owner_error(&owner);
        cx.notify();
        self.spawn_background_task(
            work,
            cx,
            |work: SlackScheduleWork| execute_slack_schedule_work(work),
            |this, result, cx| {
                this.finish_slack_schedule_submission(result, cx);
            },
        );
    }
}
