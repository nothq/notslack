use std::sync::Arc;

use gpui::{Bounds, Font, Pixels, SharedString};

use super::SlackShellIcon;
use crate::ui::{SlackConversationTab, SlackConversationTabTarget, SlackWorkspaceApiCapabilities};

pub(crate) const SLACK_CONVERSATION_TAB_MESSAGES_KEY: &str = "messages";
pub(crate) const SLACK_CONVERSATION_TAB_MORE_KEY: &str = "more";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlackConversationTabCapabilities {
    folder: bool,
    files: bool,
    pins: bool,
}

impl From<SlackWorkspaceApiCapabilities> for SlackConversationTabCapabilities {
    fn from(capabilities: SlackWorkspaceApiCapabilities) -> Self {
        Self {
            folder: capabilities.load_bookmark_folder,
            files: capabilities.load_conversation_files,
            pins: capabilities.load_pins,
        }
    }
}

impl SlackConversationTabCapabilities {
    pub(crate) fn supports(&self, target: &SlackConversationTabTarget) -> bool {
        match target {
            SlackConversationTabTarget::Canvas { .. } => true,
            SlackConversationTabTarget::Folder { .. } => self.folder,
            SlackConversationTabTarget::Files => self.files,
            SlackConversationTabTarget::Pins => self.pins,
            SlackConversationTabTarget::Unsupported { .. } => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackConversationTabAction {
    Messages,
    AddCanvas,
    CanvasLink {
        tab: SlackConversationTab,
        permalink: SharedString,
    },
    BookmarkFolder(SlackConversationTab),
    Files,
    Pins,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreparedSlackConversationTab {
    pub(crate) key: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) overflow_element_id: SharedString,
    pub(crate) label: SharedString,
    pub(crate) icon: SlackShellIcon,
    pub(crate) action: Arc<SlackConversationTabAction>,
    pub(crate) enabled: bool,
    pub(crate) width: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackConversationTabsCache {
    pub(crate) source_tabs: Arc<[SlackConversationTab]>,
    pub(crate) capabilities: SlackConversationTabCapabilities,
    pub(crate) font: Option<Font>,
    pub(crate) scale_factor_bits: u32,
    pub(crate) bounds: Option<Bounds<Pixels>>,
    pub(crate) tabs: Arc<[PreparedSlackConversationTab]>,
    pub(crate) visible_count: usize,
    pub(crate) more_width: f32,
}

impl Default for SlackConversationTabsCache {
    fn default() -> Self {
        Self {
            source_tabs: Arc::default(),
            capabilities: SlackConversationTabCapabilities::default(),
            font: None,
            scale_factor_bits: 0,
            bounds: None,
            tabs: Arc::default(),
            visible_count: 0,
            more_width: 0.0,
        }
    }
}

impl SlackConversationTabsCache {
    pub(crate) fn source_matches(
        &self,
        source_tabs: &[SlackConversationTab],
        capabilities: SlackConversationTabCapabilities,
    ) -> bool {
        self.source_tabs.as_ref() == source_tabs && self.capabilities == capabilities
    }

    pub(crate) fn has_overflow(&self) -> bool {
        self.visible_count < self.tabs.len()
    }
}
