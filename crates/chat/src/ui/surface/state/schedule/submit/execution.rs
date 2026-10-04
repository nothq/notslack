use std::sync::Arc;

use chrono::Utc;

use super::{
    SlackScheduleReconcileResult, SlackScheduleReconcileWork, SlackScheduleRemoteRequest,
    SlackScheduleTaskResult, SlackScheduleWork, SlackScheduledListRefresh,
};
use crate::ui::surface::{
    prepare_slack_drafts_sent_snapshot, PreparedSlackDraftsSentSnapshot, SlackComposerDestination,
    SlackSchedulePendingSubmission, SlackScheduledPendingMutation,
};
use crate::ui::{SlackMessageDraft, WorkspaceApi};

pub(super) fn slack_schedule_write_target(
    destination: &SlackComposerDestination,
    broadcast: bool,
) -> Result<crate::model::SlackDraftWriteTarget, String> {
    match destination {
        SlackComposerDestination::Conversation { conversation_id } => {
            crate::model::SlackDraftWriteTarget::conversation(conversation_id.clone())
        }
        SlackComposerDestination::Thread {
            conversation_id,
            thread_timestamp,
        } => crate::model::SlackDraftWriteTarget::thread(
            conversation_id.clone(),
            thread_timestamp.clone(),
            broadcast,
        ),
    }
}

pub(super) fn execute_slack_schedule_work(work: SlackScheduleWork) -> SlackScheduleTaskResult {
    let outcome = execute_slack_schedule_mutation(&work.request, work.message_draft.as_ref());
    let refreshed = load_prepared_scheduled_snapshot(&work.request.workspace_api, &work.refresh);
    let outcome = match outcome {
        Err(crate::model::SlackScheduledDraftMutationFailure::Unknown { diagnostic }) => {
            match reconcile_slack_schedule_pending(&work.request) {
                Ok(Some(receipt)) => Ok(receipt),
                Ok(None) => Err(crate::model::SlackScheduledDraftMutationFailure::Unknown {
                    diagnostic,
                }),
                Err(reconciliation_error) => {
                    Err(crate::model::SlackScheduledDraftMutationFailure::Unknown {
                        diagnostic: format!(
                            "{diagnostic}; later authoritative reconciliation failed: {reconciliation_error}"
                        ),
                    })
                }
            }
        }
        outcome => outcome,
    };
    SlackScheduleTaskResult {
        identity: work.request.identity,
        outcome,
        refreshed,
    }
}

pub(super) fn schedule_remote_request(
    pending: &SlackSchedulePendingSubmission,
) -> SlackScheduleRemoteRequest {
    SlackScheduleRemoteRequest {
        identity: pending.identity.clone(),
        mutation: pending.mutation.clone(),
        post_at_unix_seconds: pending.post_at_unix_seconds,
        file_ids: pending.file_ids.clone(),
        local_files: pending.local_files.clone(),
        workspace_api: pending.workspace_api.clone(),
    }
}

pub(super) fn execute_slack_schedule_reconcile_work(
    work: SlackScheduleReconcileWork,
) -> SlackScheduleReconcileResult {
    let outcome = reconcile_slack_schedule_pending(&work.request);
    let refreshed = load_prepared_scheduled_snapshot(&work.request.workspace_api, &work.refresh);
    SlackScheduleReconcileResult {
        identity: work.request.identity,
        attempt: work.attempt,
        outcome,
        refreshed,
    }
}

