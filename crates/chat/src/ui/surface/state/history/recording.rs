use super::{
    slack_conversation_history_entry, Context, SharedString, SlackDraftsSentTab,
    SlackHistoryDestination, SlackHistoryEntry, SlackMainRoute, SlackMainTab, SlackRailView,
    SlackShellIcon, SlackWorkspace, SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn record_slack_rail_history(&mut self, view: SlackRailView) {
        let entry = match view {
            SlackRailView::Home => {
                self.record_current_slack_conversation_history();
                return;
            }
            SlackRailView::DraftsSent => {
                self.record_slack_drafts_sent_history(self.slack_drafts_sent_tab);
                return;
            }
            SlackRailView::Dms => SlackHistoryEntry::route(
                SlackHistoryDestination::Rail(view),
                "DMs",
                "Direct messages",
                SlackShellIcon::Dm,
            ),
            SlackRailView::Activity => SlackHistoryEntry::route(
                SlackHistoryDestination::Rail(view),
                "Activity",
                "Activity",
                SlackShellIcon::Activity,
            ),
            SlackRailView::Files => SlackHistoryEntry::route(
                SlackHistoryDestination::Rail(view),
                "Files",
                "Files",
                SlackShellIcon::Files,
            ),
            SlackRailView::Later => SlackHistoryEntry::route(
                SlackHistoryDestination::Rail(view),
                "Later",
                "Later",
                SlackShellIcon::Later,
            ),
            SlackRailView::More | SlackRailView::Admin => return,
        };
        if self.slack_main_route == SlackMainRoute::Conversation
            && self.slack_active_rail_view == view
        {
            self.record_slack_history_entry(entry);
        }
    }

    pub(in crate::ui::surface::state) fn record_slack_drafts_sent_history(
        &mut self,
        tab: SlackDraftsSentTab,
    ) {
        if self.slack_main_route != SlackMainRoute::Conversation
            || self.slack_active_rail_view != SlackRailView::DraftsSent
            || self.slack_drafts_sent_tab != tab
        {
            return;
        }
        let (label, icon) = match tab {
            SlackDraftsSentTab::Drafts => ("Drafts & sent", SlackShellIcon::DraftsEdit),
            SlackDraftsSentTab::Scheduled => ("Scheduled", SlackShellIcon::Scheduled),
            SlackDraftsSentTab::Sent => ("Sent", SlackShellIcon::Drafts),
        };
        self.record_slack_history_entry(SlackHistoryEntry::route(
            SlackHistoryDestination::DraftsSent(tab),
            label,
            label,
            icon,
        ));
    }

    pub(in crate::ui::surface::state) fn record_slack_new_message_history(&mut self) {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return;
        }
        self.record_slack_history_entry(SlackHistoryEntry::route(
            SlackHistoryDestination::NewMessage,
            "New message",
            "New message",
            SlackShellIcon::Compose,
        ));
    }

    pub(in crate::ui::surface::state) fn record_slack_directory_history(&mut self) {
        if self.slack_main_route != SlackMainRoute::Directory {
            return;
        }
        self.record_slack_history_entry(SlackHistoryEntry::route(
            SlackHistoryDestination::Directory,
            "People",
            "People directory",
            SlackShellIcon::Directories,
        ));
    }

    pub(in crate::ui::surface::state) fn record_slack_all_threads_history(&mut self) {
        if self.slack_main_route != SlackMainRoute::AllThreads {
            return;
        }
        self.record_slack_history_entry(SlackHistoryEntry::route(
            SlackHistoryDestination::AllThreads,
            "Threads",
            "Threads",
            SlackShellIcon::ReplyThread,
        ));
    }

    pub(in crate::ui::surface::state) fn record_slack_search_history(&mut self, query: &str) {
        if !self.slack_search_results_open || query.is_empty() {
            return;
        }
        let label = SharedString::from(format!("Search: {query}"));
        self.record_slack_history_entry(SlackHistoryEntry {
            destination: SlackHistoryDestination::Search {
                query: SharedString::from(query.to_string()),
            },
            label: label.clone(),
            accessibility_label: label,
            icon: SlackShellIcon::Search,
            avatar_image_url: None,
        });
    }

    pub(in crate::ui::surface::state) fn record_current_slack_conversation_history(&mut self) {
        if self.slack_main_route != SlackMainRoute::Conversation
            || self.slack_search_results_open
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
        {
            return;
        }
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let entry = slack_conversation_history_entry(
            workspace,
            self.slack_active_rail_view,
            self.slack_active_tab,
            self.active_slack_history_source_tab(workspace),
        );
        self.record_slack_history_entry(entry);
    }

    pub(in crate::ui::surface::state) fn sync_slack_conversation_history(
        &mut self,
        workspace: &SlackWorkspace,
    ) {
        self.apply_slack_history_conversation_target(&workspace.conversation_id);
        if self
            .slack_history_bookmark_folder_target(&workspace.conversation_id)
            .is_some()
        {
            return;
        }
        if self.slack_main_route != SlackMainRoute::Conversation
            || self.slack_search_results_open
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
        {
            return;
        }
        let entry = slack_conversation_history_entry(
            workspace,
            self.slack_active_rail_view,
            self.slack_active_tab,
            self.active_slack_history_source_tab(workspace),
        );
        self.record_slack_history_entry(entry);
    }

    pub(in crate::ui::surface::state) fn sync_current_slack_conversation_history(&mut self) {
        let Some(conversation_id) = self
            .slack_workspace()
            .map(|workspace| workspace.conversation_id.clone())
        else {
            return;
        };
        self.apply_slack_history_conversation_target(&conversation_id);
        if self
            .slack_history_bookmark_folder_target(&conversation_id)
            .is_some()
        {
            return;
        }
        if self.slack_main_route != SlackMainRoute::Conversation
            || self.slack_search_results_open
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
        {
            return;
        }
        let Some(entry) = self.slack_workspace().map(|workspace| {
            slack_conversation_history_entry(
                workspace,
                self.slack_active_rail_view,
                self.slack_active_tab,
                self.active_slack_history_source_tab(workspace),
            )
        }) else {
            return;
        };
        self.record_slack_history_entry(entry);
    }

    pub(in crate::ui::surface::state) fn activate_current_slack_conversation_history_tab(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::Conversation {
            return;
        }
        let Some(conversation_id) = self
            .slack_workspace()
            .map(|workspace| workspace.conversation_id.clone())
        else {
            return;
        };
        if let Some(source_tab) = self.slack_history_bookmark_folder_target(&conversation_id) {
            if let Some(current_tab) = self.current_slack_history_bookmark_folder_tab(&source_tab) {
                self.activate_slack_bookmark_folder(current_tab, cx);
            } else {
                self.select_slack_tab(SlackMainTab::Messages, cx);
            }
            return;
        }
        match self.slack_active_tab {
            SlackMainTab::Messages => {}
            SlackMainTab::Canvas => {
                if !self.slack_workspace_api_capabilities.load_canvas
                    || self
                        .slack_workspace()
                        .is_none_or(|workspace| workspace.canvas_tab().is_none())
                {
                    self.slack_active_tab = SlackMainTab::Messages;
                    cx.notify();
                }
            }
            SlackMainTab::BookmarkFolder => {
                if !self.slack_bookmark_folder_context_is_current() {
                    self.reset_slack_bookmark_folder_context();
                    cx.notify();
                }
            }
            SlackMainTab::FilesLinks => self.activate_slack_conversation_files(cx),
            SlackMainTab::Pins => self.activate_slack_pins(cx),
        }
    }
}
