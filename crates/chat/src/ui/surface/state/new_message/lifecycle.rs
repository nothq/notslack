use super::{
    next_slack_new_message_generation, prepare_slack_destination_directory, Arc, Context,
    SlackMainRoute, SlackMainTab, SlackRailView, SurfaceState,
    SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS,
};
use crate::ui::surface::PreparedSlackDestinationDirectory;
use crate::ui::surface::{SlackComposerDestination, SlackComposerDraftKey};

impl SurfaceState {
    pub(crate) fn activate_slack_new_message(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace()
            || !self
                .slack_workspace_api_capabilities
                .load_destination_directory
        {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            return;
        };
        self.leave_slack_directory(cx);
        if self.slack_main_route != SlackMainRoute::NewMessage {
            self.cancel_slack_composer_capture_for_owner_change(cx);
        }
        if self.slack_new_message_team_id.as_deref() != Some(team_id.as_str()) {
            self.reset_slack_new_message_team_context(cx);
            self.slack_new_message_team_id = Some(team_id);
        }
        if self.reactivate_slack_new_message(cx) {
            return;
        }
        self.enter_slack_new_message_route(cx);
        self.refresh_slack_new_message_directory(cx);
    }

    fn reactivate_slack_new_message(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return false;
        }
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        self.record_slack_new_message_history();
        cx.notify();
        self.refresh_slack_new_message_directory(cx);
        true
    }

    fn enter_slack_new_message_route(&mut self, cx: &mut Context<Self>) {
        self.leave_slack_bookmark_folder();
        self.leave_slack_all_threads();
        self.reset_slack_schedule_context(cx);
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        let return_key = self.slack_workspace().and_then(|workspace| {
            Some(SlackComposerDraftKey {
                team_id: workspace.team_id.clone(),
                self_user_id: workspace.self_user_id.clone()?,
                destination: SlackComposerDestination::Conversation {
                    conversation_id: workspace.conversation_id.clone(),
                },
            })
        });
        self.slack_new_message_return_draft = return_key.map(|key| {
            let owner = crate::ui::surface::SlackMainComposerDraftOwner::Conversation(key.clone());
            assert!(
                self.slack_active_main_composer_context
                    .as_ref()
                    .is_some_and(|context| context.owner == owner),
                "entering New message must park the exact routed conversation draft"
            );
            let draft = self.take_slack_send_draft();
            self.clear_slack_main_composer_after_draft_taken(&owner);
            (key, draft)
        });
        self.close_slack_search_results(cx);
        self.close_slack_search(cx);
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        self.slack_main_route = SlackMainRoute::NewMessage;
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::Messages;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_expanded_attachment = None;
        self.reset_slack_new_message_selection();
        self.slack_new_message_to_focused = true;
        self.restore_slack_send_draft(None);
        self.slack_composer_focused = false;
        self.slack_error = None;
        self.slack_new_message_error = None;
        self.rebuild_slack_new_message_results();
        self.record_slack_new_message_history();
        cx.notify();
    }

    fn refresh_slack_new_message_directory(&mut self, cx: &mut Context<Self>) {
        if self.slack_new_message_directory_snapshot.is_none() {
            self.begin_slack_new_message_directory_load(cx);
        } else {
            self.queue_slack_new_message_visible_images(
                0,
                SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS,
                cx,
            );
        }
    }

    pub(crate) fn leave_slack_new_message(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return;
        }
        self.cancel_pending_slack_new_message_conversation_load(cx);
        self.store_slack_new_message_draft(cx);
        self.slack_main_route = SlackMainRoute::Conversation;
        self.restore_slack_routed_main_composer(cx);
        self.slack_new_message_to_focused = false;
        self.slack_new_message_open_generation =
            next_slack_new_message_generation(self.slack_new_message_open_generation);
        self.slack_new_message_pending_open = None;
        self.reset_slack_new_message_selection();
        let restored = self
            .slack_new_message_return_draft
            .take()
            .is_some_and(|(key, draft)| {
                let SlackComposerDestination::Conversation { conversation_id } = &key.destination
                else {
                    panic!("new-message return draft must target a conversation");
                };
                let matches_workspace = self.slack_workspace().is_some_and(|workspace| {
                    workspace.team_id == key.team_id
                        && workspace.self_user_id.as_deref() == Some(key.self_user_id.as_str())
                        && workspace.conversation_id == *conversation_id
                });
                if matches_workspace {
                    self.restore_slack_send_draft(Some(draft));
                } else {
                    self.store_slack_composer_draft(key, draft);
                }
                matches_workspace
            });
        if !restored {
            self.restore_slack_send_draft(None);
        }
        self.slack_composer_focused = false;
        self.slack_error = None;
        self.slack_new_message_error = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn reset_slack_new_message_team_context(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_new_message_team_id = None;
        self.slack_new_message_directory_snapshot = None;
        self.slack_new_message_rows = Arc::default();
        self.slack_new_message_visible_row_indices = Arc::default();
        self.slack_new_message_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_new_message_directory_generation =
            next_slack_new_message_generation(self.slack_new_message_directory_generation);
        self.slack_new_message_directory_loading = false;
        self.slack_new_message_open_generation =
            next_slack_new_message_generation(self.slack_new_message_open_generation);
        self.slack_new_message_pending_open = None;
        let drafts = self.slack_new_message_drafts.drain().collect::<Vec<_>>();
        for (key, draft) in drafts {
            self.discard_slack_composer_draft(
                crate::ui::surface::SlackFileStagingDraftOwner::Main(
                    crate::ui::surface::SlackMainComposerDraftOwner::NewMessage(key),
                ),
                draft,
                cx,
            );
        }
        if let Some((key, draft)) = self.slack_new_message_return_draft.take() {
            self.discard_slack_composer_draft(
                crate::ui::surface::SlackFileStagingDraftOwner::Main(
                    crate::ui::surface::SlackMainComposerDraftOwner::Conversation(key),
                ),
                draft,
                cx,
            );
        }
        self.slack_new_message_prefetched_range = None;
        self.slack_new_message_error = None;
        self.reset_slack_new_message_selection();
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_new_message(data);
    }

    pub(in crate::ui::surface::state) fn reset_slack_new_message_selection(&mut self) {
        self.slack_new_message_query.clear();
        self.slack_new_message_normalized_query = Default::default();
        self.slack_new_message_selected_index = None;
        self.slack_new_message_selected_people.clear();
        self.slack_new_message_destination = None;
        self.slack_new_message_active_draft_key = None;
        self.slack_new_message_prefetched_range = None;
        self.slack_new_message_visible_row_indices = Arc::default();
        self.slack_new_message_scroll_handle = gpui::UniformListScrollHandle::new();
    }

    pub(in crate::ui::surface::state) fn begin_slack_new_message_directory_load(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_new_message_directory_loading {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_new_message_error =
                Some("Slack New message requires a connected workspace.".to_string());
            cx.notify();
            return;
        };
        let Some(team_id) = self.slack_new_message_team_id.clone() else {
            return;
        };
        self.slack_new_message_directory_generation =
            next_slack_new_message_generation(self.slack_new_message_directory_generation);
        let generation = self.slack_new_message_directory_generation;
        self.slack_new_message_directory_loading = true;
        self.slack_new_message_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, team_id, generation),
            cx,
            |(workspace_api, team_id, generation)| {
                let result = workspace_api
                    .load_slack_destination_directory()
                    .and_then(prepare_slack_destination_directory);
                (team_id, generation, result)
            },
            |this, (team_id, generation, result), cx| {
                this.finish_slack_new_message_directory_load(&team_id, generation, result, cx);
            },
        );
    }

    fn finish_slack_new_message_directory_load(
        &mut self,
        team_id: &str,
        generation: u64,
        result: Result<PreparedSlackDestinationDirectory, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_new_message_team_id.as_deref() != Some(team_id)
            || self.slack_new_message_directory_generation != generation
        {
            return;
        }
        self.slack_new_message_directory_loading = false;
        match result {
            Ok(mut prepared) if prepared.snapshot.team_id == team_id => {
                self.slack_presence_authority
                    .overlay_prepared_destination(&mut prepared);
                self.slack_new_message_directory_snapshot = Some(prepared.snapshot);
                self.slack_new_message_rows = prepared.rows;
                let (authority, data) = (&mut self.slack_presence_authority, &self.data);
                authority.reindex_new_message(data);
                self.rebuild_slack_new_message_results();
                self.queue_slack_new_message_visible_images(
                    0,
                    SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS,
                    cx,
                );
            }
            Ok(_) => {
                self.slack_new_message_error =
                    Some("Slack destination directory returned another workspace.".to_string());
            }
            Err(error) => self.slack_new_message_error = Some(error),
        }
        cx.notify();
    }
}