fn execute_slack_schedule_mutation(
    request: &SlackScheduleRemoteRequest,
    message_draft: Option<&SlackMessageDraft>,
) -> Result<crate::ui::SlackScheduledDraftReceipt, crate::model::SlackScheduledDraftMutationFailure>
{
    let content = message_draft.map_or(
        crate::model::SlackDraftContent::FilesOnly,
        crate::model::SlackDraftContent::Message,
    );
    match &request.mutation {
        SlackScheduledPendingMutation::Create {
            client_message_id,
            write_target,
        } => request.workspace_api.create_slack_scheduled_draft(
            crate::model::SlackScheduledDraftCreateTarget::new(
                client_message_id,
                write_target,
                request.post_at_unix_seconds,
            ),
            content,
            &request.file_ids,
        ),
        SlackScheduledPendingMutation::Promote {
            target,
            write_target,
            client_mutation_timestamp,
            ..
        } => request.workspace_api.update_slack_scheduled_draft(
            crate::model::SlackScheduledDraftUpdateTarget::new(
                crate::model::SlackDraftUpdateTarget::new(target, client_mutation_timestamp),
                std::slice::from_ref(write_target),
                request.post_at_unix_seconds,
            ),
            content,
            &request.file_ids,
        ),
        SlackScheduledPendingMutation::Update {
            edit,
            client_mutation_timestamp,
        } => request.workspace_api.update_slack_scheduled_draft(
            crate::model::SlackScheduledDraftUpdateTarget::new(
                crate::model::SlackDraftUpdateTarget::new(&edit.target, client_mutation_timestamp),
                &edit.write_targets,
                request.post_at_unix_seconds,
            ),
            content,
            &request.file_ids,
        ),
    }
}

fn reconcile_slack_schedule_pending(
    request: &SlackScheduleRemoteRequest,
) -> Result<Option<crate::ui::SlackScheduledDraftReceipt>, String> {
    match &request.mutation {
        SlackScheduledPendingMutation::Create {
            client_message_id,
            write_target,
        } => request.workspace_api.reconcile_slack_scheduled_draft(
            crate::model::SlackScheduledDraftReconcileTarget::Create(
                crate::model::SlackScheduledDraftCreateTarget::new(
                    client_message_id,
                    write_target,
                    request.post_at_unix_seconds,
                ),
            ),
            &request.file_ids,
            &request.local_files,
        ),
        SlackScheduledPendingMutation::Promote {
            target,
            write_target,
            client_mutation_timestamp,
            ..
        } => request.workspace_api.reconcile_slack_scheduled_draft(
            crate::model::SlackScheduledDraftReconcileTarget::Update(
                crate::model::SlackScheduledDraftUpdateTarget::new(
                    crate::model::SlackDraftUpdateTarget::new(target, client_mutation_timestamp),
                    std::slice::from_ref(write_target),
                    request.post_at_unix_seconds,
                ),
            ),
            &request.file_ids,
            &request.local_files,
        ),
        SlackScheduledPendingMutation::Update {
            edit,
            client_mutation_timestamp,
        } => request.workspace_api.reconcile_slack_scheduled_draft(
            crate::model::SlackScheduledDraftReconcileTarget::Update(
                crate::model::SlackScheduledDraftUpdateTarget::new(
                    crate::model::SlackDraftUpdateTarget::new(
                        &edit.target,
                        client_mutation_timestamp,
                    ),
                    &edit.write_targets,
                    request.post_at_unix_seconds,
                ),
            ),
            &request.file_ids,
            &request.local_files,
        ),
    }
}

fn load_prepared_scheduled_snapshot(
    workspace_api: &Arc<dyn WorkspaceApi>,
    refresh: &SlackScheduledListRefresh,
) -> Result<PreparedSlackDraftsSentSnapshot, String> {
    workspace_api
        .load_slack_drafts_sent(refresh.request.clone())
        .and_then(|snapshot| {
            prepare_slack_drafts_sent_snapshot(
                snapshot,
                refresh.timezone,
                &refresh.authenticated_self_user_id,
            )
        })
}

pub(super) fn schedule_client_mutation_timestamp(
) -> Result<crate::model::SlackDraftClientMutationTimestamp, String> {
    crate::model::SlackDraftClientMutationTimestamp::from_unix_milliseconds(
        Utc::now().timestamp_millis(),
    )
}

pub(super) fn append_refresh_error(message: String, refresh_error: Option<String>) -> String {
    match refresh_error {
        Some(refresh_error) => format!("{message} {refresh_error}"),
        None => message,
    }
}
