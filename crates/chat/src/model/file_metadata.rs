use std::{collections::HashSet, sync::Arc};

use crate::model::{
    SlackAttachment, SlackDraftId, SlackDraftTarget, SlackDraftsSentItem, SlackFileId,
};

/// A file reference proven to originate in an authenticated Slack draft.
///
/// The stable draft ID records the ownership origin. Draft revisions are deliberately excluded:
/// a successful autosave advances the revision without changing file ownership.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackRemoteDraftFileReference {
    team_id: String,
    owner_user_id: String,
    origin_draft_id: SlackDraftId,
    file_id: SlackFileId,
}

impl SlackRemoteDraftFileReference {
    pub fn team_id(&self) -> &str {
        &self.team_id
    }

    pub fn owner_user_id(&self) -> &str {
        &self.owner_user_id
    }

    pub fn origin_draft_id(&self) -> &SlackDraftId {
        &self.origin_draft_id
    }

    pub fn file_id(&self) -> &SlackFileId {
        &self.file_id
    }
}

/// Ordered remote file references decoded at the authenticated draft boundary.
///
/// There is intentionally no constructor from raw file IDs. IPC and destructive operations can
/// only receive the opaque references produced after the loaded draft's team, user, and target
/// have been validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackAuthenticatedRemoteDraftFiles {
    references: Arc<[SlackRemoteDraftFileReference]>,
}

impl SlackAuthenticatedRemoteDraftFiles {
    pub fn from_loaded_draft(
        authenticated_team_id: &str,
        authenticated_self_user_id: &str,
        target: &SlackDraftTarget,
        draft: &SlackDraftsSentItem,
    ) -> Result<Self, String> {
        if draft.team_id != authenticated_team_id {
            return Err(format!(
                "Slack draft {} belongs to team {} instead of authenticated team {authenticated_team_id}",
                draft.id, draft.team_id
            ));
        }
        if draft.user_id != authenticated_self_user_id {
            return Err(format!(
                "Slack draft {} belongs to user {} instead of authenticated user {authenticated_self_user_id}",
                draft.id, draft.user_id
            ));
        }
        if target.draft_id() != &draft.id || target.revision() != &draft.revision {
            return Err(format!(
                "Slack draft {} target does not match its loaded identity and revision",
                draft.id
            ));
        }
        let mut unique_file_ids = HashSet::with_capacity(draft.file_ids.len());
        let mut references = Vec::with_capacity(draft.file_ids.len());
        for file_id in &draft.file_ids {
            if !unique_file_ids.insert(file_id) {
                return Err(format!(
                    "Slack draft {} contains duplicate file id {file_id}",
                    draft.id
                ));
            }
            references.push(SlackRemoteDraftFileReference {
                team_id: authenticated_team_id.to_string(),
                owner_user_id: authenticated_self_user_id.to_string(),
                origin_draft_id: draft.id.clone(),
                file_id: file_id.clone(),
            });
        }
        Ok(Self {
            references: references.into(),
        })
    }

    pub fn references(&self) -> &[SlackRemoteDraftFileReference] {
        &self.references
    }

    pub fn into_references(self) -> Arc<[SlackRemoteDraftFileReference]> {
        self.references
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileMetadataRequest {
    references: Vec<SlackRemoteDraftFileReference>,
}

impl SlackFileMetadataRequest {
    pub fn new(references: Vec<SlackRemoteDraftFileReference>) -> Result<Self, String> {
        if references.is_empty() {
            return Err(
                "Slack file metadata request must contain at least one authenticated reference"
                    .to_string(),
            );
        }
        let mut unique_file_ids = HashSet::with_capacity(references.len());
        for reference in &references {
            if !unique_file_ids.insert(reference.file_id()) {
                return Err(format!(
                    "Slack file metadata request contains duplicate file id {}",
                    reference.file_id()
                ));
            }
        }
        Ok(Self { references })
    }

    pub fn references(&self) -> &[SlackRemoteDraftFileReference] {
        &self.references
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackFileSharing {
    Shared,
    Unshared,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackRemoteDraftFileDeletionEligibility {
    Eligible,
    Ineligible,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileMetadata {
    file_id: SlackFileId,
    owner_user_id: Option<String>,
    sharing: SlackFileSharing,
    deletion_eligibility: SlackRemoteDraftFileDeletionEligibility,
    attachment: Arc<SlackAttachment>,
}

impl SlackFileMetadata {
    pub fn new(
        file_id: SlackFileId,
        owner_user_id: Option<String>,
        sharing: SlackFileSharing,
        deletion_eligibility: SlackRemoteDraftFileDeletionEligibility,
        attachment: SlackAttachment,
    ) -> Self {
        Self {
            file_id,
            owner_user_id,
            sharing,
            deletion_eligibility,
            attachment: Arc::new(attachment),
        }
    }

    pub fn file_id(&self) -> &SlackFileId {
        &self.file_id
    }

    pub fn owner_user_id(&self) -> Option<&str> {
        self.owner_user_id.as_deref()
    }

    pub fn sharing(&self) -> SlackFileSharing {
        self.sharing
    }

    pub fn deletion_eligibility(&self) -> SlackRemoteDraftFileDeletionEligibility {
        self.deletion_eligibility
    }

    pub fn attachment(&self) -> &SlackAttachment {
        &self.attachment
    }

    pub fn attachment_arc(&self) -> Arc<SlackAttachment> {
        self.attachment.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackFileMetadataEntry {
    Loaded {
        reference: SlackRemoteDraftFileReference,
        metadata: SlackFileMetadata,
    },
    Failed {
        reference: SlackRemoteDraftFileReference,
        diagnostic: String,
    },
}

impl SlackFileMetadataEntry {
    pub fn loaded(
        reference: SlackRemoteDraftFileReference,
        metadata: SlackFileMetadata,
    ) -> Result<Self, String> {
        if reference.file_id() != metadata.file_id() {
            return Err(format!(
                "Slack file metadata returned file {} for requested {}",
                metadata.file_id(),
                reference.file_id()
            ));
        }
        Ok(Self::Loaded {
            reference,
            metadata,
        })
    }

    pub fn failed(reference: SlackRemoteDraftFileReference, diagnostic: String) -> Self {
        Self::Failed {
            reference,
            diagnostic,
        }
    }

    pub fn reference(&self) -> &SlackRemoteDraftFileReference {
        match self {
            Self::Loaded { reference, .. } | Self::Failed { reference, .. } => reference,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileMetadataBatch {
    entries: Vec<SlackFileMetadataEntry>,
}

impl SlackFileMetadataBatch {
    pub fn from_ordered_entries(
        request: &SlackFileMetadataRequest,
        entries: Vec<SlackFileMetadataEntry>,
    ) -> Result<Self, String> {
        if entries.len() != request.references.len() {
            return Err(format!(
                "Slack file metadata returned {} entries for {} requested files",
                entries.len(),
                request.references.len()
            ));
        }
        for (requested, returned) in request.references.iter().zip(&entries) {
            if returned.reference() != requested {
                return Err(format!(
                    "Slack file metadata returned reference for file {} while loading {}",
                    returned.reference().file_id(),
                    requested.file_id()
                ));
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[SlackFileMetadataEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> Vec<SlackFileMetadataEntry> {
        self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackRemoteDraftFileCleanupOutcome {
    PreservedShared,
    ConfirmedDeleted,
    RetainedUnknown { diagnostic: String },
    Failed { diagnostic: String },
}
