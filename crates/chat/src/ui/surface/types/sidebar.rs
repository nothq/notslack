use super::{SharedString, SlackSidebarItem, SlackSidebarSectionIndicator};

#[derive(Clone)]
pub(crate) struct SlackSidebarRow {
    pub(crate) kind: SlackSidebarRowKind,
    pub(crate) boundary_state: SlackSidebarRowBoundaryState,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SlackSidebarRowBoundaryState {
    pub(crate) last_target_at_or_before: Option<SlackSidebarBoundaryTarget>,
    pub(crate) next_target_at_or_after: Option<SlackSidebarBoundaryTarget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackSidebarBoundaryDirection {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackSidebarBoundaryLabel {
    UnreadMentions,
    MoreUnreads,
}

impl SlackSidebarBoundaryLabel {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::UnreadMentions => "Unread mentions",
            Self::MoreUnreads => "More unreads",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackSidebarBoundaryTarget {
    pub(crate) row_index: usize,
    pub(crate) direction: SlackSidebarBoundaryDirection,
    pub(crate) label: SlackSidebarBoundaryLabel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSidebarRevealState {
    pub(crate) conversation_id: String,
    pub(crate) scheduled: bool,
    pub(crate) awaiting_live_sidebar: bool,
}

impl SlackSidebarRevealState {
    pub(crate) fn pending(conversation_id: String, awaiting_live_sidebar: bool) -> Self {
        Self {
            conversation_id,
            scheduled: false,
            awaiting_live_sidebar,
        }
    }
}

#[derive(Clone)]
pub(crate) enum SlackSidebarRowKind {
    Shortcut {
        label: String,
        badge: Option<String>,
    },
    SectionHeader {
        label: String,
        indicator: SlackSidebarSectionIndicator,
        collapsible: bool,
    },
    Separator,
    Spacer {
        height: f32,
    },
    DropHint,
    Item {
        item: Box<SlackSidebarItem>,
        label: SharedString,
        secondary_context: Option<SharedString>,
        finder_element_id: SharedString,
        finder_accessibility_label: SharedString,
        finder_search_key: SharedString,
        finder_group_count_label: Option<SharedString>,
    },
}
