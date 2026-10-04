use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::model::{
    SlackAuthenticatedRemoteDraftFiles, SlackDraftId, SlackDraftWriteTarget,
    SlackDraftWriteTargetView, SlackDraftsSentItem, SlackDraftsSentRequest,
    SlackDraftsSentSnapshot, SlackDraftsSentTab, SlackMessageClientId,
};

use super::{PreparedSlackRemoteDraft, PreparedSlackRemoteDrafts, WorkspaceApi};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDocument, SlackComposerDraftKey,
    SlackDraftDesiredContentFingerprint, SlackDraftSyncIdentity, SlackRemoteDraft,
};

pub(in crate::ui::surface::state) fn load_all_slack_remote_drafts(
    workspace_api: &Arc<dyn WorkspaceApi>,
    identity: &SlackDraftSyncIdentity,
) -> Result<PreparedSlackRemoteDrafts, String> {
    let mut cursor = None;
    let mut seen_cursors = HashSet::new();
    let mut seen_draft_ids = HashSet::new();
    let mut drafts = HashMap::new();

    loop {
        let snapshot = workspace_api.load_slack_drafts_sent(SlackDraftsSentRequest {
            team_id: identity.team_id.clone(),
            tab: SlackDraftsSentTab::Drafts,
            cursor,
        })?;
        validate_slack_draft_snapshot(&snapshot, identity)?;
        for item in snapshot.items {
            if !seen_draft_ids.insert(item.id.clone()) {
                return Err(format!(
                    "Slack drafts.list returned duplicate draft id {}",
                    item.id
                ));
            }
            let (key, draft) = prepare_slack_remote_draft(item, identity)?;
            if drafts.insert(key.clone(), draft).is_some() {
                return Err(format!(
                    "Slack drafts.list returned duplicate destination {}",
                    slack_draft_destination_label(&key)
                ));
            }
        }
        let Some(next_cursor) = snapshot.next_cursor else {
            break;
        };
        if !seen_cursors.insert(next_cursor.clone()) {
            return Err(format!(
                "Slack drafts.list cursor cycle detected at {}",
                next_cursor.as_str()
            ));
        }
        cursor = Some(next_cursor);
    }

    Ok(PreparedSlackRemoteDrafts { drafts })
}

fn validate_slack_draft_snapshot(
    snapshot: &SlackDraftsSentSnapshot,
    identity: &SlackDraftSyncIdentity,
) -> Result<(), String> {
    if snapshot.team_id != identity.team_id {
        return Err(format!(
            "Slack drafts.list returned team {} while hydrating team {}",
            snapshot.team_id, identity.team_id
        ));
    }
    if snapshot.tab != SlackDraftsSentTab::Drafts {
        return Err(format!(
            "Slack drafts.list returned the {} tab instead of Drafts",
            snapshot.tab.label()
        ));
    }
    Ok(())
}

fn prepare_slack_remote_draft(
    item: SlackDraftsSentItem,
    identity: &SlackDraftSyncIdentity,
) -> Result<(SlackComposerDraftKey, PreparedSlackRemoteDraft), String> {
    validate_slack_remote_draft_identity(&item, identity)?;
    let write_target = prepare_slack_draft_write_target(&item)?;
    let composer_destination = slack_composer_destination(&write_target);
    let key = SlackComposerDraftKey {
        team_id: identity.team_id.clone(),
        self_user_id: identity.self_user_id.clone(),
        destination: composer_destination,
    };
    let target = crate::model::SlackDraftTarget::new(item.id.clone(), item.revision.clone());
    let file_references = SlackAuthenticatedRemoteDraftFiles::from_loaded_draft(
        &identity.team_id,
        &identity.self_user_id,
        &target,
        &item,
    )?
    .into_references();
    let client_message_id = SlackMessageClientId::new(item.client_message_id).map_err(|error| {
        format!(
            "Slack draft {} has an invalid client message id: {error}",
            item.id
        )
    })?;
    let file_ids: Arc<[crate::model::SlackFileId]> = file_references
        .iter()
        .map(|reference| reference.file_id().clone())
        .collect::<Vec<_>>()
        .into();
    let (document, desired_content_fingerprint) =
        prepare_slack_draft_content(&item.id, item.body, item.rich_body, file_ids.clone())?;
    let broadcast = write_target.broadcast();
    let remote = Arc::new(SlackRemoteDraft {
        target,
        write_target,
        client_message_id,
        desired_content_fingerprint,
    });
    Ok((
        key,
        PreparedSlackRemoteDraft {
            document,
            broadcast,
            file_references,
            remote,
        },
    ))
}

