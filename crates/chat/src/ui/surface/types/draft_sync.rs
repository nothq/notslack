use std::sync::Arc;

use crate::model::{SlackDraftClientMutationTimestamp, SlackDraftWriteTarget};
use crate::model::{SlackDraftTarget, SlackFileId, SlackMessageClientId, SlackMessageDraft};

use super::SlackComposerDraftKey;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackDraftSyncIdentity {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackDraftSyncState {
    #[default]
    Inactive,
    Loading,
    Synchronized,
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackRemoteDraftState {
    #[default]
    Unknown,
    Absent,
    Present(Arc<SlackRemoteDraft>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackRemoteDraft {
    pub(crate) target: SlackDraftTarget,
    pub(crate) write_target: SlackDraftWriteTarget,
    pub(crate) client_message_id: SlackMessageClientId,
    pub(crate) desired_content_fingerprint: SlackDraftDesiredContentFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackDraftDesiredContentFingerprint {
    Message {
        message: SlackMessageDraft,
        file_ids: Arc<[SlackFileId]>,
    },
    FilesOnly {
        file_ids: Arc<[SlackFileId]>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackDraftLocalPresence {
    Present,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackDraftDesiredState {
    WaitingForRemote {
        token: u64,
        local_presence: SlackDraftLocalPresence,
        create_client_message_id: Option<SlackMessageClientId>,
    },
    Pending {
        token: u64,
        local_presence: SlackDraftLocalPresence,
        create_client_message_id: Option<SlackMessageClientId>,
    },
    WaitingForLocalFiles {
        token: u64,
        create_client_message_id: Option<SlackMessageClientId>,
    },
    Absent {
        token: u64,
    },
    Present(Arc<SlackDraftDesiredRemote>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDraftDesiredRemote {
    pub(crate) token: u64,
    pub(crate) client_message_id: SlackMessageClientId,
    pub(crate) write_target: SlackDraftWriteTarget,
    pub(crate) content: SlackDraftDesiredContentFingerprint,
}

#[derive(Clone, Debug)]
pub(crate) enum SlackDraftMutation {
    Create {
        client_message_id: SlackMessageClientId,
        write_target: SlackDraftWriteTarget,
        content: SlackDraftDesiredContentFingerprint,
    },
    Update {
        target: SlackDraftTarget,
        client_message_id: SlackMessageClientId,
        client_mutation_timestamp: SlackDraftClientMutationTimestamp,
        write_target: SlackDraftWriteTarget,
        content: SlackDraftDesiredContentFingerprint,
    },
    Delete {
        target: SlackDraftTarget,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct SlackDraftMutationRequest {
    pub(crate) identity_generation: u64,
    pub(crate) serial: u64,
    pub(crate) key: SlackComposerDraftKey,
    pub(crate) token: u64,
    pub(crate) attempt: u8,
    pub(crate) mutation: SlackDraftMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackDraftFailureRecovery {
    NextSemanticChange,
    ComposerFileLimit,
    AuthoritativeHydration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackDraftMutationState {
    Debouncing {
        token: u64,
        local_presence: SlackDraftLocalPresence,
    },
    InFlight {
        serial: u64,
        token: u64,
    },
    RetryScheduled {
        serial: u64,
        token: u64,
    },
    Rehydrating {
        token: u64,
    },
    Scheduling {
        generation: u64,
        token: u64,
    },
    Failed {
        token: u64,
        recovery: SlackDraftFailureRecovery,
    },
}
