use std::{collections::HashSet, sync::Arc};

use super::{
    workspace::{slack_attachment_remote_image_urls, slack_message_reaction_remote_image_urls},
    Context, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi,
};
use crate::ui::{
    surface::{prepare_slack_pins_snapshot, PreparedSlackPinsSnapshot, SlackPinsLoadRequest},
    SlackPinsRequest,
};

const SLACK_PINS_INITIAL_VISIBLE_ROWS: usize = 8;

type SlackPinsLoadResult = Result<PreparedSlackPinsSnapshot, String>;

impl SurfaceState {
    pub(crate) fn activate_slack_pins(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.load_pins {
            return;
        }
        let Some((team_id, conversation_id)) = self.slack_workspace().and_then(|workspace| {
            workspace
                .pins_tab()
                .map(|_| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        }) else {
            return;
        };
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.leave_slack_drafts_sent();
        self.slack_active_rail_view = SlackRailView::Home;
        self.slack_active_tab = SlackMainTab::Pins;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.slack_pins_list_state.remeasure();
        let snapshot_is_current = self.slack_pins_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.team_id == team_id && snapshot.conversation_id == conversation_id
        });
        cx.notify();
        if snapshot_is_current {
            self.queue_slack_pins_visible_images(0, SLACK_PINS_INITIAL_VISIBLE_ROWS, cx);
            return;
        }
        self.begin_slack_pins_load(team_id, conversation_id, cx);
    }

    pub(crate) fn retry_slack_pins(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_tab != SlackMainTab::Pins || self.slack_pins_loading {
            return;
        }
        let Some((team_id, conversation_id)) = self
            .slack_workspace()
            .map(|workspace| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        else {
            return;
        };
        self.begin_slack_pins_load(team_id, conversation_id, cx);
    }

    pub(crate) fn handle_slack_pins_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        _count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_pins_visible_images(visible_start, visible_end, cx);
    }

    fn begin_slack_pins_load(
        &mut self,
        team_id: String,
        conversation_id: String,
        cx: &mut Context<Self>,
    ) {
        self.reset_slack_pins_snapshot_for_target(&team_id, &conversation_id);
        let key = (team_id.clone(), conversation_id.clone());
        if let Some(generation) = self.slack_pins_inflight.get(&key).copied() {
            self.slack_pins_request = Some(SlackPinsLoadRequest {
                generation,
                request: SlackPinsRequest {
                    team_id,
                    conversation_id,
                },
            });
            self.slack_pins_loading = true;
            self.slack_pins_error = None;
            cx.notify();
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_pins_error = Some("Slack Pins requires a connected workspace.".to_string());
            cx.notify();
            return;
        };
        self.slack_pins_generation = self
            .slack_pins_generation
            .checked_add(1)
            .expect("Slack Pins request generation overflowed");
        let request = SlackPinsLoadRequest {
            generation: self.slack_pins_generation,
            request: SlackPinsRequest {
                team_id,
                conversation_id,
            },
        };
        self.slack_pins_inflight.insert(key, request.generation);
        self.slack_pins_request = Some(request.clone());
        self.slack_pins_loading = true;
        self.slack_pins_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackPinsLoadRequest)| {
                let result = workspace_api
                    .load_slack_pins(request.request.clone())
                    .map(prepare_slack_pins_snapshot);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_pins_load(request, result, cx);
            },
        );
    }

    fn reset_slack_pins_snapshot_for_target(&mut self, team_id: &str, conversation_id: &str) {
        if self.slack_pins_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.team_id == team_id && snapshot.conversation_id == conversation_id
        }) {
            return;
        }
        self.slack_pins_snapshot = None;
        self.slack_pins_rows = Arc::from([]);
        self.slack_pins_list_state.reset(0);
    }

    fn finish_slack_pins_load(
        &mut self,
        request: SlackPinsLoadRequest,
        result: SlackPinsLoadResult,
        cx: &mut Context<Self>,
    ) {
        let key = (
            request.request.team_id.clone(),
            request.request.conversation_id.clone(),
        );
        if self.slack_pins_inflight.get(&key) == Some(&request.generation) {
            self.slack_pins_inflight.remove(&key);
        }
        if self.slack_pins_request.as_ref() != Some(&request) {
            return;
        }
        self.slack_pins_request = None;
        self.slack_pins_loading = false;
        if !self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == request.request.team_id
                && workspace.conversation_id == request.request.conversation_id
        }) {
            return;
        }
        let prepared = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.slack_pins_error = Some(error);
                cx.notify();
                return;
            }
        };
        if prepared.snapshot.team_id != request.request.team_id
            || prepared.snapshot.conversation_id != request.request.conversation_id
        {
            self.slack_pins_error =
                Some("Slack returned Pins for a different conversation.".to_string());
            cx.notify();
            return;
        }
        self.slack_pins_snapshot = Some(prepared.snapshot);
        self.slack_pins_rows = prepared.rows;
        self.slack_pins_list_state.reset(self.slack_pins_rows.len());
        self.slack_pins_error = None;
        if self.slack_active_tab == SlackMainTab::Pins {
            self.queue_slack_pins_visible_images(0, SLACK_PINS_INITIAL_VISIBLE_ROWS, cx);
        }
        cx.notify();
    }

    fn queue_slack_pins_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let start = visible_start.min(self.slack_pins_rows.len());
        let end = visible_end.min(self.slack_pins_rows.len());
        let mut urls = HashSet::new();
        for row in &self.slack_pins_rows[start..end] {
            urls.extend(row.message.avatar_image_url.iter().cloned());
            urls.extend(slack_message_reaction_remote_image_urls(&row.message).map(str::to_string));
            urls.extend(
                row.message
                    .reply_participants
                    .iter()
                    .filter_map(|participant| {
                        participant
                            .avatar_image_url
                            .as_ref()
                            .map(ToString::to_string)
                    }),
            );
            urls.extend(row.message.latest_reply_avatar_image_url.iter().cloned());
            urls.extend(
                row.message
                    .attachments
                    .iter()
                    .flat_map(|attachment| {
                        slack_attachment_remote_image_urls(&attachment.attachment)
                    })
                    .map(str::to_string),
            );
        }
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
