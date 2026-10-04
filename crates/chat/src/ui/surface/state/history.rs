mod recording;
mod targets;

use super::{Context, SlackMainTab, SlackRailView, SurfaceState};
use crate::ui::surface::{
    SlackHistoryDestination, SlackHistoryEntry, SlackMainRoute, SlackShellIcon,
};
use crate::ui::{SlackConversationKind, SlackDraftsSentTab, SlackWorkspace};
use gpui::SharedString;

const SLACK_NAVIGATION_HISTORY_LIMIT: usize = 100;

struct SlackHistoryConversationTarget {
    conversation_id: SharedString,
    rail_view: SlackRailView,
    tab: SlackMainTab,
    source_tab: Option<crate::ui::SlackConversationTab>,
}

impl SurfaceState {
    pub(crate) fn navigate_slack_history_back(&mut self, cx: &mut Context<Self>) {
        let Some(target_index) = self.slack_conversation_history_index.checked_sub(1) else {
            return;
        };
        self.activate_slack_history_index(target_index, cx);
    }

    pub(crate) fn navigate_slack_history_forward(&mut self, cx: &mut Context<Self>) {
        let target_index = self.slack_conversation_history_index + 1;
        if target_index >= self.slack_conversation_history.len() {
            return;
        }
        self.activate_slack_history_index(target_index, cx);
    }

    pub(crate) fn activate_slack_history_index(
        &mut self,
        target_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(destination) = self
            .slack_conversation_history
            .get(target_index)
            .map(|entry| entry.destination.clone())
        else {
            return;
        };
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        self.slack_history_menu_target = None;
        self.slack_history_target_index = Some(target_index);
        self.activate_slack_history_destination(destination, Some(target_index), cx);
    }

    pub(crate) fn activate_slack_history_menu_index(
        &mut self,
        history_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(destination) = self
            .slack_conversation_history
            .get(history_index)
            .map(|entry| entry.destination.clone())
        else {
            return;
        };
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        self.slack_history_target_index = None;
        self.slack_history_menu_target = Some(destination.clone());
        self.activate_slack_history_destination(destination, None, cx);
    }

    fn activate_slack_history_destination(
        &mut self,
        destination: SlackHistoryDestination,
        target_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let expected_destination = destination.clone();
        if !matches!(&destination, SlackHistoryDestination::Search { .. }) {
            self.close_slack_search_results(cx);
            self.close_slack_search(cx);
        }
        let is_conversation = self.route_slack_history_destination(destination, cx);
        if !is_conversation {
            self.finish_slack_non_conversation_history_activation(
                &expected_destination,
                target_index,
            );
        }
    }

    fn route_slack_history_destination(
        &mut self,
        destination: SlackHistoryDestination,
        cx: &mut Context<Self>,
    ) -> bool {
        match destination {
            SlackHistoryDestination::Conversation {
                conversation_id,
                rail_view,
                tab,
                source_tab,
            } => {
                self.activate_slack_history_conversation(
                    SlackHistoryConversationTarget {
                        conversation_id,
                        rail_view,
                        tab,
                        source_tab,
                    },
                    cx,
                );
                true
            }
            SlackHistoryDestination::Rail(view) => {
                self.select_slack_rail_view(view, cx);
                self.record_slack_rail_history(view);
                false
            }
            SlackHistoryDestination::DraftsSent(tab) => {
                self.activate_slack_drafts_sent(cx);
                if self.slack_drafts_sent_tab == tab {
                    self.record_slack_drafts_sent_history(tab);
                } else {
                    self.select_slack_drafts_sent_tab(tab, cx);
                }
                false
            }
            SlackHistoryDestination::AllThreads => {
                self.activate_slack_all_threads(cx);
                self.record_slack_all_threads_history();
                false
            }
            SlackHistoryDestination::Directory => {
                self.activate_slack_directory(cx);
                self.record_slack_directory_history();
                false
            }
            SlackHistoryDestination::NewMessage => {
                self.activate_slack_new_message(cx);
                self.record_slack_new_message_history();
                false
            }
            SlackHistoryDestination::Search { query } => {
                self.open_slack_search_results(query.as_ref(), cx);
                self.record_slack_search_history(query.as_ref());
                false
            }
        }
    }

    fn finish_slack_non_conversation_history_activation(
        &mut self,
        expected_destination: &SlackHistoryDestination,
        target_index: Option<usize>,
    ) {
        if target_index.is_some() && self.slack_history_target_index == target_index {
            self.slack_history_target_index = None;
        }
        if self.slack_history_menu_target.as_ref() == Some(expected_destination) {
            self.slack_history_menu_target = None;
        }
    }

    fn activate_slack_history_conversation(
        &mut self,
        target: SlackHistoryConversationTarget,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_id() != Some(target.conversation_id.as_ref()) {
            self.slack_active_rail_view = target.rail_view;
            self.select_slack_conversation(target.conversation_id.as_ref(), cx);
            return;
        }
        self.leave_slack_directory(cx);
        self.leave_slack_new_message(cx);
        self.slack_active_rail_view = target.rail_view;
        if target.tab != SlackMainTab::BookmarkFolder {
            self.select_slack_tab(target.tab, cx);
            return;
        }
        if let Some(source_tab) = target
            .source_tab
            .and_then(|source_tab| self.current_slack_history_bookmark_folder_tab(&source_tab))
        {
            self.activate_slack_bookmark_folder(source_tab, cx);
        } else {
            self.select_slack_tab(SlackMainTab::Messages, cx);
        }
    }

