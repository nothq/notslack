use crate::ui::{SlackMessageDraft, SlackMessageTimestamp};

use super::SlackMessageForwardSource;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageEditTarget {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageEditRequest {
    pub(crate) generation: u64,
    pub(crate) target: SlackMessageEditTarget,
    pub(crate) draft: SlackMessageDraft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageDeleteRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageDeleteModal {
    pub(crate) source: SlackMessageForwardSource,
    pub(crate) deleting: bool,
    pub(crate) error: Option<String>,
}
