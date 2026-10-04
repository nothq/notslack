use std::{collections::HashSet, sync::Arc};

use gpui::Context;

use super::{next_slack_activity_generation, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    mutate_and_prepare_slack_activity_snapshot, prepare_slack_activity_workspace_context,
    prepare_slack_sidebar_snapshot, reconcile_and_prepare_slack_activity_snapshot,
    rollback_and_prepare_slack_activity_snapshot, PreparedSlackActivitySnapshot,
    PreparedSlackSidebarSnapshot, SlackActivityItemMutation, SlackActivityItemMutationRequest,
    SlackActivityItemMutationRollback, SlackActivityMutationContext, SlackActivityQueuedMutation,
};

mod actions;

struct SlackActivityItemMutationRefresh {
    activity: Option<crate::ui::SlackActivitySnapshot>,
    sidebar: Result<PreparedSlackSidebarSnapshot, String>,
    feed_refresh_error: Option<String>,
}

struct SlackActivityItemMutationRefreshContext {
    conversation_id: String,
    collapsed_sections: HashSet<String>,
    muted_conversations: HashSet<String>,
}

impl SurfaceState {
    fn queue_slack_activity_item_mutation(
        &mut self,
        mutation: SlackActivityItemMutation,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_item_mutation_is_pending(mutation.key()) {
            return;
        }
        if self.active_slack_workspace_api().is_none() {
            self.slack_activity_mutation_error = Some("missing Slack workspace api".into());
            self.slack_activity_mutation_error_request_id = None;
            cx.notify();
            return;
        }
        let Some(snapshot) = self.slack_activity_snapshot.clone() else {
            return;
        };
        let Some(item) = snapshot
            .items
            .iter()
            .find(|item| item.key == mutation.key())
        else {
            return;
        };
        let rollback = SlackActivityItemMutationRollback {
            unread: item.unread,
            archived: item.archived,
        };
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let workspace = prepare_slack_activity_workspace_context(workspace);
        let prepared = mutate_and_prepare_slack_activity_snapshot(snapshot, &mutation, &workspace);
        self.slack_activity_snapshot = Some(prepared.snapshot);
        self.slack_activity_rows = prepared.rows;
        self.slack_activity_item_mutation_queue
            .push_back(SlackActivityQueuedMutation { mutation, rollback });
        self.slack_activity_mutation_error = None;
        self.slack_activity_mutation_error_request_id = None;
        self.refresh_slack_activity_visible_rows(cx);
        self.queue_slack_activity_visible_images(0, super::SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS, cx);
        self.start_next_slack_activity_item_mutation(cx);
        cx.notify();
    }