    pub(crate) fn open_slack_history_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_conversation_history.is_empty() {
            return;
        }
        self.close_slack_search(cx);
        if self.slack_schedule_overlay.is_some() {
            self.dismiss_slack_schedule_layer(cx);
        }
        self.slack_files_menu = None;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        self.slack_channel_details_open = false;
        self.clear_slack_channel_details_view();
        self.slack_history_menu_open = true;
        self.slack_history_menu_selected_index = None;
        let avatar_urls = self
            .slack_conversation_history
            .iter()
            .rev()
            .take(11)
            .filter_map(|entry| entry.avatar_image_url.as_deref())
            .map(str::to_string)
            .collect::<Vec<_>>();
        for url in avatar_urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        cx.notify();
    }

    pub(crate) fn close_slack_history_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_history_menu_open {
            return;
        }
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        cx.notify();
    }

    pub(crate) fn move_slack_history_menu_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_history_menu_open || self.slack_conversation_history.is_empty() {
            return;
        }
        let newest = self.slack_conversation_history.len() - 1;
        let oldest = self.slack_conversation_history.len().saturating_sub(11);
        let next = if let Some(current) = self.slack_history_menu_selected_index {
            if direction < 0 {
                current.saturating_add(1).min(newest)
            } else {
                current.saturating_sub(1).max(oldest)
            }
        } else if direction < 0 {
            oldest
        } else {
            newest
        };
        self.slack_history_menu_selected_index = Some(next);
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_history_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.slack_history_menu_selected_index else {
            return false;
        };
        self.activate_slack_history_menu_index(index, cx);
        true
    }
}

fn slack_history_destinations_match(
    left: &SlackHistoryDestination,
    right: &SlackHistoryDestination,
) -> bool {
    match (left, right) {
        (
            SlackHistoryDestination::Conversation {
                conversation_id: left_conversation_id,
                rail_view: left_rail_view,
                tab: left_tab,
                source_tab: left_source_tab,
            },
            SlackHistoryDestination::Conversation {
                conversation_id: right_conversation_id,
                rail_view: right_rail_view,
                tab: right_tab,
                source_tab: right_source_tab,
            },
        ) => {
            left_conversation_id == right_conversation_id
                && left_rail_view == right_rail_view
                && left_tab == right_tab
                && slack_history_source_tabs_match(left_source_tab, right_source_tab)
        }
        _ => left == right,
    }
}

fn slack_history_source_tabs_match(
    left: &Option<crate::ui::SlackConversationTab>,
    right: &Option<crate::ui::SlackConversationTab>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) if left.id == right.id => matches!(
            (&left.target, &right.target),
            (
                crate::ui::SlackConversationTabTarget::Folder {
                    bookmark_id: left_bookmark_id,
                },
                crate::ui::SlackConversationTabTarget::Folder {
                    bookmark_id: right_bookmark_id,
                },
            ) if left_bookmark_id == right_bookmark_id
        ),
        _ => false,
    }
}

impl SlackHistoryEntry {
    fn route(
        destination: SlackHistoryDestination,
        label: &'static str,
        accessibility_label: &'static str,
        icon: SlackShellIcon,
    ) -> Self {
        Self {
            destination,
            label: label.into(),
            accessibility_label: accessibility_label.into(),
            icon,
            avatar_image_url: None,
        }
    }
}

fn slack_conversation_history_entry(
    workspace: &SlackWorkspace,
    rail_view: SlackRailView,
    tab: SlackMainTab,
    source_tab: Option<crate::ui::SlackConversationTab>,
) -> SlackHistoryEntry {
    let (tab, source_tab) = if tab == SlackMainTab::BookmarkFolder && source_tab.is_none() {
        (SlackMainTab::Messages, None)
    } else {
        (tab, source_tab)
    };
    let label = SharedString::from(workspace.channel_name.clone());
    let avatar_image_url = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.target_id == workspace.conversation_id)
        .and_then(|item| item.avatar_image_url.clone())
        .map(SharedString::from);
    let (icon, accessibility_label) = match workspace.channel_kind {
        SlackConversationKind::Channel => (
            SlackShellIcon::HashSmall,
            SharedString::from(format!("#{}", workspace.channel_name)),
        ),
        SlackConversationKind::PrivateChannel => (
            SlackShellIcon::LockSmall,
            SharedString::from(format!("#{} (Private)", workspace.channel_name)),
        ),
        SlackConversationKind::DirectMessage
        | SlackConversationKind::GroupMessage
        | SlackConversationKind::Unknown => (SlackShellIcon::Dm, label.clone()),
    };
    SlackHistoryEntry {
        destination: SlackHistoryDestination::Conversation {
            conversation_id: SharedString::from(workspace.conversation_id.clone()),
            rail_view,
            tab,
            source_tab,
        },
        label,
        accessibility_label,
        icon,
        avatar_image_url,
    }
}
