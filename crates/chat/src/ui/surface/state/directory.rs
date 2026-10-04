mod input;
mod selection;

use std::sync::Arc;

use super::{Context, SurfaceState};
use crate::ui::surface::{
    prepare_slack_destination_directory, PreparedSlackDestinationDirectory, SlackMainRoute,
    SlackMainTab, SlackNewMessageCandidateKind, SlackRailView,
};
use crate::ui::SlackDestinationTarget;

const SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS: usize = 12;

impl SurfaceState {
    pub(crate) fn activate_slack_directory(&mut self, cx: &mut Context<Self>) {
        let Some(team_id) = self.slack_directory_activation_team_id() else {
            return;
        };
        self.sync_slack_directory_team(team_id);
        if self.reactivate_slack_directory(cx) {
            return;
        }
        self.prepare_slack_directory_entry(cx);
        self.install_slack_directory_route();
        self.rebuild_slack_directory_visible_rows();
        self.finish_slack_directory_activation(cx);
    }

    fn slack_directory_activation_team_id(&self) -> Option<String> {
        if !self.is_slack_workspace()
            || !self
                .slack_workspace_api_capabilities
                .load_destination_directory
        {
            return None;
        }
        self.slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
    }

    fn sync_slack_directory_team(&mut self, team_id: String) {
        if self.slack_directory_team_id.as_deref() == Some(team_id.as_str()) {
            return;
        }
        self.reset_slack_directory_context();
        self.slack_directory_team_id = Some(team_id);
    }

    fn reactivate_slack_directory(&mut self, cx: &mut Context<Self>) -> bool {
        if self.slack_main_route != SlackMainRoute::Directory {
            return false;
        }
        self.slack_directory_focus_pending = true;
        self.rebuild_slack_directory_visible_rows();
        self.finish_slack_directory_activation(cx);
        true
    }

    fn prepare_slack_directory_entry(&mut self, cx: &mut Context<Self>) {
        self.leave_slack_bookmark_folder();
        self.leave_slack_all_threads();
        self.leave_slack_new_message(cx);
        self.reset_slack_schedule_context(cx);
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.close_slack_search_results(cx);
        self.close_slack_search(cx);
        self.reset_slack_reaction_context();
        self.reset_slack_message_action_context();
        self.park_slack_main_composer(cx);
    }

    fn install_slack_directory_route(&mut self) {
        self.slack_main_route = SlackMainRoute::Directory;
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::Messages;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_expanded_attachment = None;
        self.slack_composer_focused = false;
        self.slack_error = None;
        self.slack_directory_error = None;
        self.slack_directory_query.clear();
        self.slack_directory_normalized_query = Default::default();
        self.slack_directory_focus_pending = true;
    }

    fn finish_slack_directory_activation(&mut self, cx: &mut Context<Self>) {
        self.record_slack_directory_history();
        self.refresh_slack_directory(cx);
        cx.notify();
    }

    pub(crate) fn leave_slack_directory(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::Directory {
            return;
        }
        self.slack_directory_generation = next_slack_directory_generation(
            self.slack_directory_generation,
            "Slack People directory generation overflowed",
        );
        self.slack_directory_loading = false;
        self.slack_directory_query.clear();
        self.slack_directory_normalized_query = Default::default();
        self.slack_directory_visible_row_indices = Arc::default();
        self.slack_directory_selected_index = None;
        self.slack_directory_prefetched_range = None;
        self.slack_directory_focus_pending = false;
        self.slack_profile_panel = None;
        self.slack_main_route = SlackMainRoute::Conversation;
        self.restore_slack_routed_main_composer(cx);
        cx.notify();
    }

