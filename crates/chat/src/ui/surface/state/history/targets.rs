use super::{
    slack_history_destinations_match, SlackHistoryDestination, SlackHistoryEntry, SlackMainRoute,
    SlackMainTab, SlackWorkspace, SurfaceState, SLACK_NAVIGATION_HISTORY_LIMIT,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_slack_history_conversation_target(
        &mut self,
        conversation_id: &str,
    ) {
        if let Some(SlackHistoryDestination::Conversation {
            conversation_id: target_conversation_id,
            rail_view,
            tab,
            ..
        }) = self.slack_history_target_destination()
        {
            if target_conversation_id.as_ref() == conversation_id {
                self.slack_main_route = SlackMainRoute::Conversation;
                self.slack_active_rail_view = rail_view;
                self.slack_active_tab = if tab == SlackMainTab::BookmarkFolder
                    || (tab == SlackMainTab::Canvas
                        && !self.slack_workspace_api_capabilities.load_canvas)
                {
                    SlackMainTab::Messages
                } else {
                    tab
                };
            }
        }
    }

    pub(in crate::ui::surface::state) fn slack_history_target_destination(
        &self,
    ) -> Option<SlackHistoryDestination> {
        self.slack_history_target_index
            .and_then(|index| self.slack_conversation_history.get(index))
            .map(|entry| entry.destination.clone())
            .or_else(|| self.slack_history_menu_target.clone())
    }

    pub(in crate::ui::surface::state) fn slack_history_bookmark_folder_target(
        &self,
        conversation_id: &str,
    ) -> Option<crate::ui::SlackConversationTab> {
        let SlackHistoryDestination::Conversation {
            conversation_id: target_conversation_id,
            tab: SlackMainTab::BookmarkFolder,
            source_tab: Some(source_tab),
            ..
        } = self.slack_history_target_destination()?
        else {
            return None;
        };
        (target_conversation_id.as_ref() == conversation_id).then_some(source_tab)
    }

    pub(in crate::ui::surface::state) fn current_slack_history_bookmark_folder_tab(
        &self,
        source_tab: &crate::ui::SlackConversationTab,
    ) -> Option<crate::ui::SlackConversationTab> {
        if !self.slack_workspace_api_capabilities.load_bookmark_folder {
            return None;
        }
        let workspace = self.slack_workspace()?;
        let identity = crate::ui::surface::SlackBookmarkFolderIdentity::from_tab(
            &workspace.team_id,
            &workspace.conversation_id,
            source_tab,
        )?;
        workspace
            .tabs
            .iter()
            .find(|tab| identity.matches_tab(tab))
            .cloned()
    }

    pub(in crate::ui::surface::state) fn active_slack_history_source_tab(
        &self,
        workspace: &SlackWorkspace,
    ) -> Option<crate::ui::SlackConversationTab> {
        let identity = self.slack_active_bookmark_folder.as_ref()?;
        workspace
            .tabs
            .iter()
            .find(|tab| identity.matches_tab(tab))
            .cloned()
    }

    pub(in crate::ui::surface::state) fn record_slack_history_entry(
        &mut self,
        entry: SlackHistoryEntry,
    ) {
        if self.slack_history_menu_target.is_some() {
            self.slack_history_menu_target = None;
        }
        if let Some(target_index) = self.slack_history_target_index {
            if self
                .slack_conversation_history
                .get(target_index)
                .is_some_and(|target| {
                    slack_history_destinations_match(&target.destination, &entry.destination)
                })
            {
                self.slack_conversation_history[target_index] = entry;
                self.slack_conversation_history_index = target_index;
                self.slack_history_target_index = None;
                return;
            }
            self.slack_history_target_index = None;
        }
        let current_index = self.slack_conversation_history_index;
        if let Some(current) = self.slack_conversation_history.get_mut(current_index) {
            if slack_history_destinations_match(&current.destination, &entry.destination) {
                *current = entry;
                return;
            }
        }
        if !self.slack_conversation_history.is_empty() {
            self.slack_conversation_history.truncate(current_index + 1);
        }
        self.slack_conversation_history.push(entry);
        if self.slack_conversation_history.len() > SLACK_NAVIGATION_HISTORY_LIMIT {
            let remove_count =
                self.slack_conversation_history.len() - SLACK_NAVIGATION_HISTORY_LIMIT;
            self.slack_conversation_history.drain(..remove_count);
        }
        self.slack_conversation_history_index =
            self.slack_conversation_history.len().saturating_sub(1);
    }
}
