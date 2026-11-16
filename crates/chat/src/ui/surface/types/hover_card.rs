use gpui::{Pixels, Point};

/// A profile card for the person under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SlackHoverCard {
    pub(crate) generation: u64,
    pub(crate) user_id: String,
    /// Where the pointer was, so the card opens next to it.
    pub(crate) position: Point<Pixels>,
    /// Shown once the pointer has rested there briefly, as in Slack.
    pub(crate) visible: bool,
}
