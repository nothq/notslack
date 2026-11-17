use std::collections::HashMap;
use std::sync::Arc;

use gpui::SharedString;

use super::SlackMessageBody;
use crate::ui::SlackAppDmDetails;

/// Which app tab is showing in an app DM. `Messages` is the regular
/// conversation; non-app conversations always stay on `Messages`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackAppDmTab {
    #[default]
    Messages,
    Home,
    About,
}

/// App tabs shown in the conversation header for an app DM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackAppDmTabSet {
    pub(crate) home: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackAppDmLoad<T> {
    Loading,
    Loaded(T),
    Failed(SharedString),
}

impl<T> SlackAppDmLoad<T> {
    pub(crate) fn loaded(&self) -> Option<&T> {
        match self {
            Self::Loaded(value) => Some(value),
            Self::Loading | Self::Failed(_) => None,
        }
    }
}

/// Prepared App Home blocks; `None` when the view has no renderable content.
pub(crate) type PreparedSlackAppHomeView = Option<Arc<SlackMessageBody>>;

/// (team id, conversation id)
pub(crate) type SlackAppDmConversationKey = (String, String);
/// (team id, view id)
pub(crate) type SlackAppHomeViewKey = (String, String);

#[derive(Debug, Default)]
pub(crate) struct SlackAppDmState {
    pub(crate) conversation: Option<SlackAppDmConversationKey>,
    pub(crate) active_tab: SlackAppDmTab,
    pub(crate) details: HashMap<SlackAppDmConversationKey, SlackAppDmLoad<Arc<SlackAppDmDetails>>>,
    pub(crate) home_views: HashMap<SlackAppHomeViewKey, SlackAppDmLoad<PreparedSlackAppHomeView>>,
}

impl SlackAppDmState {
    pub(crate) fn current_details(&self) -> Option<&SlackAppDmLoad<Arc<SlackAppDmDetails>>> {
        self.details.get(self.conversation.as_ref()?)
    }
}
