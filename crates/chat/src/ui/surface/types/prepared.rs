mod profiles;

pub(crate) use profiles::{
    SlackActivationLoadProfile, SlackActivationStageFrame, SlackConversationLoadProfile,
    SlackConversationLoadResult,
};

use super::{
    Arc, Date, HashMap, HashSet, Image, Range, SharedString, SlackAttachmentRow,
    SlackComposerDraft, SlackConversationHistoryCursor, SlackConversationKind,
    SlackConversationSnapshot, SlackDmInboxSnapshot, SlackMainComposerDraftHandle, SlackMainRoute,
    SlackMediaHostId, SlackMessageActionTarget, SlackMessageChunk, SlackMessageClientId,
    SlackMessageRow, SlackMessageSendReceipt, SlackMessageTimestamp, SlackQuickSearchSnapshot,
    SlackReaction, SlackReactionRow, SlackSearchSnapshot, SlackSendDraftSource, SlackShellSnapshot,
    SlackSidebarRow, SlackSidebarSnapshot, SlackThreadReplyReceipt, SlackThreadSnapshot,
    SlackUserPresence, SlackWorkspace,
};
use crate::ui::SlackMessageDraft;

#[derive(Clone)]
pub(crate) struct PreparedSlackWorkspace {
    pub(crate) workspace: SlackWorkspace,
    pub(crate) message_rows: Arc<[SlackMessageRow]>,
    pub(crate) message_rows_local_today: Date,
    pub(crate) message_chunks: Arc<[SlackMessageChunk]>,
    pub(crate) sidebar_rows: Arc<[SlackSidebarRow]>,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
    pub(crate) active_conversation_was_muted: bool,
}

#[derive(Clone)]
pub(crate) struct PreparedSlackShellSnapshot {
    pub(crate) snapshot: SlackShellSnapshot,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
}

#[derive(Clone)]
pub(crate) struct PreparedSlackSidebarSnapshot {
    pub(crate) snapshot: SlackSidebarSnapshot,
    pub(crate) rows: Arc<[SlackSidebarRow]>,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
    pub(crate) active_conversation_was_muted: bool,
}

