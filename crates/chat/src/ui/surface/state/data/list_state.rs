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