fn validate_slack_remote_draft_identity(
    item: &SlackDraftsSentItem,
    identity: &SlackDraftSyncIdentity,
) -> Result<(), String> {
    if item.team_id != identity.team_id {
        return Err(format!(
            "Slack draft {} belongs to team {} instead of {}",
            item.id, item.team_id, identity.team_id
        ));
    }
    if item.user_id != identity.self_user_id {
        return Err(format!(
            "Slack draft {} belongs to user {} instead of {}",
            item.id, item.user_id, identity.self_user_id
        ));
    }
    if item.scheduled_unix_seconds != 0 {
        return Err(format!(
            "Slack draft {} is scheduled for {}; Drafts hydration only accepts unscheduled items",
            item.id, item.scheduled_unix_seconds
        ));
    }
    if item.destinations.len() != 1 {
        return Err(format!(
            "Slack draft {} has {} destinations; remote hydration requires exactly one",
            item.id,
            item.destinations.len()
        ));
    }
    Ok(())
}

fn prepare_slack_draft_write_target(
    item: &SlackDraftsSentItem,
) -> Result<SlackDraftWriteTarget, String> {
    let destination = item
        .destinations
        .first()
        .expect("Slack draft destination count was validated");
    let write_target = SlackDraftWriteTarget::from_loaded(destination).map_err(|error| {
        format!(
            "Slack draft {} has an invalid destination: {error}",
            item.id
        )
    })?;
    if write_target.message_timestamp().is_some() {
        return Err(format!(
            "Slack draft {} edits an existing message; edit drafts are unsupported",
            item.id
        ));
    }
    Ok(write_target)
}

fn slack_composer_destination(write_target: &SlackDraftWriteTarget) -> SlackComposerDestination {
    match write_target.view() {
        SlackDraftWriteTargetView::Conversation {
            conversation_id, ..
        } => SlackComposerDestination::Conversation {
            conversation_id: conversation_id.to_string(),
        },
        SlackDraftWriteTargetView::Thread {
            conversation_id,
            thread_timestamp,
            ..
        } => SlackComposerDestination::Thread {
            conversation_id: conversation_id.to_string(),
            thread_timestamp: thread_timestamp.clone(),
        },
    }
}

fn prepare_slack_draft_content(
    draft_id: &SlackDraftId,
    body: String,
    rich_body: Option<crate::model::SlackRichTextBody>,
    file_ids: Arc<[crate::model::SlackFileId]>,
) -> Result<(SlackComposerDocument, SlackDraftDesiredContentFingerprint), String> {
    let document = match rich_body {
        Some(rich_body) => SlackComposerDocument::from_rich_body(&rich_body)
            .map_err(|error| format!("Slack draft {draft_id} rich text is unsupported: {error}"))?,
        None if !body.trim().is_empty() => SlackComposerDocument::plain_text(body),
        None if !file_ids.is_empty() => {
            return Ok((
                SlackComposerDocument::default(),
                SlackDraftDesiredContentFingerprint::FilesOnly { file_ids },
            ));
        }
        None => {
            return Err(format!(
                "Slack draft {draft_id} has neither message content nor files"
            ));
        }
    };
    let message = document.export_message_draft().map_err(|error| {
        format!("Slack draft {draft_id} content cannot be preserved by the composer: {error}")
    })?;
    Ok((
        document,
        SlackDraftDesiredContentFingerprint::Message { message, file_ids },
    ))
}

pub(super) fn slack_draft_key_matches_identity(
    key: &SlackComposerDraftKey,
    identity: &SlackDraftSyncIdentity,
) -> bool {
    key.team_id == identity.team_id && key.self_user_id == identity.self_user_id
}

fn slack_draft_destination_label(key: &SlackComposerDraftKey) -> String {
    match &key.destination {
        SlackComposerDestination::Conversation { conversation_id } => {
            format!("conversation {conversation_id}")
        }
        SlackComposerDestination::Thread {
            conversation_id,
            thread_timestamp,
        } => format!(
            "thread {} in conversation {conversation_id}",
            thread_timestamp.as_str()
        ),
    }
}
