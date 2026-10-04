mod classification;

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use super::SlackLiveWorkspaceLoader;
use crate::live::{
    api::SlackObservedApiPost,
    payload::{message::attachments::slack_attachment_from_file, util::string_at},
};
use crate::model::{SlackAttachment, SlackFileId};
use crate::model::{
    SlackFileMetadata, SlackFileMetadataBatch, SlackFileMetadataEntry, SlackFileMetadataRequest,
    SlackFileSharing, SlackRemoteDraftFileCleanupOutcome, SlackRemoteDraftFileDeletionEligibility,
    SlackRemoteDraftFileReference,
};
use classification::{slack_file_sharing, slack_remote_draft_file_deletion_eligibility};

const SLACK_FILE_METADATA_CACHE_INTERVAL: Duration = Duration::from_secs(60);
const SLACK_FILE_METADATA_MAX_CONCURRENCY: usize = 4;

#[derive(Clone)]
pub(super) struct SlackHydratedFileMetadata {
    authoritative: SlackAuthoritativeFileMetadata,
    attachment: SlackAttachment,
}

impl SlackHydratedFileMetadata {
    pub(super) fn into_owner_and_attachment(self) -> (Option<String>, SlackAttachment) {
        (self.authoritative.owner_user_id, self.attachment)
    }
}

#[derive(Clone)]
struct SlackAuthoritativeFileMetadata {
    owner_user_id: Option<String>,
    sharing: SlackFileSharing,
    deletion_eligibility: SlackRemoteDraftFileDeletionEligibility,
}

struct SlackFetchedFileMetadata {
    authoritative: SlackAuthoritativeFileMetadata,
    attachment: Option<SlackAttachment>,
}

pub(super) struct CachedSlackFileMetadata {
    metadata: SlackHydratedFileMetadata,
    loaded_at: Instant,
}

impl SlackLiveWorkspaceLoader {
    pub fn load_file_metadata(
        &self,
        request: &SlackFileMetadataRequest,
    ) -> Result<SlackFileMetadataBatch, String> {
        if let [reference] = request.references() {
            return SlackFileMetadataBatch::from_ordered_entries(
                request,
                vec![self.load_file_metadata_reference(reference.clone())],
            );
        }

        let mut entries = Vec::with_capacity(request.references().len());
        for references in request
            .references()
            .chunks(SLACK_FILE_METADATA_MAX_CONCURRENCY)
        {
            let chunk_entries = thread::scope(|scope| {
                references
                    .iter()
                    .map(|reference| {
                        let reference = reference.clone();
                        let task_reference = reference.clone();
                        let task =
                            scope.spawn(move || self.load_file_metadata_reference(task_reference));
                        (reference, task)
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|(reference, task)| match task.join() {
                        Ok(entry) => entry,
                        Err(_) => SlackFileMetadataEntry::failed(
                            reference.clone(),
                            format!(
                                "Slack files.info request thread panicked for {}",
                                reference.file_id()
                            ),
                        ),
                    })
                    .collect::<Vec<_>>()
            });
            entries.extend(chunk_entries);
        }
        SlackFileMetadataBatch::from_ordered_entries(request, entries)
    }

    fn load_file_metadata_reference(
        &self,
        reference: SlackRemoteDraftFileReference,
    ) -> SlackFileMetadataEntry {
        match self.load_file_metadata_entry(reference.file_id()) {
            Ok(metadata) => {
                let failed_reference = reference.clone();
                SlackFileMetadataEntry::loaded(
                    reference,
                    SlackFileMetadata::new(
                        failed_reference.file_id().clone(),
                        metadata.authoritative.owner_user_id,
                        metadata.authoritative.sharing,
                        metadata.authoritative.deletion_eligibility,
                        metadata.attachment,
                    ),
                )
                .unwrap_or_else(|diagnostic| {
                    SlackFileMetadataEntry::failed(failed_reference, diagnostic)
                })
            }
            Err(diagnostic) => SlackFileMetadataEntry::failed(reference, diagnostic),
        }
    }

    pub fn cleanup_remote_draft_file(
        &self,
        reference: &SlackRemoteDraftFileReference,
    ) -> SlackRemoteDraftFileCleanupOutcome {
        if let Err(outcome) = validate_cleanup_workspace(reference, &self.team_id) {
            return outcome;
        }
        let authoritative = match self.fetch_file_metadata(reference.file_id()) {
            Ok(metadata) => metadata.authoritative,
            Err(diagnostic) => {
                return SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                    diagnostic: format!(
                        "Preserved remote draft file {} because Slack ownership metadata could not be confirmed: {diagnostic}",
                        reference.file_id()
                    ),
                };
            }
        };
        if let Err(outcome) = validate_cleanup_metadata(reference, &authoritative) {
            return outcome;
        }

        match self.api.post_observed(
            "files.delete",
            &[
                ("file", reference.file_id().as_str().to_string()),
                ("is_draft", true.to_string()),
            ],
        ) {
            SlackObservedApiPost::Accepted(_) => {
                self.file_metadata_cache
                    .lock()
                    .expect("Slack file metadata cache mutex poisoned")
                    .remove(reference.file_id());
                SlackRemoteDraftFileCleanupOutcome::ConfirmedDeleted
            }
            SlackObservedApiPost::NotSent { diagnostic }
            | SlackObservedApiPost::Rejected { diagnostic } => {
                SlackRemoteDraftFileCleanupOutcome::Failed { diagnostic }
            }
            SlackObservedApiPost::Unknown { diagnostic } => {
                SlackRemoteDraftFileCleanupOutcome::RetainedUnknown { diagnostic }
            }
        }
    }