    fn start_next_slack_activity_item_mutation(&mut self, cx: &mut Context<Self>) {
        if self.slack_activity_item_mutation_request.is_some() {
            return;
        }
        let Some(queued) = self.slack_activity_item_mutation_queue.pop_front() else {
            return;
        };
        let Some(context) = self.current_slack_activity_mutation_context() else {
            self.rollback_all_slack_activity_item_mutations(queued, cx);
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.rollback_all_slack_activity_item_mutations(queued, cx);
            self.slack_activity_mutation_error = Some("missing Slack workspace api".into());
            self.slack_activity_mutation_error_request_id = None;
            cx.notify();
            return;
        };
        let request = SlackActivityItemMutationRequest {
            request_id: self.next_slack_activity_mutation_request_id(),
            context,
            mutation: queued.mutation,
            rollback: queued.rollback,
        };
        let refresh_context = SlackActivityItemMutationRefreshContext {
            conversation_id: request.context.conversation_id.clone(),
            collapsed_sections: self.slack_collapsed_sections.clone(),
            muted_conversations: self.slack_muted_conversations.clone(),
        };
        self.slack_activity_item_mutation_request = Some(request.clone());
        self.slack_activity_mutation_error = None;
        self.slack_activity_mutation_error_request_id = None;
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            move |(workspace_api, request)| {
                let result = load_slack_activity_item_mutation_refresh(
                    workspace_api,
                    &request.mutation,
                    refresh_context,
                );
                (request, result)
            },
            move |this, (request, result), cx| {
                this.finish_slack_activity_item_mutation(request, result, cx);
            },
        );
    }

    fn finish_slack_activity_item_mutation(
        &mut self,
        request: SlackActivityItemMutationRequest,
        result: Result<SlackActivityItemMutationRefresh, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_activity_item_mutation_request.as_ref() != Some(&request) {
            return;
        }
        self.slack_activity_item_mutation_request = None;
        if !self.slack_activity_mutation_context_is_current(&request.context) {
            return;
        }
        match result {
            Ok(refresh) => self.apply_slack_activity_item_mutation_refresh(&request, refresh, cx),
            Err(error) => {
                eprintln!(
                    "Slack Activity item update failed for {}: {error}",
                    request.mutation.key()
                );
                self.rollback_slack_activity_item_mutation(
                    &SlackActivityQueuedMutation {
                        mutation: request.mutation.clone(),
                        rollback: request.rollback,
                    },
                    cx,
                );
                self.slack_activity_mutation_error =
                    Some(request.mutation.failure_message().into());
                self.slack_activity_mutation_error_request_id = Some(request.request_id);
            }
        }
        self.start_next_slack_activity_item_mutation(cx);
        cx.notify();
    }

    fn apply_slack_activity_item_mutation_refresh(
        &mut self,
        request: &SlackActivityItemMutationRequest,
        refresh: SlackActivityItemMutationRefresh,
        cx: &mut Context<Self>,
    ) {
        if let Some(refreshed_snapshot) = refresh.activity {
            let Some(existing_snapshot) = self.slack_activity_snapshot.clone() else {
                return;
            };
            let Some(workspace) = self.slack_workspace() else {
                return;
            };
            let workspace = prepare_slack_activity_workspace_context(workspace);
            let mut mutations =
                Vec::with_capacity(1 + self.slack_activity_item_mutation_queue.len());
            mutations.push(request.mutation.clone());
            mutations.extend(
                self.slack_activity_item_mutation_queue
                    .iter()
                    .map(|queued| queued.mutation.clone()),
            );
            let prepared = reconcile_and_prepare_slack_activity_snapshot(
                existing_snapshot,
                refreshed_snapshot,
                &mutations,
                &workspace,
            );
            self.apply_prepared_slack_activity_mutation(prepared, cx);
        }
        self.clear_slack_activity_mutation_error_through(request.request_id);
        if let Some(error) = refresh.feed_refresh_error {
            eprintln!(
                "Slack Activity feed refresh failed after {} {}: {error}",
                request.mutation.success_description(),
                request.mutation.key()
            );
            self.slack_activity_mutation_error =
                Some("Updated the item, but couldn’t refresh Activity.".into());
            self.slack_activity_mutation_error_request_id = Some(request.request_id);
        }
        match refresh.sidebar {
            Ok(sidebar) => self.apply_slack_activity_mutation_sidebar(
                request.request_id,
                &request.context,
                sidebar,
                cx,
            ),
            Err(error) => {
                eprintln!(
                    "Slack Activity sidebar refresh failed after {} {}: {error}",
                    request.mutation.success_description(),
                    request.mutation.key()
                );
                self.slack_activity_mutation_error =
                    Some("Updated the item, but couldn’t refresh Activity counts.".into());
                self.slack_activity_mutation_error_request_id = Some(request.request_id);
            }
        }
    }

    fn apply_prepared_slack_activity_mutation(
        &mut self,
        prepared: PreparedSlackActivitySnapshot,
        cx: &mut Context<Self>,
    ) {
        self.slack_activity_generation = next_slack_activity_generation(
            self.slack_activity_generation,
            "Slack Activity request generation overflowed",
        );
        self.slack_activity_loading = false;
        self.slack_activity_snapshot = Some(prepared.snapshot);
        self.slack_activity_rows = prepared.rows;
        self.refresh_slack_activity_visible_rows(cx);
        self.queue_slack_activity_visible_images(0, super::SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS, cx);
    }

    fn rollback_slack_activity_item_mutation(
        &mut self,
        queued: &SlackActivityQueuedMutation,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self.slack_activity_snapshot.clone() else {
            return;
        };
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let workspace = prepare_slack_activity_workspace_context(workspace);
        let prepared = rollback_and_prepare_slack_activity_snapshot(
            snapshot,
            queued.mutation.key(),
            queued.rollback,
            &workspace,
        );
        self.apply_prepared_slack_activity_mutation(prepared, cx);
    }

    fn rollback_all_slack_activity_item_mutations(
        &mut self,
        current: SlackActivityQueuedMutation,
        cx: &mut Context<Self>,
    ) {
        while let Some(queued) = self.slack_activity_item_mutation_queue.pop_back() {
            self.rollback_slack_activity_item_mutation(&queued, cx);
        }
        self.rollback_slack_activity_item_mutation(&current, cx);
    }

    fn apply_slack_activity_mutation_sidebar(
        &mut self,
        request_id: u64,
        context: &SlackActivityMutationContext,
        sidebar: PreparedSlackSidebarSnapshot,
        cx: &mut Context<Self>,
    ) {
        if request_id <= self.slack_activity_mutation_applied_sidebar_request_id
            || sidebar.snapshot.team_id != context.team_id
            || sidebar.snapshot.conversation_id != context.conversation_id
        {
            return;
        }
        self.slack_activity_mutation_applied_sidebar_request_id = request_id;
        self.apply_prepared_slack_sidebar_snapshot(sidebar, cx);
    }

    fn current_slack_activity_mutation_context(&self) -> Option<SlackActivityMutationContext> {
        self.slack_workspace()
            .map(|workspace| SlackActivityMutationContext {
                generation: self.slack_activity_mutation_context_generation,
                team_id: workspace.team_id.clone(),
                conversation_id: workspace.conversation_id.clone(),
            })
    }

    fn slack_activity_mutation_context_is_current(
        &self,
        context: &SlackActivityMutationContext,
    ) -> bool {
        self.slack_activity_mutation_context_generation == context.generation
            && self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == context.team_id
                    && workspace.conversation_id == context.conversation_id
            })
    }

    fn next_slack_activity_mutation_request_id(&mut self) -> u64 {
        self.slack_activity_mutation_next_request_id = next_slack_activity_generation(
            self.slack_activity_mutation_next_request_id,
            "Slack Activity mutation request id overflowed",
        );
        self.slack_activity_mutation_next_request_id
    }

    fn clear_slack_activity_mutation_error_through(&mut self, request_id: u64) {
        if self
            .slack_activity_mutation_error_request_id
            .is_none_or(|error_request_id| error_request_id <= request_id)
        {
            self.slack_activity_mutation_error = None;
            self.slack_activity_mutation_error_request_id = None;
        }
    }

    pub(crate) fn slack_activity_mutation_is_pending(&self) -> bool {
        self.slack_activity_item_mutation_request.is_some()
            || !self.slack_activity_item_mutation_queue.is_empty()
    }

    pub(crate) fn slack_activity_item_mutation_is_pending(&self, key: &str) -> bool {
        self.slack_activity_item_mutation_request
            .as_ref()
            .is_some_and(|request| request.mutation.key() == key)
            || self
                .slack_activity_item_mutation_queue
                .iter()
                .any(|queued| queued.mutation.key() == key)
    }

    pub(in crate::ui::surface::state) fn reset_slack_activity_mutation_context(&mut self) {
        self.slack_activity_mutation_context_generation = next_slack_activity_generation(
            self.slack_activity_mutation_context_generation,
            "Slack Activity mutation context generation overflowed",
        );
        self.slack_activity_item_mutation_request = None;
        self.slack_activity_item_mutation_queue.clear();
        self.slack_activity_mutation_error = None;
        self.slack_activity_mutation_error_request_id = None;
    }
}

