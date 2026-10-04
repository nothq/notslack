#[cfg(test)]
use super::SlackAttachment;
use super::{
    Arc, Image, Range, SharedString, SlackMainTab, SlackMessageTimestamp, SlackUploadFile,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackAuxPanelState {
    pub title: String,
    pub subtitle: Option<String>,
    pub query: Option<String>,
    pub query_behavior: Option<SlackAuxPanelQueryBehavior>,
    pub sections: Vec<SlackAuxPanelSection>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackAuxPanelQueryBehavior {
    #[default]
    SearchWorkspace,
    Emoji,
    Mention,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackAuxPanelSection {
    pub title: Option<String>,
    pub rows: Vec<SlackAuxPanelRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackAuxPanelRow {
    pub label: String,
    pub emoji_glyph: Option<SharedString>,
    pub detail: Option<String>,
    pub accessory: Option<String>,
    pub image_url: Option<String>,
    pub image_base64: Option<String>,
    pub image_mimetype: Option<String>,
    pub muted: bool,
    pub action: Option<SlackAuxPanelRowAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackAuxPanelRowAction {
    SelectConversation(String),
    OpenProfile(String),
    SelectMainTab(SlackMainTab),
    OpenMembersPanel,
    InsertComposerSnippet(SharedString),
    InsertComposerUserMention {
        user_id: String,
        label: String,
    },
    InsertComposerBroadcastMention(crate::model::SlackRichTextBroadcastRange),
    #[cfg(test)]
    AttachComposerAttachment(Box<SlackAttachment>),
    OpenSearchQuery(String),
    SendDraftNow,
    #[cfg(test)]
    ToggleAttachmentPlayback(String),
    #[cfg(test)]
    ToggleAttachmentTranscript(String),
    ToggleAttachmentExpanded(String),
    #[cfg(test)]
    CycleAttachmentPlaybackSpeed(String),
    OpenExternalConnection(String, String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackComposerTarget {
    Main,
    Reply(SlackReplyComposerTarget),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMentionPickerState {
    Toolbar {
        target: SlackComposerTarget,
    },
    Inline {
        target: SlackComposerTarget,
        range: Range<usize>,
        document_revision: u64,
        dismissed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMentionInsertionMode {
    Append,
    Replace(Range<usize>),
    Invalid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackComposerFormatAction {
    #[default]
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Link,
    OrderedList,
    BulletedList,
    Quote,
    Code,
    CodeBlock,
}

impl SlackComposerFormatAction {
    pub(crate) const TOOLBAR_CONTROLS: [Self; 10] = [
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::Strikethrough,
        Self::Link,
        Self::OrderedList,
        Self::BulletedList,
        Self::Quote,
        Self::Code,
        Self::CodeBlock,
    ];

    pub(crate) const fn toolbar_index(self) -> usize {
        match self {
            Self::Bold => 0,
            Self::Italic => 1,
            Self::Underline => 2,
            Self::Strikethrough => 3,
            Self::Link => 4,
            Self::OrderedList => 5,
            Self::BulletedList => 6,
            Self::Quote => 7,
            Self::Code => 8,
            Self::CodeBlock => 9,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackComposerDestination {
    Conversation {
        conversation_id: String,
    },
    Thread {
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackComposerDraftKey {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) destination: SlackComposerDestination,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackReplyComposerTarget {
    ThreadPanel {
        panel_generation: u64,
        parent_message_id: SharedString,
        draft_key: SlackComposerDraftKey,
    },
    AllThreads {
        thread_key: SharedString,
        draft_key: SlackComposerDraftKey,
        broadcast_supported: bool,
    },
}

impl SlackReplyComposerTarget {
    pub(crate) fn draft_key(&self) -> &SlackComposerDraftKey {
        match self {
            Self::ThreadPanel { draft_key, .. } | Self::AllThreads { draft_key, .. } => draft_key,
        }
    }

    pub(crate) fn all_threads_key(&self) -> Option<&SharedString> {
        match self {
            Self::ThreadPanel { .. } => None,
            Self::AllThreads { thread_key, .. } => Some(thread_key),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackThreadDraftHandle {
    key: SlackComposerDraftKey,
    draft_id: SlackComposerDraftId,
}

impl SlackThreadDraftHandle {
    pub(crate) fn new(key: SlackComposerDraftKey, draft_id: SlackComposerDraftId) -> Self {
        Self { key, draft_id }
    }

    pub(crate) fn key(&self) -> &SlackComposerDraftKey {
        &self.key
    }

    pub(crate) fn draft_id(&self) -> SlackComposerDraftId {
        self.draft_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackPendingThreadReplySend {
    pub(crate) generation: u64,
    pub(crate) draft_token: u64,
    pub(crate) draft_document_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackNewMessageDraftKey {
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) draft_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackComposerDraftId(std::num::NonZeroU64);

impl Default for SlackComposerDraftId {
    fn default() -> Self {
        Self(std::num::NonZeroU64::MIN)
    }
}

impl SlackComposerDraftId {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackComposerIdAllocator {
    active_draft_id: SlackComposerDraftId,
    draft_generation: std::num::NonZeroU64,
    file_generation: u64,
}

impl Default for SlackComposerIdAllocator {
    fn default() -> Self {
        Self {
            active_draft_id: SlackComposerDraftId::default(),
            draft_generation: std::num::NonZeroU64::MIN,
            file_generation: 0,
        }
    }
}

impl SlackComposerIdAllocator {
    pub(crate) fn active_draft_id(&self) -> SlackComposerDraftId {
        self.active_draft_id
    }

    pub(crate) fn set_active_draft_id(&mut self, id: SlackComposerDraftId) {
        assert!(
            id.0 <= self.draft_generation,
            "Slack composer cannot restore a draft id that this surface did not allocate"
        );
        self.active_draft_id = id;
    }

    pub(crate) fn replace_active_draft_id(&mut self) -> SlackComposerDraftId {
        let next = self.next_draft_id();
        self.active_draft_id = next;
        next
    }

    pub(crate) fn next_draft_id(&mut self) -> SlackComposerDraftId {
        let generation = self
            .draft_generation
            .get()
            .checked_add(1)
            .expect("Slack composer draft id generation overflowed");
        self.draft_generation =
            std::num::NonZeroU64::new(generation).expect("incremented draft id must be non-zero");
        SlackComposerDraftId(self.draft_generation)
    }

    pub(crate) fn next_file_id(&mut self) -> SlackComposerFileId {
        self.file_generation = self
            .file_generation
            .checked_add(1)
            .expect("Slack composer file id generation overflowed");
        SlackComposerFileId::from_generation(self.file_generation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlackComposerFileId(std::num::NonZeroU64);

impl SlackComposerFileId {
    pub(crate) fn from_generation(generation: u64) -> Self {
        Self(
            std::num::NonZeroU64::new(generation)
                .expect("Slack composer file ids must use a non-zero generation"),
        )
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let generation = value
            .strip_prefix("composer-file-")
            .ok_or_else(|| "Slack composer file id must start with composer-file-.".to_string())?
            .parse::<u64>()
            .map_err(|error| {
                format!("Slack composer file id has an invalid generation: {error}")
            })?;
        let generation = std::num::NonZeroU64::new(generation)
            .ok_or_else(|| "Slack composer file id generation must be non-zero.".to_string())?;
        Ok(Self(generation))
    }
}

impl std::fmt::Display for SlackComposerFileId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "composer-file-{}", self.0.get())
    }
}

#[derive(Clone, Debug)]
pub struct SlackPreparedUploadFile {
    upload: SlackUploadFile,
    preview: Option<Arc<Image>>,
}

impl SlackPreparedUploadFile {
    pub(crate) fn new(upload: SlackUploadFile, preview: Option<Arc<Image>>) -> Self {
        Self { upload, preview }
    }

    pub(crate) fn from_upload(upload: SlackUploadFile) -> Self {
        Self {
            upload,
            preview: None,
        }
    }

    pub(crate) fn mimetype(&self) -> &str {
        self.upload.mimetype()
    }

    pub(crate) fn into_parts(self) -> (SlackUploadFile, Option<Arc<Image>>) {
        (self.upload, self.preview)
    }
}