    pub(super) fn load_file_metadata_entry(
        &self,
        file_id: &SlackFileId,
    ) -> Result<SlackHydratedFileMetadata, String> {
        if let Some(metadata) = self.cached_file_metadata(file_id)? {
            return Ok(metadata);
        }
        let fetch_lock = self
            .file_metadata_fetch_locks
            .lock()
            .map_err(|_| "Slack file metadata fetch locks mutex poisoned".to_string())?
            .entry(file_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _fetch_guard = fetch_lock
            .lock()
            .map_err(|_| format!("Slack file metadata fetch mutex poisoned for {file_id}"))?;
        if let Some(metadata) = self.cached_file_metadata(file_id)? {
            return Ok(metadata);
        }

        let fetched = self.fetch_file_metadata(file_id)?;
        let attachment = fetched.attachment.ok_or_else(|| {
            format!("Slack files.info returned file {file_id} without a title or name")
        })?;
        let metadata = SlackHydratedFileMetadata {
            authoritative: fetched.authoritative,
            attachment,
        };
        self.file_metadata_cache
            .lock()
            .map_err(|_| "Slack file metadata cache mutex poisoned".to_string())?
            .insert(
                file_id.clone(),
                CachedSlackFileMetadata {
                    metadata: metadata.clone(),
                    loaded_at: Instant::now(),
                },
            );
        Ok(metadata)
    }

    fn cached_file_metadata(
        &self,
        file_id: &SlackFileId,
    ) -> Result<Option<SlackHydratedFileMetadata>, String> {
        Ok(self
            .file_metadata_cache
            .lock()
            .map_err(|_| "Slack file metadata cache mutex poisoned".to_string())?
            .get(file_id)
            .filter(|cached| cached.loaded_at.elapsed() < SLACK_FILE_METADATA_CACHE_INTERVAL)
            .map(|cached| cached.metadata.clone()))
    }

    fn fetch_file_metadata(
        &self,
        file_id: &SlackFileId,
    ) -> Result<SlackFetchedFileMetadata, String> {
        let payload = self
            .api
            .post("files.info", &[("file", file_id.as_str().to_string())])?;
        let file = payload
            .get("file")
            .ok_or_else(|| format!("Slack files.info response omitted file {file_id}"))?;
        let returned_id = string_at(file, &["id"])
            .ok_or_else(|| "Slack files.info response omitted file.id".to_string())
            .and_then(|returned_id| {
                SlackFileId::parse(returned_id).map_err(|error| {
                    format!("Slack files.info returned an invalid file id: {error}")
                })
            })?;
        if &returned_id != file_id {
            return Err(format!(
                "Slack files.info returned file {returned_id} for requested {file_id}"
            ));
        }
        Ok(SlackFetchedFileMetadata {
            authoritative: SlackAuthoritativeFileMetadata {
                owner_user_id: string_at(file, &["user"]),
                sharing: slack_file_sharing(file),
                deletion_eligibility: slack_remote_draft_file_deletion_eligibility(file),
            },
            attachment: slack_attachment_from_file(file),
        })
    }
}

fn validate_cleanup_workspace(
    reference: &SlackRemoteDraftFileReference,
    team_id: &str,
) -> Result<(), SlackRemoteDraftFileCleanupOutcome> {
    if reference.team_id() != team_id {
        return Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
            diagnostic: format!(
                "Preserved remote draft file {} because its authenticated team no longer matches this workspace.",
                reference.file_id()
            ),
        });
    }
    Ok(())
}

fn validate_cleanup_metadata(
    reference: &SlackRemoteDraftFileReference,
    authoritative: &SlackAuthoritativeFileMetadata,
) -> Result<(), SlackRemoteDraftFileCleanupOutcome> {
    match authoritative.owner_user_id.as_deref() {
        Some(owner_user_id) if owner_user_id == reference.owner_user_id() => {}
        Some(owner_user_id) => {
            return Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                diagnostic: format!(
                    "Preserved remote draft file {} because Slack reports owner {owner_user_id}, not the authenticated draft owner.",
                    reference.file_id()
                ),
            });
        }
        None => {
            return Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                diagnostic: format!(
                    "Preserved remote draft file {} because Slack omitted its owner.",
                    reference.file_id()
                ),
            });
        }
    }
    match authoritative.sharing {
        SlackFileSharing::Shared => {
            return Err(SlackRemoteDraftFileCleanupOutcome::PreservedShared);
        }
        SlackFileSharing::Unknown => {
            return Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                diagnostic: format!(
                    "Preserved remote draft file {} because Slack did not provide authoritative sharing metadata.",
                    reference.file_id()
                ),
            });
        }
        SlackFileSharing::Unshared => {}
    }
    validate_cleanup_eligibility(reference, authoritative.deletion_eligibility)
}

fn validate_cleanup_eligibility(
    reference: &SlackRemoteDraftFileReference,
    eligibility: SlackRemoteDraftFileDeletionEligibility,
) -> Result<(), SlackRemoteDraftFileCleanupOutcome> {
    match eligibility {
        SlackRemoteDraftFileDeletionEligibility::Eligible => Ok(()),
        SlackRemoteDraftFileDeletionEligibility::Ineligible => {
            Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                diagnostic: format!(
                    "Preserved remote draft file {} because Slack identifies it as a Quip, post, or list file.",
                    reference.file_id()
                ),
            })
        }
        SlackRemoteDraftFileDeletionEligibility::Unknown => {
            Err(SlackRemoteDraftFileCleanupOutcome::RetainedUnknown {
                diagnostic: format!(
                    "Preserved remote draft file {} because Slack did not provide authoritative file-type eligibility.",
                    reference.file_id()
                ),
            })
        }
    }
}
