mod row;
mod snapshot;

pub(crate) use snapshot::{
    merge_and_prepare_slack_activity_snapshot, mutate_and_prepare_slack_activity_snapshot,
    prepare_slack_activity_workspace_context, reconcile_and_prepare_slack_activity_snapshot,
    rollback_and_prepare_slack_activity_snapshot,
};

use row::SlackActivityChannel;

use std::sync::Arc;

use crate::model::{
    SlackActivityArchiveTarget, SlackActivityReadTarget, SlackActivitySnapshot,
    SlackMessageTimestamp,
};
use gpui::SharedString;

use super::{SlackActiveMainComposerContext, SlackMessageBody, SlackMessageRow};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackActivityFilter {
    #[default]
    All,
    Dms,
    Mentions,
    Threads,
}

impl SlackActivityFilter {
    pub(crate) const ALL: [Self; 4] = [Self::All, Self::Dms, Self::Mentions, Self::Threads];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Dms => "DMs",
            Self::Mentions => "Mentions",
            Self::Threads => "Threads",
        }
    }

    pub(crate) fn includes(self, kind: SlackActivityRowKind) -> bool {
        match self {
            Self::All => true,
            Self::Dms => kind.is_dm(),
            Self::Mentions => kind == SlackActivityRowKind::Mention,
            Self::Threads => kind == SlackActivityRowKind::Thread,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackActivityRowKind {
    Reaction,
    Thread,
    Mention,
    Dm,
    BotDm,
}

impl SlackActivityRowKind {
    pub(crate) fn is_dm(self) -> bool {
        matches!(self, Self::Dm | Self::BotDm)
    }

    pub(crate) fn context_prefix(self) -> &'static str {
        match self {
            Self::Reaction => "Reacted in",
            Self::Thread => "Thread in",
            Self::Mention => "Mention in",
            Self::Dm => "DM",
            Self::BotDm => "Bot DM",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackActivityRow {
    pub(crate) key: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) actions_id: SharedString,
    pub(crate) actions_hover_group: SharedString,
    pub(crate) read_action_id: SharedString,
    pub(crate) archive_action_id: SharedString,
    pub(crate) read_target: Option<SlackActivityReadTarget>,
    pub(crate) archive_target: Option<SlackActivityArchiveTarget>,
    pub(crate) kind: SlackActivityRowKind,
    pub(crate) channel_id: SharedString,
    pub(crate) message_timestamp: SlackMessageTimestamp,
    pub(crate) thread_timestamp: Option<SlackMessageTimestamp>,
    pub(crate) channel_label: Option<SharedString>,
    pub(crate) actor_label: SharedString,
    pub(crate) avatar_text: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) timestamp_label: SharedString,
    pub(crate) body: SlackMessageBody,
    pub(crate) reaction_label: Option<SharedString>,
    pub(crate) divider_label: Option<SharedString>,
    pub(crate) archived: bool,
    pub(crate) unread: bool,
    pub(crate) card_height: f32,
    pub(crate) accessibility_label: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackActivityMutationContext {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackActivityItemMutation {
    MarkRead(SlackActivityReadTarget),
    MarkUnread(SlackActivityReadTarget),
    Archive {
        target: SlackActivityArchiveTarget,
        reason: &'static str,
    },
    Unarchive {
        target: SlackActivityArchiveTarget,
        reason: &'static str,
    },
}

impl SlackActivityItemMutation {
    pub(crate) fn key(&self) -> &str {
        match self {
            Self::MarkRead(target) | Self::MarkUnread(target) => target.key(),
            Self::Archive { target, .. } | Self::Unarchive { target, .. } => target.key(),
        }
    }

    pub(crate) fn unread(&self) -> Option<bool> {
        match self {
            Self::MarkRead(_) => Some(false),
            Self::MarkUnread(_) => Some(true),
            Self::Archive { .. } | Self::Unarchive { .. } => None,
        }
    }

    pub(crate) fn archived(&self) -> Option<bool> {
        match self {
            Self::Archive { .. } => Some(true),
            Self::Unarchive { .. } => Some(false),
            Self::MarkRead(_) | Self::MarkUnread(_) => None,
        }
    }

    pub(crate) fn failure_message(&self) -> &'static str {
        match self {
            Self::MarkRead(_) => "Couldn’t mark this Activity item as read.",
            Self::MarkUnread(_) => "Couldn’t mark this Activity item as unread.",
            Self::Archive { .. } => "Couldn’t clear this Activity item.",
            Self::Unarchive { .. } => "Couldn’t restore this Activity item.",
        }
    }

    pub(crate) fn success_description(&self) -> &'static str {
        match self {
            Self::MarkRead(_) => "marking read",
            Self::MarkUnread(_) => "marking unread",
            Self::Archive { .. } => "clearing",
            Self::Unarchive { .. } => "restoring",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackActivityItemMutationRollback {
    pub(crate) unread: bool,
    pub(crate) archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackActivityQueuedMutation {
    pub(crate) mutation: SlackActivityItemMutation,
    pub(crate) rollback: SlackActivityItemMutationRollback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackActivityItemMutationRequest {
    pub(crate) request_id: u64,
    pub(crate) context: SlackActivityMutationContext,
    pub(crate) mutation: SlackActivityItemMutation,
    pub(crate) rollback: SlackActivityItemMutationRollback,
}

pub(crate) struct PreparedSlackActivitySnapshot {
    pub(crate) snapshot: SlackActivitySnapshot,
    pub(crate) rows: Arc<[SlackActivityRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackActivityDetailTarget {
    Conversation {
        composer: Box<SlackActiveMainComposerContext>,
    },
    Thread {
        team_id: String,
        self_user_id: String,
        conversation_id: String,
        item_key: SharedString,
        thread_timestamp: SlackMessageTimestamp,
    },
}

impl SlackActivityDetailTarget {
    pub(crate) fn composer(&self) -> Option<&SlackActiveMainComposerContext> {
        match self {
            Self::Conversation { composer } => Some(composer.as_ref()),
            Self::Thread { .. } => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SlackActivityWorkspaceContext {
    channels: Arc<[SlackActivityChannel]>,
}

#[derive(Clone, Default)]
pub(crate) enum SlackActivityDetailState {
    #[default]
    Empty,
    Loading {
        key: SharedString,
        channel_label: Option<SharedString>,
    },
    Loaded {
        key: SharedString,
        channel_label: SharedString,
        message_timestamp: SlackMessageTimestamp,
        target: SlackActivityDetailTarget,
        rows: Arc<[SlackMessageRow]>,
    },
    Error {
        key: SharedString,
        channel_label: Option<SharedString>,
        message: SharedString,
    },
}

impl SlackActivityDetailState {
    pub(crate) fn key(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Loading { key, .. } | Self::Loaded { key, .. } | Self::Error { key, .. } => {
                Some(key.as_ref())
            }
        }
    }

    pub(crate) fn channel_label(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Loading { channel_label, .. } | Self::Error { channel_label, .. } => {
                channel_label.as_deref()
            }
            Self::Loaded { channel_label, .. } => Some(channel_label.as_ref()),
        }
    }

    pub(crate) fn composer(&self) -> Option<&SlackActiveMainComposerContext> {
        let Self::Loaded { target, .. } = self else {
            return None;
        };
        target.composer()
    }
}
