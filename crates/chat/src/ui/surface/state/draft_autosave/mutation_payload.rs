use super::{
    Arc, SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey, SlackDraftContent,
    SlackDraftDesiredContentFingerprint, SlackDraftDesiredRemote, SlackDraftDesiredState,
    SlackDraftFileDeletion, SlackDraftMutation, SlackDraftMutationSuccess, SlackDraftUpdateTarget,
    SlackDraftWriteTarget, SlackFileId, SlackFileStagingLocator, SlackMessageClientId,
    SlackRemoteDraftState, WorkspaceApi,
};

pub(super) fn slack_staged_file_token(
    draft_id: crate::ui::surface::SlackComposerDraftId,
    token: u64,
    files: &crate::ui::surface::SlackComposerFiles,
    locator: &SlackFileStagingLocator,
) -> Option<u64> {
    (draft_id == locator.draft_id
        && files.file(locator.file_id).is_some_and(|file| {
            matches!(
                file.state(),
                crate::ui::surface::SlackComposerFileState::Staged { staged, .. }
                    if staged.operation_id() == &locator.operation_id
            )
        }))
    .then_some(token)
}

pub(super) fn prepare_slack_draft_desired_state(
    key: &SlackComposerDraftKey,
    token: u64,
    draft: Option<&SlackComposerDraft>,
    remote: &SlackRemoteDraftState,
    create_client_message_id: Option<SlackMessageClientId>,
) -> Result<SlackDraftDesiredState, String> {
    let file_ids = draft
        .map(|draft| draft.files.projected_slack_file_ids())
        .transpose()?
        .unwrap_or_else(|| Arc::from([]));
    let document = draft.map(|draft| &draft.document);
    let has_message = document.is_some_and(|document| document.has_message_content());
    let content = if has_message {
        SlackDraftDesiredContentFingerprint::Message {
            message: document
                .expect("Slack draft message presence was established")
                .export_message_draft()?,
            file_ids,
        }
    } else if !file_ids.is_empty() {
        SlackDraftDesiredContentFingerprint::FilesOnly { file_ids }
    } else {
        return Ok(SlackDraftDesiredState::Absent { token });
    };
    let broadcast = draft.is_some_and(|draft| draft.broadcast);
    let (client_message_id, write_target) = match remote {
        SlackRemoteDraftState::Present(remote) => (
            remote.client_message_id.clone(),
            remote.write_target.with_broadcast(broadcast)?,
        ),
        SlackRemoteDraftState::Absent => (
            create_client_message_id
                .or_else(|| draft.and_then(|draft| draft.client_message_id.clone()))
                .unwrap_or_else(SlackMessageClientId::generate),
            slack_draft_write_target(key, broadcast)?,
        ),
        SlackRemoteDraftState::Unknown => {
            return Err("Slack draft remote state is not hydrated".to_string());
        }
    };
    Ok(SlackDraftDesiredState::Present(Arc::new(
        SlackDraftDesiredRemote {
            token,
            client_message_id,
            write_target,
            content,
        },
    )))
}

pub(super) fn prepare_slack_draft_mutation(
    remote: &SlackRemoteDraftState,
    desired: &SlackDraftDesiredState,
) -> Result<Option<SlackDraftMutation>, String> {
    match (remote, desired) {
        (SlackRemoteDraftState::Absent, SlackDraftDesiredState::Absent { .. }) => Ok(None),
        (SlackRemoteDraftState::Present(remote), SlackDraftDesiredState::Present(desired))
            if remote.write_target == desired.write_target
                && remote.desired_content_fingerprint == desired.content =>
        {
            Ok(None)
        }
        (SlackRemoteDraftState::Absent, SlackDraftDesiredState::Present(desired)) => {
            Ok(Some(SlackDraftMutation::Create {
                client_message_id: desired.client_message_id.clone(),
                write_target: desired.write_target.clone(),
                content: desired.content.clone(),
            }))
        }
        (SlackRemoteDraftState::Present(remote), SlackDraftDesiredState::Present(desired)) => {
            Ok(Some(SlackDraftMutation::Update {
                target: remote.target.clone(),
                client_message_id: desired.client_message_id.clone(),
                client_mutation_timestamp:
                    crate::model::SlackDraftClientMutationTimestamp::from_unix_milliseconds(
                        chrono::Utc::now().timestamp_millis(),
                    )?,
                write_target: desired.write_target.clone(),
                content: desired.content.clone(),
            }))
        }
        (SlackRemoteDraftState::Present(remote), SlackDraftDesiredState::Absent { .. }) => {
            Ok(Some(SlackDraftMutation::Delete {
                target: remote.target.clone(),
            }))
        }
        (SlackRemoteDraftState::Unknown, _) => {
            Err("Slack draft remote state is not hydrated".to_string())
        }
        (
            _,
            SlackDraftDesiredState::WaitingForRemote { .. }
            | SlackDraftDesiredState::Pending { .. }
            | SlackDraftDesiredState::WaitingForLocalFiles { .. },
        ) => Err("Slack draft desired state is not ready for mutation".to_string()),
    }
}

fn slack_draft_write_target(
    key: &SlackComposerDraftKey,
    broadcast: bool,
) -> Result<SlackDraftWriteTarget, String> {
    match &key.destination {
        SlackComposerDestination::Conversation { conversation_id } => {
            SlackDraftWriteTarget::conversation(conversation_id.clone())
        }
        SlackComposerDestination::Thread {
            conversation_id,
            thread_timestamp,
        } => SlackDraftWriteTarget::thread(
            conversation_id.clone(),
            thread_timestamp.clone(),
            broadcast,
        ),
    }
}

pub(super) fn execute_slack_draft_mutation(
    workspace_api: &Arc<dyn WorkspaceApi>,
    mutation: &SlackDraftMutation,
) -> Result<SlackDraftMutationSuccess, String> {
    match mutation {
        SlackDraftMutation::Create {
            client_message_id,
            write_target,
            content,
        } => workspace_api
            .create_slack_draft(
                client_message_id,
                write_target,
                slack_draft_content(content),
                &slack_draft_content_file_ids(content),
            )
            .map(|receipt| SlackDraftMutationSuccess::Upserted(receipt.target)),
        SlackDraftMutation::Update {
            target,
            client_mutation_timestamp,
            write_target,
            content,
            ..
        } => workspace_api
            .update_slack_draft(
                SlackDraftUpdateTarget::new(target, client_mutation_timestamp),
                write_target,
                slack_draft_content(content),
                &slack_draft_content_file_ids(content),
            )
            .map(|receipt| SlackDraftMutationSuccess::Upserted(receipt.target)),
        SlackDraftMutation::Delete { target } => workspace_api
            .delete_slack_draft(target, SlackDraftFileDeletion::Delete)
            .map(|()| SlackDraftMutationSuccess::Deleted),
    }
}

fn slack_draft_content(
    content: &SlackDraftDesiredContentFingerprint,
) -> SlackDraftContent<'_, crate::model::SlackMessageDraft> {
    match content {
        SlackDraftDesiredContentFingerprint::Message { message, .. } => {
            SlackDraftContent::Message(message)
        }
        SlackDraftDesiredContentFingerprint::FilesOnly { .. } => SlackDraftContent::FilesOnly,
    }
}

fn slack_draft_content_file_ids(
    content: &SlackDraftDesiredContentFingerprint,
) -> Arc<[SlackFileId]> {
    match content {
        SlackDraftDesiredContentFingerprint::Message { file_ids, .. }
        | SlackDraftDesiredContentFingerprint::FilesOnly { file_ids } => file_ids.clone(),
    }
}
