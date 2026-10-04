use std::sync::Arc;

use gpui::Context;

use super::{SurfaceState, WorkspaceApi};
use crate::model::{SlackRealtimeBatch, SlackRealtimeRecvError, SlackRealtimeThreadTarget};

mod sidebar_refresh;

impl SurfaceState {
    pub(in crate::ui::surface) fn replace_slack_realtime_subscription(
        &mut self,
        team_id: String,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        self.reset_slack_realtime_subscription();
        self.slack_presence_authority.sync_team(&team_id);
        if !self.slack_workspace_api_capabilities.subscribe_realtime {
            self.schedule_slack_realtime_sidebar_refresh(cx);
            return;
        }
        let mut subscription = match workspace_api.subscribe_slack_realtime() {
            Ok(subscription) => subscription,
            Err(error) => {
                report_slack_realtime_error("subscription", &error);
                self.schedule_slack_realtime_sidebar_refresh(cx);
                return;
            }
        };
        self.slack_realtime_team_id = Some(team_id.clone());
        self.slack_realtime_task = Some(cx.spawn(async move |this, cx| loop {
            match subscription.recv().await {
                Ok(batch) => {
                    if this
                        .update(cx, |this, cx| {
                            if this.slack_realtime_team_id.as_deref() == Some(team_id.as_str()) {
                                this.apply_slack_realtime_batch(&team_id, batch, cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(SlackRealtimeRecvError::Lagged) => {
                    if this
                        .update(cx, |this, cx| {
                            if this.slack_realtime_team_id.as_deref() == Some(team_id.as_str()) {
                                this.apply_slack_realtime_resync(cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(SlackRealtimeRecvError::Closed) => {
                    let _ = this.update(cx, |this, _cx| {
                        if this.slack_realtime_team_id.as_deref() == Some(team_id.as_str()) {
                            self::clear_slack_realtime_identity(this);
                        }
                    });
                    break;
                }
            }
        }));
        self.schedule_slack_realtime_sidebar_refresh(cx);
    }

    pub(in crate::ui::surface) fn ensure_slack_realtime_subscription(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let team_id = self
            .slack_shell
            .as_ref()
            .map(|shell| shell.team_id.clone())
            .or_else(|| {
                self.slack_workspace()
                    .map(|workspace| workspace.team_id.clone())
            });
        let Some(team_id) = team_id else {
            self.reset_slack_realtime_subscription();
            return;
        };
        if self.slack_realtime_team_id.as_deref() == Some(team_id.as_str())
            && self.slack_realtime_task.is_some()
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.reset_slack_realtime_subscription();
            return;
        };
        self.replace_slack_realtime_subscription(team_id, workspace_api, cx);
    }

    pub(in crate::ui::surface) fn reset_slack_realtime_subscription(&mut self) {
        self.slack_realtime_task = None;
        self.slack_realtime_team_id = None;
        self.reset_slack_realtime_sidebar_refresh();
        self.slack_realtime_conversation_refresh_pending = false;
        self.slack_realtime_thread_refresh_pending = false;
        self.slack_realtime_activity_stale = false;
        self.slack_realtime_later_stale = false;
        self.slack_realtime_files_stale = false;
        self.slack_realtime_all_threads_stale = false;
    }

    pub(in crate::ui::surface) fn ensure_slack_realtime_pending_refreshes(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.active {
            return;
        }
        if self.slack_realtime_conversation_refresh_pending
            && self.current_slack_conversation_live_target_ids().is_some()
        {
            self.slack_realtime_conversation_refresh_pending = false;
            self.queue_slack_conversation_reconciliation(cx);
        }
        self.schedule_slack_realtime_sidebar_refresh(cx);
        if self.slack_realtime_thread_refresh_pending {
            self.queue_slack_thread_realtime_refresh(cx);
        }
        self.refresh_slack_activity_from_realtime_if_visible(cx);
        self.refresh_slack_later_from_realtime_if_visible(cx);
        self.refresh_slack_files_from_realtime_if_visible(cx);
        self.refresh_slack_all_threads_from_realtime_if_visible(cx);
    }

    fn apply_slack_realtime_batch(
        &mut self,
        team_id: &str,
        batch: SlackRealtimeBatch,
        cx: &mut Context<Self>,
    ) {
        assert!(
            batch.notifications.is_empty(),
            "app-owned Slack realtime must strip native notifications before presentation"
        );
        let patch = self.slack_presence_authority.apply_realtime(
            team_id,
            batch.presence_revision,
            batch.presence_snapshot.as_ref(),
            &batch.presence_changes,
        );
        let presence_changed = {
            let (authority, data) = (&self.slack_presence_authority, &mut self.data);
            authority.patch_loaded_views(data, patch, &batch.presence_changes)
        };
        if presence_changed {
            cx.notify();
        }
        let current_conversation_id = self
            .slack_workspace()
            .map(|workspace| workspace.conversation_id.clone());
        let current_conversation_changed =
            current_conversation_id
                .as_deref()
                .is_some_and(|conversation_id| {
                    batch.full_resync
                        || batch
                            .conversation_ids
                            .iter()
                            .any(|changed| changed == conversation_id)
                        || batch
                            .threads
                            .iter()
                            .any(|thread| thread.conversation_id == conversation_id)
                });
        if current_conversation_changed {
            self.slack_realtime_conversation_refresh_pending = true;
        }
        if batch.full_resync
            || batch.sidebar_changed
            || !batch.conversation_ids.is_empty()
            || !batch.threads.is_empty()
        {
            self.request_slack_realtime_sidebar_refresh();
        }
        if batch.full_resync || self.slack_realtime_thread_changed(batch.threads.as_slice()) {
            self.slack_realtime_thread_refresh_pending = true;
        }
        self.slack_realtime_activity_stale |= batch.full_resync || batch.activity_changed;
        self.slack_realtime_later_stale |= batch.full_resync || batch.later_changed;
        self.slack_realtime_files_stale |= batch.full_resync || batch.files_changed;
        self.slack_realtime_all_threads_stale |= batch.full_resync || batch.all_threads_changed;
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    fn apply_slack_realtime_resync(&mut self, cx: &mut Context<Self>) {
        self.slack_realtime_conversation_refresh_pending = true;
        self.request_slack_realtime_sidebar_refresh();
        self.slack_realtime_thread_refresh_pending = self.slack_thread_panel.is_some();
        self.slack_realtime_activity_stale = true;
        self.slack_realtime_later_stale = true;
        self.slack_realtime_files_stale = true;
        self.slack_realtime_all_threads_stale = true;
        self.ensure_slack_realtime_pending_refreshes(cx);
    }

    fn slack_realtime_thread_changed(&self, threads: &[SlackRealtimeThreadTarget]) -> bool {
        let Some(panel) = self.slack_thread_panel.as_ref() else {
            return false;
        };
        threads.iter().any(|thread| {
            thread.conversation_id == panel.conversation_id
                && thread.thread_timestamp.as_str() == panel.parent_message_id
        })
    }
}

fn report_slack_realtime_error(operation: &str, error: &str) {
    eprintln!("Slack realtime {operation} failed: {error}");
}

fn clear_slack_realtime_identity(surface: &mut SurfaceState) {
    surface.slack_realtime_team_id = None;
}
