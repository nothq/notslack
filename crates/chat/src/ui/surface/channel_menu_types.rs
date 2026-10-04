use crate::ui::SlackStarMutation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackChannelMenuAction {
    ChannelDetails,
    Copy,
    ToggleStar(SlackStarMutation),
}

impl SlackChannelMenuAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ChannelDetails => "Channel details",
            Self::Copy => "Copy",
            Self::ToggleStar(SlackStarMutation::Add) => "Add to Starred",
            Self::ToggleStar(SlackStarMutation::Remove) => "Remove from Starred",
        }
    }

    pub(crate) fn element_id(self) -> &'static str {
        match self {
            Self::ChannelDetails => "slack-channel-menu-details",
            Self::Copy => "slack-channel-menu-copy",
            Self::ToggleStar(_) => "slack-channel-menu-star",
        }
    }

    pub(crate) fn submenu(self) -> Option<SlackChannelMenuSubmenu> {
        match self {
            Self::ChannelDetails => Some(SlackChannelMenuSubmenu::ChannelDetails),
            Self::Copy => Some(SlackChannelMenuSubmenu::Copy),
            Self::ToggleStar(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackChannelMenuSubmenu {
    ChannelDetails,
    Copy,
}

impl SlackChannelMenuSubmenu {
    pub(crate) fn root_index(self) -> usize {
        match self {
            Self::ChannelDetails => 0,
            Self::Copy => 1,
        }
    }

    pub(crate) fn width(self) -> f32 {
        match self {
            Self::ChannelDetails => 300.0,
            Self::Copy => 230.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackChannelSubmenuAction {
    OpenChannelDetails,
    SearchInChannel,
    CopyName,
    CopyLink,
}

impl SlackChannelSubmenuAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::OpenChannelDetails => "Open channel details",
            Self::SearchInChannel => "Search in channel",
            Self::CopyName => "Copy name",
            Self::CopyLink => "Copy link",
        }
    }

    pub(crate) fn element_id(self) -> &'static str {
        match self {
            Self::OpenChannelDetails => "slack-channel-menu-open-details",
            Self::SearchInChannel => "slack-channel-menu-search",
            Self::CopyName => "slack-channel-menu-copy-name",
            Self::CopyLink => "slack-channel-menu-copy-link",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelMenuItems<T: Copy, const N: usize> {
    entries: [Option<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> SlackChannelMenuItems<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            entries: [None; N],
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, item: T) {
        assert!(self.len < N, "Slack channel menu item capacity exceeded");
        self.entries[self.len] = Some(item);
        self.len += 1;
    }

    pub(crate) fn len(self) -> usize {
        self.len
    }

    pub(crate) fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) fn get(self, index: usize) -> Option<T> {
        self.entries.get(index).copied().flatten()
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = T> {
        self.entries.into_iter().take(self.len).flatten()
    }
}

pub(crate) type SlackChannelMenuActions = SlackChannelMenuItems<SlackChannelMenuAction, 3>;
pub(crate) type SlackChannelSubmenuActions = SlackChannelMenuItems<SlackChannelSubmenuAction, 4>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelStarRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) mutation: SlackStarMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelPermalinkRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
}