fn load_slack_activity_item_mutation_refresh(
    workspace_api: Arc<dyn WorkspaceApi>,
    mutation: &SlackActivityItemMutation,
    context: SlackActivityItemMutationRefreshContext,
) -> Result<SlackActivityItemMutationRefresh, String> {
    mutate_slack_activity_item(workspace_api.as_ref(), mutation)?;
    let (activity, feed_refresh_error) = match workspace_api.load_slack_activity(None) {
        Ok(snapshot) => (Some(snapshot), None),
        Err(error) => (None, Some(error)),
    };
    let SlackActivityItemMutationRefreshContext {
        conversation_id,
        collapsed_sections,
        muted_conversations,
    } = context;
    let sidebar = workspace_api
        .load_slack_sidebar(&conversation_id)
        .map(|sidebar| {
            prepare_slack_sidebar_snapshot(sidebar, &collapsed_sections, &muted_conversations)
        });
    Ok(SlackActivityItemMutationRefresh {
        activity,
        sidebar,
        feed_refresh_error,
    })
}

fn mutate_slack_activity_item(
    workspace_api: &dyn WorkspaceApi,
    mutation: &SlackActivityItemMutation,
) -> Result<(), String> {
    match mutation {
        SlackActivityItemMutation::MarkRead(target) => {
            workspace_api.mark_slack_activity_item_read(target)
        }
        SlackActivityItemMutation::MarkUnread(target) => {
            workspace_api.mark_slack_activity_item_unread(target)
        }
        SlackActivityItemMutation::Archive { target, reason } => {
            workspace_api.archive_slack_activity_item(target, reason)
        }
        SlackActivityItemMutation::Unarchive { target, reason } => {
            workspace_api.unarchive_slack_activity_item(target, reason)
        }
    }
}
