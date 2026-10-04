use std::time::Duration;

use gpui::Context;

use super::report_slack_realtime_error;
use crate::ui::surface::{
    prepare_slack_sidebar_snapshot, PreparedSlackSidebarSnapshot, SurfaceState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackRealtimeSidebarRefreshRequest {
    lifecycle_generation: u64,
    request_generation: u64,
    team_id: String,
    conversation_id: String,
    read_epoch: u64,
}

impl SlackRealtimeSidebarRefreshRequest {
    fn token(&self) -> (u64, u64) {
        (self.lifecycle_generation, self.request_generation)
    }
}

type SlackRealtimeSidebarRefreshResult = Result<PreparedSlackSidebarSnapshot, String>;

impl SurfaceState {
    pub(super) fn reset_slack_realtime_sidebar_refresh(&mut self) {
        self.slack_realtime_sidebar_refresh_lifecycle_generation = self
            .slack_realtime_sidebar_refresh_lifecycle_generation
            .checked_add(1)
            .expect("Slack realtime sidebar refresh lifecycle generation overflowed");
        self.slack_realtime_sidebar_refresh_retry_timer = None;
    }

    pub(super) fn request_slack_realtime_sidebar_refresh(&mut self) {
        self.slack_realtime_sidebar_refresh_requested_generation = self
            .slack_realtime_sidebar_refresh_requested_generation
            .checked_add(1)
            .expect("Slack realtime sidebar refresh request generation overflowed");
    }

    pub(super) fn schedule_slack_realtime_sidebar_refresh(&mut self, cx: &mut Context<Self>) {
        if !self.active
            || self.slack_realtime_sidebar_refresh_requested_generation
                == self.slack_realtime_sidebar_refresh_completed_generation
            || self.slack_realtime_sidebar_refresh_in_flight.is_some()
            || self.slack_realtime_sidebar_refresh_retry_timer.is_some()
            || !self.slack_workspace_api_capabilities.load_sidebar
        {
            return;
        }
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let request = SlackRealtimeSidebarRefreshRequest {
            lifecycle_generation: self.slack_realtime_sidebar_refresh_lifecycle_generation,
            request_generation: self.slack_realtime_sidebar_refresh_requested_generation,
            team_id: workspace.team_id.clone(),
            conversation_id: workspace.conversation_id.clone(),
            read_epoch: self.slack_sidebar_read_epoch,
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let collapsed_sections = self.slack_collapsed_sections.clone();
        let muted_conversations = self.slack_muted_conversations.clone();
        self.slack_realtime_sidebar_refresh_in_flight = Some(request.token());
        self.spawn_background_task(
            request,
            cx,
            move |request| {
                let result = workspace_api
                    .refresh_slack_sidebar_from_realtime(&request.conversation_id)
                    .and_then(|snapshot| {
                        validate_slack_realtime_sidebar_snapshot(&request, &snapshot)?;
                        Ok(prepare_slack_sidebar_snapshot(
                            snapshot,
                            &collapsed_sections,
                            &muted_conversations,
                        ))
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_realtime_sidebar_refresh(request, result, cx);
            },
        );
    }

    fn finish_slack_realtime_sidebar_refresh(
        &mut self,
        request: SlackRealtimeSidebarRefreshRequest,
        result: SlackRealtimeSidebarRefreshResult,
        cx: &mut Context<Self>,
    ) {
        if self.slack_realtime_sidebar_refresh_in_flight != Some(request.token()) {
            return;
        }
        self.slack_realtime_sidebar_refresh_in_flight = None;
        if !self.slack_realtime_sidebar_refresh_request_is_current(&request) {
            self.request_slack_realtime_sidebar_refresh();
            self.schedule_slack_realtime_sidebar_refresh(cx);
            return;
        }
        match result {
            Ok(prepared) => {
                self.apply_prepared_slack_sidebar_snapshot(prepared, cx);
                self.slack_realtime_sidebar_refresh_completed_generation =
                    request.request_generation;
                self.slack_realtime_sidebar_refresh_failures = 0;
                self.schedule_slack_realtime_sidebar_refresh(cx);
            }
            Err(error) => {
                report_slack_realtime_error("sidebar refresh", &error);
                self.slack_realtime_sidebar_refresh_failures = self
                    .slack_realtime_sidebar_refresh_failures
                    .saturating_add(1);
                self.schedule_slack_realtime_sidebar_refresh_retry(cx);
            }
        }
    }

    fn slack_realtime_sidebar_refresh_request_is_current(
        &self,
        request: &SlackRealtimeSidebarRefreshRequest,
    ) -> bool {
        request.lifecycle_generation == self.slack_realtime_sidebar_refresh_lifecycle_generation
            && request.read_epoch == self.slack_sidebar_read_epoch
            && self.slack_workspace().is_some_and(|workspace| {
                workspace.team_id == request.team_id
                    && workspace.conversation_id == request.conversation_id
            })
    }

    fn schedule_slack_realtime_sidebar_refresh_retry(&mut self, cx: &mut Context<Self>) {
        if self.slack_realtime_sidebar_refresh_retry_timer.is_some() {
            return;
        }
        let token = (
            self.slack_realtime_sidebar_refresh_lifecycle_generation,
            self.slack_realtime_sidebar_refresh_requested_generation,
        );
        let delay = slack_realtime_sidebar_refresh_retry_delay(
            self.slack_realtime_sidebar_refresh_failures,
        );
        self.slack_realtime_sidebar_refresh_retry_timer = Some(token);
        self.spawn_timer_task(token, delay, cx, |this, token, cx| {
            if this.slack_realtime_sidebar_refresh_retry_timer != Some(token) {
                return;
            }
            this.slack_realtime_sidebar_refresh_retry_timer = None;
            this.schedule_slack_realtime_sidebar_refresh(cx);
        });
    }

    pub(in crate::ui::surface::state) fn queue_slack_realtime_sidebar_refresh_after_read(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_sidebar {
            return;
        }
        self.request_slack_realtime_sidebar_refresh();
        self.schedule_slack_realtime_sidebar_refresh(cx);
    }
}

fn validate_slack_realtime_sidebar_snapshot(
    request: &SlackRealtimeSidebarRefreshRequest,
    snapshot: &crate::model::SlackSidebarSnapshot,
) -> Result<(), String> {
    if snapshot.team_id == request.team_id && snapshot.conversation_id == request.conversation_id {
        Ok(())
    } else {
        Err("Slack realtime sidebar refresh targeted another workspace".to_string())
    }
}

fn slack_realtime_sidebar_refresh_retry_delay(failures: u8) -> Duration {
    match failures {
        0 | 1 => Duration::from_secs(2),
        2 => Duration::from_secs(5),
        3 => Duration::from_secs(15),
        4 => Duration::from_secs(30),
        _ => Duration::from_secs(60),
    }
}
