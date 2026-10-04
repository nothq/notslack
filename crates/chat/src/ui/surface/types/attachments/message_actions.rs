use super::{
    Arc, SharedString, SlackMessageActionTarget, SlackMessageElementIds, SlackMessageTimestamp,
    SlackReaction, SlackReactionMutation, SlackReactionName, SlackSavedMessageMutation,
    SlackShellIcon,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum SlackReactionOwnership {
    CurrentUser,
    OtherUsers,
}

impl SlackReactionOwnership {
    pub(crate) fn is_current_user(self) -> bool {
        self == Self::CurrentUser
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionVariantRow {
    pub(crate) display: SharedString,
    pub(crate) image_cache_key: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionRow {
    pub(crate) message_id: SharedString,
    pub(crate) mutation_name: SharedString,
    pub(crate) count: u32,
    pub(crate) count_label: SharedString,
    pub(crate) ownership: SlackReactionOwnership,
    pub(crate) variants: Arc<[SlackReactionVariantRow]>,
    pub(crate) element_ids: SlackMessageElementIds,
    pub(crate) accessibility_label: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionRequest {
    pub(crate) generation: u64,
    pub(crate) target: Arc<SlackMessageActionTarget>,
    pub(crate) reaction_name: SlackReactionName,
    pub(crate) mutation: SlackReactionMutation,
    pub(crate) original_reactions: Arc<[SlackReaction]>,
    pub(crate) optimistic_reactions: Arc<[SlackReaction]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackMessageMenuAction {
    Edit,
    MarkUnread,
    CopyLink,
    CopyMessage,
    Delete,
}

impl SlackMessageMenuAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Edit => "Edit",
            Self::MarkUnread => "Mark unread",
            Self::CopyLink => "Copy link",
            Self::CopyMessage => "Copy message",
            Self::Delete => "Delete message…",
        }
    }

    pub(crate) fn shortcut(self) -> &'static str {
        match self {
            Self::Edit | Self::Delete => "",
            Self::MarkUnread => "U",
            Self::CopyLink => "L",
            Self::CopyMessage => "⌘C",
        }
    }

    pub(crate) fn element_id(self) -> &'static str {
        match self {
            Self::Edit => "slack-message-menu-edit",
            Self::MarkUnread => "slack-message-menu-mark-unread",
            Self::CopyLink => "slack-message-menu-copy-link",
            Self::CopyMessage => "slack-message-menu-copy-message",
            Self::Delete => "slack-message-menu-delete",
        }
    }

    pub(crate) fn icon(self) -> Option<SlackShellIcon> {
        match self {
            Self::Edit => Some(SlackShellIcon::MessageMenuEdit),
            Self::MarkUnread => Some(SlackShellIcon::MessageMenuMarkUnread),
            Self::CopyLink => Some(SlackShellIcon::MessageMenuCopyLink),
            Self::CopyMessage => Some(SlackShellIcon::MessageMenuCopyMessage),
            Self::Delete => None,
        }
    }

    pub(crate) fn group(self) -> u8 {
        match self {
            Self::Edit => 0,
            Self::MarkUnread => 1,
            Self::CopyLink | Self::CopyMessage => 2,
            Self::Delete => 3,
        }
    }

    pub(crate) fn danger(self) -> bool {
        self == Self::Delete
    }
}

#[derive(Clone)]
pub(crate) struct SlackMessageMenuOpenContext {
    pub(crate) action_target: Arc<SlackMessageActionTarget>,
    pub(crate) body: SharedString,
    pub(crate) user_id: Option<String>,
}

impl SlackMessageMenuOpenContext {
    pub(crate) fn new(
        action_target: Arc<SlackMessageActionTarget>,
        body: SharedString,
        user_id: Option<String>,
    ) -> Self {
        Self {
            action_target,
            body,
            user_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackMessageMenuState {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
    pub(crate) body: SharedString,
    pub(crate) mark_unread_cursor: Option<SlackMessageTimestamp>,
    pub(crate) actions: Arc<[SlackMessageMenuAction]>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) top: f32,
    pub(crate) right: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageSavedRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
    pub(crate) mutation: SlackSavedMessageMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessagePermalinkRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) message_timestamp: SlackMessageTimestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageMarkUnreadRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) target_timestamp: SlackMessageTimestamp,
    pub(crate) cursor: SlackMessageTimestamp,
}
