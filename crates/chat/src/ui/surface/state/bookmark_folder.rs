use std::sync::Arc;

use super::{Context, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    prepare_slack_bookmark_folder_snapshot, PreparedSlackBookmarkFolderSnapshot,
    SlackBookmarkFolderIdentity, SlackBookmarkFolderLoad, SlackMainRoute,
};
use crate::ui::{SlackConversationTab, SlackWorkspace};

type SlackBookmarkFolderLoadResult = Result<PreparedSlackBookmarkFolderSnapshot, String>;

impl SurfaceState {
    pub(crate) fn activate_slack_bookmark_folder(
        &mut self,
        tab: SlackConversationTab,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_bookmark_folder {
            return;
        }
        let Some((identity, label)) = self.slack_workspace().and_then(|workspace| {
            SlackBookmarkFolderIdentity::from_tab(
                &workspace.team_id,
                &workspace.conversation_id,
                &tab,
            )
            .map(|identity| (identity, tab.label.clone()))
        }) else {
            return;
        };

        self.reset_slack_schedule_context(cx);
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        if self.slack_main_route == SlackMainRoute::AllThreads {
            self.leave_slack_all_threads();
            self.slack_main_route = SlackMainRoute::Conversation;
        }
        if self.slack_main_route == SlackMainRoute::NewMessage {
            self.leave_slack_new_message(cx);
        }
        if self.slack_main_route == SlackMainRoute::Directory {
            self.leave_slack_directory(cx);
        }
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_conversation_files();
        self.leave_slack_drafts_sent();
        self.reset_slack_bookmark_folder_context();
        self.slack_active_bookmark_folder = Some(identity);
        self.slack_active_bookmark_folder_label = Some(label.into());
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::BookmarkFolder;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.record_current_slack_conversation_history();
        self.begin_slack_bookmark_folder_load(cx);
    }

    pub(crate) fn retry_slack_bookmark_folder(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_tab != SlackMainTab::BookmarkFolder
            || self.slack_bookmark_folder_loading
            || self.slack_active_bookmark_folder.is_none()
        {
            return;
        }
        self.begin_slack_bookmark_folder_load(cx);
    }

    pub(crate) fn open_slack_bookmark_folder_item(
        &mut self,
        target_url: &str,
        cx: &mut Context<Self>,
    ) {
        self.open_slack_link(target_url, cx);
    }

    pub(crate) fn leave_slack_bookmark_folder(&mut self) {
        if self.slack_active_tab != SlackMainTab::BookmarkFolder
            && self.slack_active_bookmark_folder.is_none()
            && !self.slack_bookmark_folder_loading
            && self.slack_bookmark_folder_rows.is_empty()
            && self.slack_bookmark_folder_error.is_none()
        {
            return;
        }
        self.reset_slack_bookmark_folder_context();
    }

    pub(crate) fn reset_slack_bookmark_folder_context(&mut self) {
        self.slack_bookmark_folder_generation = self
            .slack_bookmark_folder_generation
            .checked_add(1)
            .expect("Slack bookmark folder generation overflowed");
        self.slack_active_bookmark_folder = None;
        self.slack_active_bookmark_folder_label = None;
        self.slack_bookmark_folder_rows = Arc::default();
        self.slack_bookmark_folder_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_bookmark_folder_loading = false;
        self.slack_bookmark_folder_error = None;
        if self.slack_active_tab == SlackMainTab::BookmarkFolder {
            self.slack_active_tab = SlackMainTab::Messages;
        }
    }

    pub(crate) fn sync_slack_bookmark_folder_context(&mut self, workspace: &SlackWorkspace) {
        let Some(identity) = self.slack_active_bookmark_folder.as_ref() else {
            return;
        };
        let matching_tab = workspace
            .tabs
            .iter()
            .find(|tab| identity.matches_tab(tab))
            .map(|tab| tab.label.clone());
        if !identity.matches_workspace(&workspace.team_id, &workspace.conversation_id)
            || !self.slack_workspace_api_capabilities.load_bookmark_folder
            || matching_tab.is_none()
        {
            self.reset_slack_bookmark_folder_context();
            return;
        }
        self.slack_active_bookmark_folder_label = matching_tab.map(gpui::SharedString::from);
    }

    pub(crate) fn slack_bookmark_folder_context_is_current(&self) -> bool {
        let Some(identity) = self.slack_active_bookmark_folder.as_ref() else {
            return false;
        };
        self.slack_workspace().is_some_and(|workspace| {
            identity.matches_workspace(&workspace.team_id, &workspace.conversation_id)
                && workspace.tabs.iter().any(|tab| identity.matches_tab(tab))
        })
    }

    fn begin_slack_bookmark_folder_load(&mut self, cx: &mut Context<Self>) {
        if self.slack_bookmark_folder_loading {
            return;
        }
        let Some(identity) = self.slack_active_bookmark_folder.clone() else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_bookmark_folder_error =
                Some("This folder requires a connected Slack workspace.".into());
            cx.notify();
            return;
        };
        self.slack_bookmark_folder_generation = self
            .slack_bookmark_folder_generation
            .checked_add(1)
            .expect("Slack bookmark folder generation overflowed");
        let load = SlackBookmarkFolderLoad {
            generation: self.slack_bookmark_folder_generation,
            identity,
        };
        self.slack_bookmark_folder_rows = Arc::default();
        self.slack_bookmark_folder_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_bookmark_folder_loading = true;
        self.slack_bookmark_folder_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, load),
            cx,
            |(workspace_api, load): (Arc<dyn WorkspaceApi>, SlackBookmarkFolderLoad)| {
                let result = workspace_api
                    .load_slack_bookmark_folder(load.identity.request())
                    .map(prepare_slack_bookmark_folder_snapshot);
                (load, result)
            },
            |this, (load, result), cx| {
                this.finish_slack_bookmark_folder_load(load, result, cx);
            },
        );
    }

    fn finish_slack_bookmark_folder_load(
        &mut self,
        load: SlackBookmarkFolderLoad,
        result: SlackBookmarkFolderLoadResult,
        cx: &mut Context<Self>,
    ) {
        let current = self.slack_active_tab == SlackMainTab::BookmarkFolder
            && self.slack_bookmark_folder_generation == load.generation
            && self.slack_active_bookmark_folder.as_ref() == Some(&load.identity)
            && self.slack_workspace().is_some_and(|workspace| {
                load.identity
                    .matches_workspace(&workspace.team_id, &workspace.conversation_id)
            });
        if !current {
            return;
        }
        self.slack_bookmark_folder_loading = false;
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_bookmark_folder_error = Some(error.into());
                cx.notify();
                return;
            }
        };
        if prepared.team_id != load.identity.team_id.as_ref()
            || prepared.conversation_id != load.identity.conversation_id.as_ref()
            || prepared.folder_bookmark_id != load.identity.folder_bookmark_id.as_ref()
        {
            self.slack_bookmark_folder_error =
                Some("Slack returned items for a different folder.".into());
            cx.notify();
            return;
        }
        self.slack_bookmark_folder_rows = prepared.rows;
        for icon_url in self
            .slack_bookmark_folder_rows
            .iter()
            .filter_map(|row| row.icon_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
        {
            self.enqueue_slack_remote_image_url(icon_url, cx);
        }
        self.slack_bookmark_folder_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_bookmark_folder_error = None;
        cx.notify();
    }
}
