use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use gpui::SharedString;

use crate::ui::surface::{
    SlackComposerDocument, SlackComposerDraft, SlackMainComposerDraftHandle,
    SlackRemoteDraftFileLocator, SlackScheduleDraftOwner, SlackSendDraftSource,
};
use crate::ui::{SlackDraftsSentSnapshot, SlackDraftsSentTab};

mod prepare;

pub(crate) use prepare::prepare_slack_drafts_sent_snapshot;

#[derive(Clone)]
pub(crate) enum SlackDraftsSentRowTarget {
    Draft {
        conversation_id: SharedString,
        document: SlackComposerDocument,
    },
    Scheduled {
        edit: SlackScheduledEdit,
        document: SlackComposerDocument,
    },
    Conversation(SharedString),
    Message(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduledEdit {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) conversation_id: String,
    pub(crate) target: crate::model::SlackDraftTarget,
    pub(crate) client_message_id: crate::model::SlackMessageClientId,
    pub(crate) write_targets: Vec<crate::model::SlackDraftWriteTarget>,
    pub(crate) post_at_unix_seconds: i64,
    pub(crate) remote_files: Arc<[crate::model::SlackRemoteDraftFileReference]>,
}

#[derive(Clone)]
pub(crate) struct SlackActiveScheduledEdit {
    pub(crate) edit: SlackScheduledEdit,
    pub(crate) edit_draft_handle: SlackMainComposerDraftHandle,
    pub(crate) prior_draft_handle: SlackMainComposerDraftHandle,
    pub(crate) prior_draft: SlackComposerDraft,
    pub(crate) deferred_remote_removals: Vec<SlackRemoteDraftFileLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduledMutationIdentity {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone)]
pub(crate) enum SlackScheduledPendingMutation {
    Create {
        client_message_id: crate::model::SlackMessageClientId,
        write_target: crate::model::SlackDraftWriteTarget,
    },
    Promote {
        target: crate::model::SlackDraftTarget,
        write_target: crate::model::SlackDraftWriteTarget,
        client_mutation_timestamp: crate::model::SlackDraftClientMutationTimestamp,
    },
    Update {
        edit: SlackScheduledEdit,
        client_mutation_timestamp: crate::model::SlackDraftClientMutationTimestamp,
    },
}

#[derive(Clone)]
pub(crate) enum SlackScheduledPendingOrigin {
    Composer,
    ScheduledEdit {
        edit: Box<SlackScheduledEdit>,
        prior_draft_handle: SlackMainComposerDraftHandle,
        deferred_remote_removals: Vec<SlackRemoteDraftFileLocator>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackScheduledPendingPhase {
    Submitting,
    ReconcilingUnknown { attempt: u8, diagnostic: String },
    ProtectedUnknown { diagnostic: String },
}

#[derive(Clone)]
pub(crate) struct SlackSchedulePendingSubmission {
    pub(crate) identity: SlackScheduledMutationIdentity,
    pub(crate) owner: SlackScheduleDraftOwner,
    pub(crate) accepted_draft: SlackComposerDraft,
    pub(crate) mutation: SlackScheduledPendingMutation,
    pub(crate) origin: SlackScheduledPendingOrigin,
    pub(crate) post_at_unix_seconds: i64,
    pub(crate) file_ids: Arc<[crate::model::SlackFileId]>,
    pub(crate) local_files: Arc<[crate::model::SlackScheduledDraftLocalFile]>,
    pub(crate) workspace_api: Arc<dyn crate::ui::WorkspaceApi>,
    pub(crate) timezone: chrono_tz::Tz,
    pub(crate) phase: SlackScheduledPendingPhase,
}

#[derive(Clone)]
pub(crate) struct SlackComposerScheduleRecovery {
    pub(crate) identity: SlackScheduledMutationIdentity,
    pub(crate) owner: SlackScheduleDraftOwner,
    pub(crate) draft: SlackComposerDraft,
    pub(crate) file_ids: Arc<[crate::model::SlackFileId]>,
    pub(crate) workspace_api: Arc<dyn crate::ui::WorkspaceApi>,
    pub(crate) failure_diagnostic: String,
}

#[derive(Clone)]
pub(crate) struct SlackScheduledEditRecovery {
    pub(crate) edit: SlackScheduledEdit,
    pub(crate) source: SlackSendDraftSource,
    pub(crate) edit_draft: SlackComposerDraft,
    pub(crate) deferred_remote_removals: Vec<SlackRemoteDraftFileLocator>,
    pub(crate) failure_diagnostic: String,
}

pub(crate) struct SlackScheduledDraftDisposition {
    pub(crate) owner: SlackScheduleDraftOwner,
    pub(crate) draft: SlackComposerDraft,
    pub(crate) deferred_remote_removals: Vec<SlackRemoteDraftFileLocator>,
    pub(crate) cleanup_api: Option<Arc<dyn crate::ui::WorkspaceApi>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduledDeleteIdentity {
    pub(crate) mutation: SlackScheduledMutationIdentity,
    pub(crate) target: crate::model::SlackDraftTarget,
}

impl SlackScheduledDeleteIdentity {
    pub(crate) fn new(
        generation: u64,
        team_id: String,
        self_user_id: String,
        conversation_id: String,
        target: crate::model::SlackDraftTarget,
    ) -> Self {
        Self {
            mutation: SlackScheduledMutationIdentity {
                generation,
                team_id,
                self_user_id,
                conversation_id,
            },
            target,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SlackDraftRestore {
    pub(crate) conversation_id: SharedString,
    pub(crate) document: SlackComposerDocument,
    pub(crate) scheduled_edit: Option<SlackScheduledEdit>,
}

#[derive(Clone)]
pub(crate) struct SlackDraftsSentItemRow {
    pub(crate) id: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) destination: SharedString,
    pub(crate) body: SharedString,
    pub(crate) timestamp: SharedString,
    pub(crate) avatar_label: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) files: SlackDraftsSentFilePresentation,
    pub(crate) target: SlackDraftsSentRowTarget,
    pub(crate) sent_date_key: Option<SharedString>,
    pub(crate) sent_group_first: bool,
    pub(crate) sent_group_last: bool,
}

impl SlackDraftsSentItemRow {
    pub(crate) fn height(&self) -> f32 {
        if self.files.has_files() {
            120.0
        } else {
            68.0
        }
    }
}

#[derive(Clone)]
pub(crate) enum SlackDraftsSentFilePresentation {
    None,
    Pending {
        references: Arc<[crate::model::SlackRemoteDraftFileReference]>,
    },
    Loading {
        references: Arc<[crate::model::SlackRemoteDraftFileReference]>,
    },
    Loaded {
        references: Arc<[crate::model::SlackRemoteDraftFileReference]>,
        cards: Arc<[SlackDraftsSentFileCard]>,
    },
}

impl SlackDraftsSentFilePresentation {
    pub(crate) fn from_authenticated_references(
        references: Arc<[crate::model::SlackRemoteDraftFileReference]>,
    ) -> Self {
        if references.is_empty() {
            Self::None
        } else {
            Self::Pending { references }
        }
    }

    pub(crate) fn references(&self) -> Option<&Arc<[crate::model::SlackRemoteDraftFileReference]>> {
        match self {
            Self::None => None,
            Self::Pending { references }
            | Self::Loading { references }
            | Self::Loaded { references, .. } => Some(references),
        }
    }

    pub(crate) fn pending_references(
        &self,
    ) -> Option<&Arc<[crate::model::SlackRemoteDraftFileReference]>> {
        match self {
            Self::Pending { references } => Some(references),
            Self::None | Self::Loading { .. } | Self::Loaded { .. } => None,
        }
    }

    pub(crate) fn has_files(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone)]
pub(crate) enum SlackDraftsSentFileCard {
    Loading,
    Loaded(Arc<crate::ui::SlackAttachment>),
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackDraftsSentFileMetadataTarget {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) tab: SlackDraftsSentTab,
    pub(crate) row_id: SharedString,
    pub(crate) references: Arc<[crate::model::SlackRemoteDraftFileReference]>,
}

#[derive(Default)]
pub(crate) struct SlackDraftsSentFileMetadataCoordinator {
    queued: VecDeque<SlackDraftsSentFileMetadataTarget>,
    active: HashSet<SlackDraftsSentFileMetadataTarget>,
}

impl SlackDraftsSentFileMetadataCoordinator {
    pub(crate) fn enqueue(&mut self, target: SlackDraftsSentFileMetadataTarget) {
        if self.queued.contains(&target) || self.active.contains(&target) {
            return;
        }
        self.queued.push_back(target);
    }

    pub(crate) fn start_next(
        &mut self,
        max_active: usize,
    ) -> Option<SlackDraftsSentFileMetadataTarget> {
        if self.active.len() >= max_active {
            return None;
        }
        let target = self.queued.pop_front()?;
        self.active.insert(target.clone());
        Some(target)
    }

    pub(crate) fn finish(&mut self, target: &SlackDraftsSentFileMetadataTarget) -> bool {
        self.active.remove(target)
    }

    pub(crate) fn reset(&mut self) {
        self.queued.clear();
        self.active.clear();
    }
}

#[derive(Clone)]
pub(crate) enum SlackDraftsSentRow {
    DateDivider {
        element_id: SharedString,
        label: SharedString,
    },
    Item(Arc<SlackDraftsSentItemRow>),
}

impl SlackDraftsSentRow {
    pub(crate) fn height(&self) -> f32 {
        match self {
            Self::DateDivider { .. } => 54.0,
            Self::Item(row) => row.height(),
        }
    }
}

pub(crate) struct PreparedSlackDraftsSentSnapshot {
    pub(crate) snapshot: SlackDraftsSentSnapshot,
    pub(crate) authenticated_self_user_id: String,
    pub(crate) rows: Arc<[SlackDraftsSentRow]>,
}
