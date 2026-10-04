use std::sync::Arc;

use crate::ui::surface::{SlackConversationTabAction, SlackMainTab, SurfaceState};
use crate::ui::{Context, SlackConversationTab, SlackConversationTabTarget};

impl SurfaceState {
    pub(super) fn activate_slack_conversation_tab_action(
        &mut self,
        action: Arc<SlackConversationTabAction>,
        cx: &mut Context<Self>,
    ) {
        self.close_slack_conversation_tabs_overflow_after_action(cx);
        match action.as_ref() {
            SlackConversationTabAction::Messages => {
                self.select_slack_tab(SlackMainTab::Messages, cx);
            }
            SlackConversationTabAction::AddCanvas => {}
            SlackConversationTabAction::CanvasLink { tab, permalink } => {
                let Some(current) = self.current_slack_conversation_source_tab(tab) else {
                    return;
                };
                let SlackConversationTabTarget::Canvas {
                    permalink: Some(current_permalink),
                    ..
                } = current.target
                else {
                    return;
                };
                if current_permalink.trim().is_empty()
                    || current_permalink.as_str() != permalink.as_ref()
                {
                    return;
                }
                self.open_slack_link(&current_permalink, cx);
            }
            SlackConversationTabAction::BookmarkFolder(tab) => {
                let Some(current) = self.current_slack_conversation_source_tab(tab) else {
                    return;
                };
                self.activate_slack_bookmark_folder(current, cx);
            }
            SlackConversationTabAction::Files => {
                if self.slack_workspace().is_some_and(|workspace| {
                    workspace.tabs.iter().any(|tab| {
                        !tab.is_disabled && tab.target == SlackConversationTabTarget::Files
                    })
                }) {
                    self.select_slack_tab(SlackMainTab::FilesLinks, cx);
                }
            }
            SlackConversationTabAction::Pins => {
                if self.slack_workspace().is_some_and(|workspace| {
                    workspace.tabs.iter().any(|tab| {
                        !tab.is_disabled && tab.target == SlackConversationTabTarget::Pins
                    })
                }) {
                    self.select_slack_tab(SlackMainTab::Pins, cx);
                }
            }
        }
    }

    fn close_slack_conversation_tabs_overflow_after_action(&mut self, cx: &mut Context<Self>) {
        let overflow_was_open = self.slack_conversation_tabs_overflow_open;
        self.slack_conversation_tabs_overflow_open = false;
        self.slack_conversation_tabs_overflow_selected_index = None;
        self.slack_conversation_tabs_overflow_focus_pending = false;
        if overflow_was_open {
            cx.notify();
        }
    }

    fn current_slack_conversation_source_tab(
        &self,
        source: &SlackConversationTab,
    ) -> Option<SlackConversationTab> {
        let workspace = self.slack_workspace()?;
        workspace
            .tabs
            .iter()
            .find(|tab| {
                !tab.is_disabled
                    && tab.id == source.id
                    && slack_conversation_tab_targets_share_identity(&tab.target, &source.target)
            })
            .cloned()
    }
}

fn slack_conversation_tab_targets_share_identity(
    current: &SlackConversationTabTarget,
    source: &SlackConversationTabTarget,
) -> bool {
    match (current, source) {
        (
            SlackConversationTabTarget::Canvas {
                file_id: current_file_id,
                ..
            },
            SlackConversationTabTarget::Canvas {
                file_id: source_file_id,
                ..
            },
        ) => current_file_id == source_file_id,
        (
            SlackConversationTabTarget::Folder {
                bookmark_id: current_bookmark_id,
            },
            SlackConversationTabTarget::Folder {
                bookmark_id: source_bookmark_id,
            },
        ) => current_bookmark_id == source_bookmark_id,
        (SlackConversationTabTarget::Files, SlackConversationTabTarget::Files)
        | (SlackConversationTabTarget::Pins, SlackConversationTabTarget::Pins) => true,
        (
            SlackConversationTabTarget::Unsupported {
                source_type: current_source_type,
            },
            SlackConversationTabTarget::Unsupported {
                source_type: source_source_type,
            },
        ) => current_source_type == source_source_type,
        _ => false,
    }
}
