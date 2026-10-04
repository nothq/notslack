mod action_target;

pub(crate) use action_target::SlackMessageActionTarget;

use super::{
    Arc, Date, Deref, Range, SharedString, SlackAttachmentLayoutRow, SlackAttachmentRow,
    SlackLaterState, SlackReaction, SlackReactionRow, SlackTableRow,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlackMessageBody {
    pub(crate) text: SharedString,
    pub(crate) links: Arc<[SlackMessageBodyLink]>,
    pub(crate) blocks: Arc<[SlackMessageBodyBlock]>,
}

impl Deref for SlackMessageBody {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.text.as_ref()
    }
}

impl SlackMessageBody {
    pub(crate) fn has_renderable_content(&self) -> bool {
        !self.blocks.is_empty()
    }

    pub(crate) fn remote_image_urls(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().flat_map(|block| {
            block
                .accessory_image
                .iter()
                .map(|image| image.image_url.as_ref())
                .chain(block.context_elements.iter().filter_map(|element| {
                    let SlackMessageBodyContextElement::Image(image) = element else {
                        return None;
                    };
                    Some(image.image_url.as_ref())
                }))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyLink {
    pub(crate) range: Range<usize>,
    pub(crate) target: SlackMessageBodyTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageBodyTarget {
    Url(SharedString),
    User(SharedString),
    Channel(SharedString),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageBodyBlockKind {
    Paragraph,
    ListItem { indent: u8 },
    Quote,
    Preformatted,
    BlockKitSection,
    BlockKitHeader,
    BlockKitContext,
    BlockKitDivider,
    BlockKitActions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlackMessageTextStyle {
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) strike: bool,
    pub(crate) code: bool,
    pub(crate) link: bool,
    pub(crate) mention: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyStyleRange {
    pub(crate) range: Range<usize>,
    pub(crate) style: SlackMessageTextStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageElementIds {
    pub(crate) conversation: SharedString,
    pub(crate) thread: SharedString,
    pub(crate) all_threads: SharedString,
    pub(crate) activity: SharedString,
    pub(crate) later: SharedString,
    pub(crate) pins: SharedString,
    pub(crate) search: SharedString,
}

impl SlackMessageElementIds {
    pub(crate) fn for_context(&self, context: SlackMessageRenderContext) -> SharedString {
        match context {
            SlackMessageRenderContext::Conversation => self.conversation.clone(),
            SlackMessageRenderContext::Thread => self.thread.clone(),
            SlackMessageRenderContext::AllThreads => self.all_threads.clone(),
            SlackMessageRenderContext::Activity => self.activity.clone(),
            SlackMessageRenderContext::Later => self.later.clone(),
            SlackMessageRenderContext::Pins => self.pins.clone(),
            SlackMessageRenderContext::Search => self.search.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyBlock {
    pub(crate) kind: SlackMessageBodyBlockKind,
    pub(crate) text: SharedString,
    pub(crate) styles: Arc<[SlackMessageBodyStyleRange]>,
    pub(crate) links: Arc<[SlackMessageBodyLink]>,
    pub(crate) link_ranges: Arc<[Range<usize>]>,
    pub(crate) code_ranges: Arc<[Range<usize>]>,
    pub(crate) code_font_family: SharedString,
    pub(crate) fields: Arc<[SlackMessageBodyText]>,
    pub(crate) accessory_image: Option<SlackMessageBodyImage>,
    pub(crate) context_elements: Arc<[SlackMessageBodyContextElement]>,
    pub(crate) actions: Arc<[SlackMessageBodyAction]>,
    pub(crate) element_ids: SlackMessageElementIds,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyText {
    pub(crate) text: SharedString,
    pub(crate) styles: Arc<[SlackMessageBodyStyleRange]>,
    pub(crate) links: Arc<[SlackMessageBodyLink]>,
    pub(crate) link_ranges: Arc<[Range<usize>]>,
    pub(crate) code_ranges: Arc<[Range<usize>]>,
    pub(crate) code_font_family: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyImage {
    pub(crate) image_url: SharedString,
    pub(crate) alt_text: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageBodyContextElement {
    Text(SlackMessageBodyText),
    Image(SlackMessageBodyImage),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageBodyAction {
    pub(crate) label: SharedString,
    pub(crate) kind: SlackMessageBodyActionKind,
    pub(crate) style: SlackMessageBodyActionStyle,
    pub(crate) url: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageBodyActionKind {
    Button,
    Select,
    Overflow,
    DatePicker,
    TimePicker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageBodyActionStyle {
    Default,
    Primary,
    Danger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageRenderContext {
    Conversation,
    Thread,
    AllThreads,
    Activity,
    Later,
    Pins,
    Search,
}

impl SlackMessageRenderContext {
    pub(crate) fn id_segment(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Thread => "thread",
            Self::AllThreads => "all-threads",
            Self::Activity => "activity",
            Self::Later => "later",
            Self::Pins => "pins",
            Self::Search => "search",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageDeliveryState {
    Pending {
        label: SharedString,
    },
    Failed {
        label: SharedString,
        detail: SharedString,
        actions: Arc<[SlackMessageDeliveryAction]>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageDeliveryActionKind {
    Retry,
    Restore,
    Dismiss,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageDeliveryAction {
    kind: SlackMessageDeliveryActionKind,
    element_id: SharedString,
    accessibility_label: SharedString,
    label: SharedString,
    client_message_id: crate::ui::SlackMessageClientId,
}

impl SlackMessageDeliveryAction {
    pub(crate) fn new(
        kind: SlackMessageDeliveryActionKind,
        client_message_id: crate::ui::SlackMessageClientId,
    ) -> Self {
        let (id, accessibility_label, label) = match kind {
            SlackMessageDeliveryActionKind::Retry => ("retry", "Retry sending message", "Retry"),
            SlackMessageDeliveryActionKind::Restore => (
                "restore",
                "Restore failed message to the composer",
                "Restore",
            ),
            SlackMessageDeliveryActionKind::Dismiss => {
                ("dismiss", "Dismiss failed message", "Dismiss")
            }
        };
        Self {
            kind,
            element_id: format!("slack-{id}-delivery-{}", client_message_id.as_str()).into(),
            accessibility_label: accessibility_label.into(),
            label: label.into(),
            client_message_id,
        }
    }

    pub(crate) fn kind(&self) -> SlackMessageDeliveryActionKind {
        self.kind
    }

    pub(crate) fn element_id(&self) -> SharedString {
        self.element_id.clone()
    }

    pub(crate) fn accessibility_label(&self) -> SharedString {
        self.accessibility_label.clone()
    }

    pub(crate) fn label(&self) -> SharedString {
        self.label.clone()
    }

    pub(crate) fn client_message_id(&self) -> &crate::ui::SlackMessageClientId {
        &self.client_message_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReplyParticipantRow {
    pub(crate) profile_element_id: SharedString,
    pub(crate) profile_accessibility_label: SharedString,
    pub(crate) user_id: SharedString,
    pub(crate) avatar_text: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReplySummaryRow {
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) count_label: SharedString,
    pub(crate) latest_reply_label: SharedString,
}

impl SlackReplySummaryRow {
    pub(crate) fn new(message_id: &str, reply_count: u32, latest_reply_timestamp: &str) -> Self {
        let reply_noun = if reply_count == 1 { "reply" } else { "replies" };
        Self {
            element_id: format!("slack-thread-summary-{message_id}").into(),
            accessibility_label: format!("Open thread with {reply_count} {reply_noun}").into(),
            count_label: format!("{reply_count} {reply_noun}").into(),
            latest_reply_label: format!("Last reply {latest_reply_timestamp}").into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageDividerRow {
    pub(crate) local_date: Option<Date>,
    pub(crate) label: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageRow {
    pub(crate) id: String,
    pub(crate) author: String,
    pub(crate) timestamp: String,
    pub(crate) compact: bool,
    pub(crate) action_target: Option<Arc<SlackMessageActionTarget>>,
    pub(crate) delivery: Option<SlackMessageDeliveryState>,
    pub(crate) saved_state: Option<SlackLaterState>,
    pub(crate) user_id: Option<String>,
    pub(crate) avatar_text: String,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<String>,
    pub(crate) body: SlackMessageBody,
    pub(crate) edit_round_trip_supported: bool,
    pub(crate) table_rows: Vec<SlackTableRow>,
    pub(crate) edited_label: Option<SharedString>,
    pub(crate) local_date: Option<Date>,
    pub(crate) divider: Option<SlackMessageDividerRow>,
    pub(crate) unread_boundary_before: bool,
    pub(crate) date_label: Option<SharedString>,
    pub(crate) attachments: Vec<SlackAttachmentRow>,
    pub(crate) attachment_layout: Arc<[SlackAttachmentLayoutRow]>,
    pub(crate) reaction_state: Arc<[SlackReaction]>,
    pub(crate) reactions: Vec<SlackReactionRow>,
    pub(crate) reply_count: Option<u32>,
    pub(crate) latest_reply_timestamp: Option<String>,
    pub(crate) reply_summary: Option<SlackReplySummaryRow>,
    pub(crate) reply_participant_user_ids: Arc<[SharedString]>,
    pub(crate) reply_participants: Arc<[SlackReplyParticipantRow]>,
    pub(crate) latest_reply_author: Option<String>,
    pub(crate) latest_reply_user_id: Option<String>,
    pub(crate) latest_reply_avatar_text: Option<String>,
    pub(crate) latest_reply_avatar_fill: u32,
    pub(crate) latest_reply_avatar_image_url: Option<String>,
    pub(crate) replies: Vec<SlackMessageRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageChunk {
    pub(crate) row_range: Range<usize>,
}
