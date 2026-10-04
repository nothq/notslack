use super::SlackShellIcon;
use crate::ui::{SlackConversationTab, SlackDraftsSentTab};
use gpui::SharedString;

#[derive(Clone, Copy)]
pub(crate) struct SlackRailItemSpec {
    pub(crate) label: &'static str,
    pub(crate) icon: SlackShellIcon,
}

impl SlackRailItemSpec {
    pub(crate) const fn new(label: &'static str, icon: SlackShellIcon) -> Self {
        Self { label, icon }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackSidebarSectionIndicator {
    Chevron,
    Star,
    ExternalConnections,
    Channels,
    DirectMessages,
    Apps,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SlackRailView {
    #[default]
    Home,
    Dms,
    Activity,
    Files,
    Later,
    DraftsSent,
    More,
    Admin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackRailMenu {
    Workspace,
    More,
    Admin,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackMoreView {
    #[default]
    Tools,
    CustomizeNavigation,
}

impl SlackRailView {
    pub(crate) fn from_label(label: &str) -> Option<Self> {
        match label {
            "Home" => Some(Self::Home),
            "DMs" => Some(Self::Dms),
            "Activity" => Some(Self::Activity),
            "Files" => Some(Self::Files),
            "Later" => Some(Self::Later),
            "Drafts & sent" => Some(Self::DraftsSent),
            "More" => Some(Self::More),
            "Admin" => Some(Self::Admin),
            _ => None,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Dms => "Direct messages",
            Self::Activity => "Activity",
            Self::Files => "Files",
            Self::Later => "Later",
            Self::DraftsSent => "Drafts & sent",
            Self::More => "More",
            Self::Admin => "Admin",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackMainRoute {
    #[default]
    Conversation,
    AllThreads,
    Directory,
    NewMessage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackHeaderControl {
    #[default]
    ChannelMove,
    ChannelDetails,
    Members,
    Notifications,
    Search,
    More,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackHistoryDestination {
    Conversation {
        conversation_id: SharedString,
        rail_view: SlackRailView,
        tab: SlackMainTab,
        source_tab: Option<SlackConversationTab>,
    },
    Rail(SlackRailView),
    DraftsSent(SlackDraftsSentTab),
    AllThreads,
    Directory,
    NewMessage,
    Search {
        query: SharedString,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackHistoryEntry {
    pub(crate) destination: SlackHistoryDestination,
    pub(crate) label: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) icon: SlackShellIcon,
    pub(crate) avatar_image_url: Option<SharedString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackSearchUnreadDraftPosition {
    pub(crate) section_index: usize,
    pub(crate) item_index: usize,
    pub(crate) has_draft: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackMainTab {
    #[default]
    Messages,
    Canvas,
    BookmarkFolder,
    FilesLinks,
    Pins,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackPlaybackSpeed {
    #[default]
    OneX,
    OnePointFiveX,
    TwoX,
}

#[cfg(test)]
impl SlackPlaybackSpeed {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::OneX => Self::OnePointFiveX,
            Self::OnePointFiveX => Self::TwoX,
            Self::TwoX => Self::OneX,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::OneX => "1x",
            Self::OnePointFiveX => "1.5x",
            Self::TwoX => "2x",
        }
    }

    pub(crate) fn step_millis(self) -> u32 {
        match self {
            Self::OneX => 1_000,
            Self::OnePointFiveX => 1_500,
            Self::TwoX => 2_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackFilesFilter {
    #[default]
    All,
    Recordings,
    Links,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlackMediaState {
    pub playing: bool,
    pub muted: bool,
    pub captions_enabled: bool,
    pub transcript_visible: bool,
    pub transcript_generated: bool,
    pub playback_speed: SlackPlaybackSpeed,
    pub progress_millis: u32,
}
