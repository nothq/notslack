use std::sync::Arc;

use gpui::SharedString;

use super::{build_slack_pinned_message_row, SlackMessageActionTarget, SlackMessageRow};
use crate::ui::{SlackPinnedItem, SlackPinsRequest, SlackPinsSnapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackPinRow {
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) message: SlackMessageRow,
}

pub(crate) struct PreparedSlackPinsSnapshot {
    pub(crate) snapshot: SlackPinsSnapshot,
    pub(crate) rows: Arc<[SlackPinRow]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackPinsLoadRequest {
    pub(crate) generation: u64,
    pub(crate) request: SlackPinsRequest,
}

pub(crate) fn prepare_slack_pins_snapshot(
    snapshot: SlackPinsSnapshot,
) -> PreparedSlackPinsSnapshot {
    let rows = snapshot
        .items
        .iter()
        .map(|item| {
            let conversation_id = match item {
                SlackPinnedItem::Message(pin) => pin.conversation_id.as_str(),
                SlackPinnedItem::File(pin) => pin.conversation_id.as_str(),
                SlackPinnedItem::FileComment(pin) => pin.conversation_id.as_str(),
            };
            let mut message =
                build_slack_pinned_message_row(item.message(), &snapshot.team_id, conversation_id);
            if let SlackPinnedItem::Message(pin) = item {
                message.action_target = pin.thread_timestamp.as_deref().map_or_else(
                    || {
                        SlackMessageActionTarget::conversation_message(
                            &snapshot.team_id,
                            &pin.conversation_id,
                            &pin.message.id,
                        )
                    },
                    |thread_timestamp| {
                        SlackMessageActionTarget::thread_reply(
                            &snapshot.team_id,
                            &pin.conversation_id,
                            thread_timestamp,
                            &pin.message.id,
                        )
                    },
                );
            }
            SlackPinRow {
                element_id: format!("slack-pin-{}", item.id()).into(),
                accessibility_label: format!(
                    "{} by {} at {}",
                    item.accessibility_kind_label(),
                    item.message().author,
                    item.message().timestamp
                )
                .into(),
                message,
            }
        })
        .collect::<Vec<_>>()
        .into();
    PreparedSlackPinsSnapshot { snapshot, rows }
}
