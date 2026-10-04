use super::{
    Arc, ListState, Range, RefCell, SharedString, SlackComposerDocument, SlackComposerDraftId,
    SlackComposerDraftKey, SlackComposerFiles, SlackComposerFormatAction, SlackComposerLinkUrl,
    SlackFileStagingDraftOwner, SlackLaterItemKey, SlackMessageClientId, SlackMessageRow,
    SlackReplyComposerTarget, SlackThreadDraftHandle,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackMainComposerDraftOwner {
    Conversation(SlackComposerDraftKey),
    NewMessage(super::SlackNewMessageDraftKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMainComposerTarget {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMainComposerNotice {
    Message(SharedString),
    NotificationsPaused {
        conversation_label: SharedString,
    },
    PeerLocalTime {
        conversation_label: SharedString,
        timezone: crate::model::SlackIanaTimezone,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMainComposerPresentation {
    pub(crate) conversation_label: SharedString,
    pub(crate) placeholder: SharedString,
    pub(crate) private_channel: bool,
    pub(crate) notice: Option<SlackMainComposerNotice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackActiveMainComposerContext {
    pub(crate) owner: SlackMainComposerDraftOwner,
    pub(crate) source: SlackSendDraftSource,
    pub(crate) target: SlackMainComposerTarget,
    pub(crate) presentation: SlackMainComposerPresentation,
}

impl SlackActiveMainComposerContext {
    pub(crate) fn new(
        owner: SlackMainComposerDraftOwner,
        source: SlackSendDraftSource,
        target: SlackMainComposerTarget,
        presentation: SlackMainComposerPresentation,
    ) -> Self {
        match (&owner, &source) {
            (
                SlackMainComposerDraftOwner::Conversation(key),
                SlackSendDraftSource::Conversation { conversation_id }
                | SlackSendDraftSource::Activity {
                    conversation_id, ..
                },
            ) => {
                let super::SlackComposerDestination::Conversation {
                    conversation_id: owner_conversation_id,
                } = &key.destination
                else {
                    panic!("main conversation composer cannot own a thread draft");
                };
                assert!(
                    key.team_id == target.team_id
                        && key.self_user_id == target.self_user_id
                        && owner_conversation_id == &target.conversation_id
                        && conversation_id == &target.conversation_id,
                    "Slack conversation composer owner, source, and target must match exactly"
                );
            }
            (
                SlackMainComposerDraftOwner::NewMessage(key),
                SlackSendDraftSource::NewMessage { draft_key },
            ) => {
                assert!(
                    key.team_id == target.team_id
                        && key.self_user_id == target.self_user_id
                        && key.draft_key == *draft_key,
                    "Slack new-message composer owner, source, and target must match exactly"
                );
            }
            _ => panic!("Slack main composer owner and source kinds must match"),
        }
        Self {
            owner,
            source,
            target,
            presentation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackMainComposerDraftHandle {
    pub(crate) owner: SlackMainComposerDraftOwner,
    pub(crate) draft_id: SlackComposerDraftId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackSendDraftSource {
    Conversation {
        conversation_id: String,
    },
    NewMessage {
        draft_key: String,
    },
    Activity {
        item_key: SharedString,
        conversation_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackScheduleDraftOwner {
    Main {
        handle: SlackMainComposerDraftHandle,
        source: SlackSendDraftSource,
    },
    ThreadPanel {
        handle: SlackThreadDraftHandle,
        panel_generation: u64,
    },
    AllThreads {
        handle: SlackThreadDraftHandle,
        thread_key: SharedString,
    },
}

pub(crate) struct SlackScheduleButtonPresentation {
    pub(crate) element_id: SharedString,
    pub(crate) owner: Option<SlackScheduleDraftOwner>,
    pub(crate) enabled: bool,
    pub(crate) group_enabled: bool,
    pub(crate) editing: bool,
}

impl SlackScheduleDraftOwner {
    pub(crate) fn draft_id(&self) -> SlackComposerDraftId {
        match self {
            Self::Main { handle, .. } => handle.draft_id,
            Self::ThreadPanel { handle, .. } | Self::AllThreads { handle, .. } => handle.draft_id(),
        }
    }

    pub(crate) fn draft_key(&self) -> Option<&SlackComposerDraftKey> {
        match self {
            Self::Main {
                handle:
                    SlackMainComposerDraftHandle {
                        owner: SlackMainComposerDraftOwner::Conversation(key),
                        ..
                    },
                ..
            } => Some(key),
            Self::ThreadPanel { handle, .. } | Self::AllThreads { handle, .. } => {
                Some(handle.key())
            }
            Self::Main {
                handle:
                    SlackMainComposerDraftHandle {
                        owner: SlackMainComposerDraftOwner::NewMessage(_),
                        ..
                    },
                ..
            } => None,
        }
    }

    pub(crate) fn file_staging_owner(&self) -> SlackFileStagingDraftOwner {
        match self {
            Self::Main { handle, .. } => SlackFileStagingDraftOwner::Main(handle.owner.clone()),
            Self::ThreadPanel { handle, .. } | Self::AllThreads { handle, .. } => {
                SlackFileStagingDraftOwner::Thread(handle.key().clone())
            }
        }
    }

    pub(crate) fn matches_owned_draft(
        &self,
        owner: &SlackFileStagingDraftOwner,
        draft_id: SlackComposerDraftId,
    ) -> bool {
        if self.draft_id() != draft_id {
            return false;
        }
        match (self, owner) {
            (Self::Main { handle, .. }, SlackFileStagingDraftOwner::Main(file_owner)) => {
                handle.owner == *file_owner
            }
            (
                Self::ThreadPanel { handle, .. } | Self::AllThreads { handle, .. },
                SlackFileStagingDraftOwner::Thread(key),
            ) => handle.key() == key,
            (Self::Main { .. }, SlackFileStagingDraftOwner::Thread(_))
            | (
                Self::ThreadPanel { .. } | Self::AllThreads { .. },
                SlackFileStagingDraftOwner::Main(_),
            ) => false,
        }
    }

    pub(crate) fn main_source(&self) -> Option<&SlackSendDraftSource> {
        match self {
            Self::Main { source, .. } => Some(source),
            Self::ThreadPanel { .. } | Self::AllThreads { .. } => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SlackComposerDraft {
    pub(crate) id: SlackComposerDraftId,
    pub(crate) token: u64,
    pub(crate) client_message_id: Option<SlackMessageClientId>,
    pub(crate) document: SlackComposerDocument,
    pub(crate) files: SlackComposerFiles,
    pub(crate) broadcast: bool,
}

impl SlackComposerDraft {
    pub(crate) fn new(id: SlackComposerDraftId) -> Self {
        Self {
            id,
            token: 0,
            client_message_id: None,
            document: SlackComposerDocument::default(),
            files: SlackComposerFiles::default(),
            broadcast: false,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.document.has_message_content() && self.files.is_empty() && !self.broadcast
    }

    pub(crate) fn text(&self) -> &str {
        self.document.text()
    }

    pub(crate) fn replace_text(&mut self, text: &str, token: u64) -> bool {
        if self.document.text() == text {
            return false;
        }
        self.document.apply_text_edit(text);
        self.token = token;
        self.client_message_id = None;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackComposerLinkTarget {
    Main {
        team_id: String,
        conversation_id: String,
        source: SlackSendDraftSource,
        draft_revision: u64,
        document_revision: u64,
    },
    Reply {
        target: SlackReplyComposerTarget,
        document_revision: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackComposerLinkDialog {
    pub(crate) target: SlackComposerLinkTarget,
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
    pub(crate) raw_url: String,
    pub(crate) parsed_url: Option<SlackComposerLinkUrl>,
    pub(crate) editing_existing_link: bool,
    pub(crate) error: Option<String>,
}

impl SlackComposerLinkDialog {
    pub(crate) fn set_url(&mut self, raw_url: String) {
        self.parsed_url = SlackComposerLinkUrl::parse(&raw_url);
        self.raw_url = raw_url;
        self.error = None;
    }

    pub(crate) fn can_save(&self) -> bool {
        !self.text.trim().is_empty() && self.parsed_url.is_some()
    }
}

#[derive(Clone)]
pub(crate) struct SlackThreadPanelState {
    pub(crate) origin: SlackThreadPanelOrigin,
    pub(crate) conversation_id: String,
    pub(crate) conversation_name: String,
    pub(crate) parent_message_id: String,
    pub(crate) timezone: chrono_tz::Tz,
    pub(crate) parent_row: SlackMessageRow,
    pub(crate) parent_hydrated: bool,
    pub(crate) reply_rows: Arc<[SlackMessageRow]>,
    pub(crate) list_rows: Arc<[SlackThreadListRow]>,
    pub(crate) list_state: ListState,
    pub(crate) expected_reply_count: u32,
    pub(crate) reply_label: SharedString,
    pub(crate) broadcast_label: Option<SharedString>,
    pub(crate) generation: u64,
    pub(crate) next_cursor: Option<String>,
    pub(crate) pagination_initialized: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) reply_draft_key: SlackComposerDraftKey,
    pub(crate) reply_draft: RefCell<SlackComposerDraft>,
    pub(crate) reply_formatting_enabled: bool,
    pub(crate) reply_format_roving_target: SlackComposerFormatAction,
    pub(crate) reply_composer_focused: bool,
    pub(crate) reply_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackThreadListRow {
    Parent,
    ReplyDivider,
    Reply(usize),
    Loading,
    Error,
    Composer,
}

impl SlackThreadPanelState {
    pub(crate) fn is_cold_loading(&self) -> bool {
        self.loading && !self.parent_hydrated && self.reply_rows.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackThreadPanelOrigin {
    Conversation,
    Later {
        item_key: SlackLaterItemKey,
        selected_message_id: SharedString,
    },
}

impl SlackThreadPanelOrigin {
    pub(crate) fn is_conversation(&self) -> bool {
        matches!(self, Self::Conversation)
    }

    pub(crate) fn later_item_key(&self) -> Option<&SlackLaterItemKey> {
        match self {
            Self::Conversation => None,
            Self::Later { item_key, .. } => Some(item_key),
        }
    }

    pub(crate) fn later_selected_message_id(&self) -> Option<&str> {
        match self {
            Self::Conversation => None,
            Self::Later {
                selected_message_id,
                ..
            } => Some(selected_message_id.as_ref()),
        }
    }
}