#[derive(Clone)]
pub(crate) struct PreparedSlackDmInboxSnapshot {
    pub(crate) snapshot: SlackDmInboxSnapshot,
    pub(crate) rows: Arc<[SlackDmRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDmRow {
    pub(crate) conversation_id: SharedString,
    pub(crate) latest_message_timestamp: SlackMessageTimestamp,
    pub(crate) kind: SlackConversationKind,
    pub(crate) title: SharedString,
    pub(crate) finder_element_id: SharedString,
    pub(crate) finder_accessibility_label: SharedString,
    pub(crate) finder_search_key: SharedString,
    pub(crate) finder_group_count_label: Option<SharedString>,
    pub(crate) finder_avatar_initials: SharedString,
    pub(crate) finder_avatar_fill: u32,
    pub(crate) preview: SharedString,
    pub(crate) timestamp_label: SharedString,
    pub(crate) participants: Arc<[SlackDmRowParticipant]>,
    pub(crate) unread: bool,
    pub(crate) mention_count: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackDmRowParticipant {
    pub(crate) user_id: SharedString,
    pub(crate) label: SharedString,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) presence: Option<SlackUserPresence>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackSearchRow {
    pub(crate) result_id: SharedString,
    pub(crate) result_element_id: SharedString,
    pub(crate) action_target: Arc<SlackMessageActionTarget>,
    pub(crate) conversation_label: SharedString,
    pub(crate) author: SharedString,
    pub(crate) avatar_text: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) timestamp_label: SharedString,
    pub(crate) full_timestamp_label: SharedString,
    pub(crate) body_preview: SharedString,
    pub(crate) text_highlights: Arc<[SlackSearchTextHighlight]>,
    pub(crate) attachments: Arc<[SlackSearchAttachmentRow]>,
    pub(crate) reaction_state: Arc<[SlackReaction]>,
    pub(crate) reactions: Arc<[SlackReactionRow]>,
    pub(crate) reply_count: Option<u32>,
    pub(crate) show_thread_action: bool,
    pub(crate) latest_reply_label: Option<SharedString>,
    pub(crate) reply_avatar_image_urls: Arc<[SharedString]>,
    pub(crate) accessibility_label: SharedString,
    pub(crate) card_height: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSearchTextHighlight {
    pub(crate) range: Range<usize>,
    pub(crate) kind: SlackSearchTextHighlightKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackSearchTextHighlightKind {
    Link,
    QueryMatch,
    LinkAndQueryMatch,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackSearchAttachmentRow {
    pub(crate) attachment: SlackAttachmentRow,
    pub(crate) host: SlackMediaHostId,
    pub(crate) presentation: SlackSearchAttachmentPresentation,
    pub(crate) subtitle: SharedString,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SlackSearchAttachmentPresentation {
    Summary,
    Audio,
    Video { width: f32, height: f32 },
}

impl SlackSearchAttachmentPresentation {
    pub(crate) fn height(self) -> f32 {
        match self {
            Self::Summary => 61.0,
            Self::Audio => 78.0,
            Self::Video { height, .. } => height,
        }
    }
}

pub(crate) struct PreparedSlackSearchSnapshot {
    pub(crate) snapshot: SlackSearchSnapshot,
    pub(crate) rows: Arc<[SlackSearchRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackQuickSearchRow {
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) label: SharedString,
    pub(crate) detail: SharedString,
    pub(crate) kind: SlackQuickSearchRowKind,
    pub(crate) target: SlackQuickSearchTarget,
    pub(crate) avatar_initials: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackQuickSearchMessageRow {
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) author_label: SharedString,
    pub(crate) conversation_label: SharedString,
    pub(crate) conversation_kind: SlackConversationKind,
    pub(crate) is_thread: bool,
    pub(crate) timestamp_label: SharedString,
    pub(crate) excerpt: SharedString,
    pub(crate) highlights: Arc<[Range<usize>]>,
    pub(crate) action_target: Arc<SlackMessageActionTarget>,
    pub(crate) avatar_initials: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackQuickSearchRowKind {
    Channel,
    PrivateChannel,
    DirectMessage,
    GroupMessage,
    Person,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackQuickSearchTarget {
    Conversation(SharedString),
    Profile(SharedString),
}

pub(crate) struct PreparedSlackQuickSearchSnapshot {
    pub(crate) snapshot: SlackQuickSearchSnapshot,
    pub(crate) rows: Arc<[SlackQuickSearchRow]>,
    pub(crate) message_rows: Arc<[SlackQuickSearchMessageRow]>,
}

pub(crate) type SlackAuthoredClientMessageIds = HashSet<(SlackMessageClientId, String)>;

#[derive(Clone)]
pub(crate) struct PreparedSlackConversationSnapshot {
    pub(crate) snapshot: SlackConversationSnapshot,
    pub(crate) message_rows: Arc<[SlackMessageRow]>,
    pub(crate) message_rows_local_today: Date,
    pub(crate) message_chunks: Arc<[SlackMessageChunk]>,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
    pub(crate) authored_client_message_ids: Arc<SlackAuthoredClientMessageIds>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackConversationLiveTarget {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationReadOverlay {
    pub(crate) target: SlackConversationLiveTarget,
    pub(crate) read_through: SlackMessageTimestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackConversationReadReadiness {
    AwaitingVisibility { check_scheduled: bool },
    HeldUnread,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationReadState {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) target: SlackConversationLiveTarget,
    pub(crate) floor: Option<SlackMessageTimestamp>,
    pub(crate) readiness: SlackConversationReadReadiness,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationRefreshRequest {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) target: SlackConversationLiveTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationReadRequest {
    pub(crate) generation: u64,
    pub(crate) target: SlackConversationLiveTarget,
    pub(crate) message_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackConversationLoadRoute {
    #[default]
    Conversation,
    NewMessage,
}

impl SlackConversationLoadRoute {
    pub(crate) fn main_route(self) -> SlackMainRoute {
        match self {
            Self::Conversation => SlackMainRoute::Conversation,
            Self::NewMessage => SlackMainRoute::NewMessage,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationHistoryRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) cursor: SlackConversationHistoryCursor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSendRequest {
    pub(crate) generation: u64,
    pub(crate) draft_token: u64,
    pub(crate) client_message_id: SlackMessageClientId,
    pub(crate) team_id: String,
    pub(crate) self_user_id: String,
    pub(crate) conversation_id: String,
    pub(crate) draft_source: SlackSendDraftSource,
    pub(crate) draft_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackSendPayload {
    Message(SlackMessageDraft),
    Share(crate::model::SlackFileShareRequest<SlackMessageDraft>),
}

#[derive(Clone)]
pub(crate) struct SlackOutboundDelivery {
    pub(crate) request: SlackSendRequest,
    pub(crate) retry_payload: Option<Arc<SlackSendPayload>>,
    pub(crate) accepted_draft_handle: SlackMainComposerDraftHandle,
    pub(crate) accepted_draft: SlackComposerDraft,
    pub(crate) row: SlackMessageRow,
}

pub(crate) struct PreparedSlackConversationHistoryPage {
    pub(crate) snapshot: SlackConversationSnapshot,
    pub(crate) message_rows: Arc<[SlackMessageRow]>,
    pub(crate) message_rows_local_today: Date,
    pub(crate) message_chunks: Arc<[SlackMessageChunk]>,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
    pub(crate) prepended_row_count: usize,
}

pub(crate) struct PreparedSlackThreadSnapshot {
    pub(crate) snapshot: SlackThreadSnapshot,
    pub(crate) timezone: chrono_tz::Tz,
    pub(crate) parent_row: Option<SlackMessageRow>,
    pub(crate) reply_rows: Arc<[SlackMessageRow]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackThreadReadReadiness {
    AwaitingVisibility { check_scheduled: bool },
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackThreadReadIdentity {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) thread_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackThreadReadState {
    pub(crate) generation: u64,
    pub(crate) target: SlackThreadReadIdentity,
    pub(crate) floor: Option<SlackMessageTimestamp>,
    pub(crate) unread_count: u32,
    pub(crate) readiness: SlackThreadReadReadiness,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackThreadReadRequest {
    pub(crate) generation: u64,
    pub(crate) target: SlackThreadReadIdentity,
    pub(crate) message_timestamp: SlackMessageTimestamp,
}

pub(crate) struct PreparedSlackThreadReplyReceipt {
    pub(crate) receipt: SlackThreadReplyReceipt,
    pub(crate) reply_row: SlackMessageRow,
}

#[derive(Clone)]
pub(crate) struct PreparedSlackMessageSendReceipt {
    pub(crate) receipt: SlackMessageSendReceipt,
    pub(crate) previous_message_id: Option<String>,
    pub(crate) message_row: SlackMessageRow,
    pub(crate) message_row_local_today: Date,
    pub(crate) remote_images: HashMap<String, Arc<Image>>,
}
