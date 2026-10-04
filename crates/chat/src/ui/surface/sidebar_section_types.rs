use gpui::UniformListScrollHandle;

use crate::ui::{SlackConversationKind, SlackSidebarSectionSort};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSidebarSectionChoice {
    pub(crate) conversation_id: String,
    pub(crate) label: String,
    pub(crate) kind: SlackConversationKind,
    pub(crate) selected: bool,
    pub(crate) source_index: usize,
}

pub(crate) struct SlackSidebarSectionDialog {
    pub(crate) name: String,
    pub(crate) sort: SlackSidebarSectionSort,
    pub(crate) choices: Vec<SlackSidebarSectionChoice>,
    pub(crate) scroll_handle: UniformListScrollHandle,
    pub(crate) saving: bool,
    pub(crate) error: Option<String>,
}

impl SlackSidebarSectionDialog {
    pub(crate) fn selected_count(&self) -> usize {
        self.choices.iter().filter(|choice| choice.selected).count()
    }
}
