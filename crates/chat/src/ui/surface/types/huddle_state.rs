use gpui::SharedString;

use crate::ui::SlackHuddleJoinReceipt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackHuddleStatus {
    /// Waiting for Slack's `rooms.join`.
    Joining,
    /// Connecting the audio.
    Connecting,
    Connected,
    Leaving,
}

/// The huddle notslack is in, if any.
pub(crate) struct SlackHuddleState {
    pub(crate) generation: u64,
    pub(crate) conversation_id: String,
    pub(crate) label: SharedString,
    pub(crate) status: SlackHuddleStatus,
    pub(crate) muted: bool,
    pub(crate) participants: usize,
    pub(crate) receipt: Option<SlackHuddleJoinReceipt>,
    pub(crate) session: Option<::huddle::HuddleSession>,
}
