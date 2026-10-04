use gpui::SharedString;

use super::SlackMessageRow;
use crate::ui::{
    SlackAttachment, SlackConversationKind, SlackLaterHydrationTarget, SlackLaterItemKey,
    SlackLaterState, SlackReminderClientId, SlackReminderId,
};

mod queue;
mod rows;

pub(crate) use queue::SlackLaterHydrationQueue;
pub(crate) use rows::{
    build_slack_later_rows, mark_slack_later_hydration_error, prepare_slack_later_hydrated_row,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackLaterThreadTarget {
    pub(crate) item_key: SlackLaterItemKey,
    pub(crate) conversation_id: String,
    pub(crate) conversation_kind: SlackConversationKind,
    pub(crate) conversation_name: SharedString,
    pub(crate) thread_timestamp: String,
    pub(crate) selected_message_id: SharedString,
    pub(crate) selected_message_row: SlackMessageRow,
    pub(crate) expected_reply_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackLaterRowContent {
    Placeholder { target: SlackLaterHydrationTarget },
    HydrationError { target: SlackLaterHydrationTarget },
    Message { detail: Box<SlackLaterThreadTarget> },
    File { attachment: Box<SlackAttachment> },
    Reminder { detail: SlackLaterReminderDetail },
    Tombstone,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackLaterReminderDetail {
    pub(crate) reminder_id: SlackReminderId,
    pub(crate) description: SharedString,
    pub(crate) due_at: u64,
    pub(crate) state: SlackLaterState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackLaterReminderDialogMode {
    Create { client_id: SlackReminderClientId },
    Edit { reminder_id: SlackReminderId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackLaterReminderDialog {
    pub(crate) mode: SlackLaterReminderDialogMode,
    pub(crate) description: String,
    pub(crate) due_at: u64,
    pub(crate) saving: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackLaterRow {
    pub(crate) key: SlackLaterItemKey,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) type_label: Option<SharedString>,
    pub(crate) title: SharedString,
    pub(crate) preview: Option<SharedString>,
    pub(crate) avatar_label: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) height: f32,
    pub(crate) content: SlackLaterRowContent,
}

impl SlackLaterRow {
    pub(crate) fn pending_hydration_target(&self) -> Option<&SlackLaterHydrationTarget> {
        match &self.content {
            SlackLaterRowContent::Placeholder { target } => Some(target),
            SlackLaterRowContent::HydrationError { .. }
            | SlackLaterRowContent::Message { .. }
            | SlackLaterRowContent::File { .. }
            | SlackLaterRowContent::Reminder { .. }
            | SlackLaterRowContent::Tombstone
            | SlackLaterRowContent::Unsupported => None,
        }
    }

    pub(crate) fn retry_hydration_target(&self) -> Option<&SlackLaterHydrationTarget> {
        match &self.content {
            SlackLaterRowContent::Placeholder { target }
            | SlackLaterRowContent::HydrationError { target } => Some(target),
            SlackLaterRowContent::Message { .. }
            | SlackLaterRowContent::File { .. }
            | SlackLaterRowContent::Reminder { .. }
            | SlackLaterRowContent::Tombstone
            | SlackLaterRowContent::Unsupported => None,
        }
    }

    pub(crate) fn thread_target(&self) -> Option<&SlackLaterThreadTarget> {
        match &self.content {
            SlackLaterRowContent::Message { detail } => Some(detail),
            SlackLaterRowContent::Placeholder { .. }
            | SlackLaterRowContent::HydrationError { .. }
            | SlackLaterRowContent::File { .. }
            | SlackLaterRowContent::Reminder { .. }
            | SlackLaterRowContent::Tombstone
            | SlackLaterRowContent::Unsupported => None,
        }
    }

    pub(crate) fn reminder_detail(&self) -> Option<&SlackLaterReminderDetail> {
        match &self.content {
            SlackLaterRowContent::Reminder { detail } => Some(detail),
            _ => None,
        }
    }
}
