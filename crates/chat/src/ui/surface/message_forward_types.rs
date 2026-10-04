use std::sync::Arc;

use gpui::{SharedString, UniformListScrollHandle};

use crate::ui::{SlackDestinationTarget, SlackMessageTimestamp};

use super::{SlackNewMessageCandidateKind, SlackNewMessageCandidateRow};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageForwardSource {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
    pub(crate) private_message: bool,
    pub(crate) author: SharedString,
    pub(crate) timestamp_label: SharedString,
    pub(crate) preview: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageForwardDestination {
    pub(crate) target: SlackDestinationTarget,
    pub(crate) kind: SlackNewMessageCandidateKind,
    pub(crate) label: SharedString,
    pub(crate) conversation_id: Option<SharedString>,
    pub(crate) opening: bool,
}

pub(crate) struct SlackMessageForwardModal {
    pub(crate) generation: u64,
    pub(crate) source: SlackMessageForwardSource,
    pub(crate) rows: Arc<[SlackNewMessageCandidateRow]>,
    pub(crate) visible_row_indices: Arc<[usize]>,
    pub(crate) scroll_handle: UniformListScrollHandle,
    pub(crate) query: String,
    pub(crate) normalized_query: SharedString,
    pub(crate) selected_index: Option<usize>,
    pub(crate) destination: Option<SlackMessageForwardDestination>,
    pub(crate) note: String,
    pub(crate) directory_loading: bool,
    pub(crate) forwarding: bool,
    pub(crate) copying_link: bool,
    pub(crate) link_copied: bool,
    pub(crate) error: Option<String>,
}