    pub(in crate::ui::surface) fn reset_slack_directory_context(&mut self) {
        self.slack_directory_generation = next_slack_directory_generation(
            self.slack_directory_generation,
            "Slack People directory generation overflowed",
        );
        self.slack_directory_team_id = None;
        self.slack_directory_snapshot = None;
        self.slack_directory_rows = Arc::default();
        self.slack_directory_visible_row_indices = Arc::default();
        self.slack_directory_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_directory_query.clear();
        self.slack_directory_normalized_query = Default::default();
        self.slack_directory_selected_index = None;
        self.slack_directory_prefetched_range = None;
        self.slack_directory_loading = false;
        self.slack_directory_error = None;
        self.slack_directory_focus_pending = false;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_directory(data);
    }

    fn refresh_slack_directory(&mut self, cx: &mut Context<Self>) {
        if self.slack_directory_snapshot.is_none() {
            self.begin_slack_directory_load(cx);
        } else {
            self.queue_slack_directory_visible_images(0, SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS, cx);
        }
    }

    pub(crate) fn retry_slack_directory(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::Directory || self.slack_directory_loading {
            return;
        }
        self.slack_directory_error = None;
        self.begin_slack_directory_load(cx);
    }

    fn begin_slack_directory_load(&mut self, cx: &mut Context<Self>) {
        if self.slack_directory_loading {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_directory_error =
                Some("Slack People requires a connected workspace.".to_string());
            cx.notify();
            return;
        };
        let Some(team_id) = self.slack_directory_team_id.clone() else {
            return;
        };
        self.slack_directory_generation = next_slack_directory_generation(
            self.slack_directory_generation,
            "Slack People directory generation overflowed",
        );
        let generation = self.slack_directory_generation;
        self.slack_directory_loading = true;
        self.slack_directory_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, team_id, generation),
            cx,
            |(workspace_api, team_id, generation)| {
                let result = workspace_api
                    .load_slack_destination_directory()
                    .and_then(prepare_slack_people_directory);
                (team_id, generation, result)
            },
            |this, (team_id, generation, result), cx| {
                this.finish_slack_directory_load(&team_id, generation, result, cx);
            },
        );
    }

    fn finish_slack_directory_load(
        &mut self,
        team_id: &str,
        generation: u64,
        result: Result<PreparedSlackDestinationDirectory, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_directory_team_id.as_deref() != Some(team_id)
            || self.slack_directory_generation != generation
        {
            return;
        }
        self.slack_directory_loading = false;
        match result {
            Ok(prepared) if prepared.snapshot.team_id == team_id => {
                self.apply_slack_directory_load(prepared, cx);
            }
            Ok(_) => {
                self.slack_directory_error =
                    Some("Slack People returned another workspace.".to_string());
            }
            Err(error) => self.slack_directory_error = Some(error),
        }
        cx.notify();
    }

    fn apply_slack_directory_load(
        &mut self,
        mut prepared: PreparedSlackDestinationDirectory,
        cx: &mut Context<Self>,
    ) {
        self.slack_presence_authority
            .overlay_prepared_destination(&mut prepared);
        self.slack_directory_snapshot = Some(prepared.snapshot);
        self.slack_directory_rows = prepared.rows;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_directory(data);
        self.rebuild_slack_directory_visible_rows();
        self.slack_directory_error = None;
        self.slack_directory_prefetched_range = None;
        self.queue_slack_directory_visible_images(0, SLACK_DIRECTORY_INITIAL_VISIBLE_ROWS, cx);
    }
}

fn prepare_slack_people_directory(
    mut snapshot: crate::ui::SlackDestinationDirectorySnapshot,
) -> Result<PreparedSlackDestinationDirectory, String> {
    snapshot
        .candidates
        .retain(|candidate| matches!(&candidate.target, SlackDestinationTarget::Person { .. }));
    let prepared = prepare_slack_destination_directory(snapshot)?;
    if prepared
        .rows
        .iter()
        .any(|row| row.kind != SlackNewMessageCandidateKind::Person)
    {
        return Err("Slack People directory candidate projection was inconsistent.".to_string());
    }
    Ok(prepared)
}

fn next_slack_directory_generation(generation: u64, overflow_message: &str) -> u64 {
    generation.checked_add(1).expect(overflow_message)
}
