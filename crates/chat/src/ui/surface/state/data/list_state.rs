use std::ops::{Deref, DerefMut};

use super::super::super::{px, ListAlignment, ListState, SLACK_MESSAGE_LIST_OVERDRAW};

pub(crate) struct SlackListState(ListState);

impl Default for SlackListState {
    fn default() -> Self {
        Self(ListState::new(
            0,
            ListAlignment::Top,
            px(SLACK_MESSAGE_LIST_OVERDRAW),
        ))
    }
}

impl SlackListState {
    /// Whether the list is scrolled to within `tolerance` of its end, or
    /// can't scroll at all. GPUI resumes following the tail only within a
    /// pixel of the end, which a trackpad's last nudge or an image settling
    /// its height easily misses.
    pub(crate) fn is_near_end(&self, tolerance: gpui::Pixels) -> bool {
        slack_scroll_near_end(
            self.0.max_offset_for_scrollbar().y,
            -self.0.scroll_px_offset_for_scrollbar().y,
            tolerance,
        )
    }
}

fn slack_scroll_near_end(
    max_offset: gpui::Pixels,
    offset: gpui::Pixels,
    tolerance: gpui::Pixels,
) -> bool {
    max_offset <= px(0.0) || max_offset - offset <= tolerance
}

impl From<ListState> for SlackListState {
    fn from(state: ListState) -> Self {
        Self(state)
    }
}

impl Deref for SlackListState {
    type Target = ListState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SlackListState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{px, slack_scroll_near_end};

    #[test]
    fn a_list_counts_as_at_the_end_within_the_tolerance() {
        assert!(slack_scroll_near_end(px(1000.0), px(1000.0), px(48.0)));
        assert!(slack_scroll_near_end(px(1000.0), px(970.0), px(48.0)));
        assert!(!slack_scroll_near_end(px(1000.0), px(800.0), px(48.0)));
        assert!(slack_scroll_near_end(px(0.0), px(0.0), px(48.0)));
    }
}
