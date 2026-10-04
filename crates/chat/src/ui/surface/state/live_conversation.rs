mod read;
mod refresh;
mod unread;

use std::time::Duration;

use gpui::ListOffset;

use crate::ui::surface::{
    build_slack_dm_rows, prepare_slack_conversation_refresh_reusing_rows,
    PreparedSlackConversationSnapshot, SlackConversationLiveTarget, SlackConversationReadReadiness,
    SlackConversationReadRequest, SlackConversationReadState, SlackConversationRefreshRequest,
    SlackMainRoute,
};
use crate::ui::{
    Context, SlackLastReadTimestamp, SlackMainTab, SlackMessageTimestamp, SlackRailView,
    SlackSidebarItem, SurfaceState, Window,
};
use unread::{
    advance_slack_last_read, later_slack_message_timestamp, slack_conversation_read_at_or_after,
    slack_conversation_read_retry_delay, slack_conversation_refresh_delay,
};

const SLACK_CONVERSATION_REFRESH_INTERVAL: Duration = Duration::from_secs(15);
const SLACK_CONVERSATION_READ_DEBOUNCE: Duration = Duration::from_millis(150);

type SlackConversationRefreshResult = Result<Option<PreparedSlackConversationSnapshot>, String>;

impl SurfaceState {
    pub(crate) fn advance_slack_conversation_revision(&mut self) {
        self.slack_conversation_revision = self
            .slack_conversation_revision
            .checked_add(1)
            .expect("Slack conversation revision overflowed");
    }

    pub(crate) fn begin_initial_slack_conversation_refresh(
        &mut self,
        team_id: &str,
        conversation_id: &str,
    ) -> SlackConversationRefreshRequest {
        assert!(
            self.slack_conversation_refresh_request.is_none(),
            "initial Slack conversation refresh requires no in-flight refresh"
        );
        self.slack_conversation_refresh_timer_generation = None;
        let request = SlackConversationRefreshRequest {
            generation: self.slack_conversation_live_generation,
            revision: self.slack_conversation_revision,
            target: SlackConversationLiveTarget {
                team_id: team_id.to_string(),
                conversation_id: conversation_id.to_string(),
            },
        };
        self.slack_conversation_refresh_request = Some(request.clone());
        request
    }

    pub(crate) fn finish_initial_slack_conversation_refresh(
        &mut self,
        request: &SlackConversationRefreshRequest,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_refresh_request.as_ref() != Some(request) {
            return;
        }
        self.slack_conversation_refresh_request = None;
        self.ensure_slack_conversation_live_sync(cx);
    }

    pub(crate) fn ensure_slack_conversation_live_sync(&mut self, cx: &mut Context<Self>) {
        let target_is_current = match (
            self.current_slack_conversation_live_target_ids(),
            self.slack_conversation_live_target.as_ref(),
        ) {
            (Some((team_id, conversation_id)), Some(target)) => {
                target.team_id == team_id && target.conversation_id == conversation_id
            }
            (None, None) => true,
            _ => false,
        };
        if !target_is_current {
            let previous_target = self.slack_conversation_live_target.clone();
            let target = self.current_slack_conversation_live_target_ids().map(
                |(team_id, conversation_id)| SlackConversationLiveTarget {
                    team_id: team_id.to_string(),
                    conversation_id: conversation_id.to_string(),
                },
            );
            self.slack_conversation_live_generation = self
                .slack_conversation_live_generation
                .checked_add(1)
                .expect("Slack conversation live generation overflowed");
            self.slack_conversation_live_target = target;
            self.slack_conversation_refresh_timer_generation = None;
            self.slack_conversation_reconciliation_target = None;
            self.slack_conversation_refresh_failures = 0;
            self.slack_conversation_read_pending = None;
            self.slack_conversation_read_timer = None;
            self.slack_conversation_read_state = None;
            self.slack_conversation_read_failures = 0;
            if let Some(previous_target) = previous_target.filter(|previous_target| {
                self.slack_conversation_read_request
                    .as_ref()
                    .is_none_or(|request| request.target != *previous_target)
            }) {
                self.clear_slack_conversation_read_overlay(&previous_target);
            }
        }
        if self.slack_conversation_live_target.is_none() {
            return;
        }
        if self.slack_conversation_reconciliation_target.is_some() {
            self.schedule_queued_slack_conversation_reconciliation(cx);
        } else if self.slack_workspace_api_capabilities.refresh_conversation
            && self.slack_conversation_refresh_request.is_none()
            && self.slack_conversation_refresh_timer_generation.is_none()
        {
            self.schedule_slack_conversation_refresh(
                self.slack_conversation_live_generation,
                SLACK_CONVERSATION_REFRESH_INTERVAL,
                cx,
            );
        }
        if self.slack_workspace_api_capabilities.mark_conversation_read {
            self.queue_slack_conversation_read(cx);
        }
    }

    pub(crate) fn pause_slack_conversation_read_for_live_positioning(&mut self) {
        self.slack_conversation_read_pending = None;
        self.slack_conversation_read_timer = None;
        self.slack_conversation_read_failures = 0;
        if let Some(state) = self
            .slack_conversation_read_state
            .as_mut()
            .filter(|state| state.readiness != SlackConversationReadReadiness::HeldUnread)
        {
            state.readiness = SlackConversationReadReadiness::AwaitingVisibility {
                check_scheduled: false,
            };
        }
    }

    pub(crate) fn install_slack_conversation_read_after_live_positioning(
        &mut self,
        target: SlackConversationLiveTarget,
        last_read: Option<&SlackLastReadTimestamp>,
        last_read_boundary_loaded: bool,
        cx: &mut Context<Self>,
    ) {
        self.ensure_slack_conversation_live_sync(cx);
        if self.slack_conversation_live_target.as_ref() != Some(&target) {
            return;
        }
        if !last_read_boundary_loaded {
            self.slack_conversation_read_state = None;
            self.clear_slack_conversation_read_overlay(&target);
            return;
        }
        let generation = self.slack_conversation_live_generation;
        let authoritative_floor = last_read.map(|last_read| {
            SlackMessageTimestamp::parse(last_read.as_str())
                .expect("typed Slack last_read must be a valid Slack message timestamp")
        });
        let existing_state = self
            .slack_conversation_read_state
            .take()
            .filter(|state| state.generation == generation && state.target == target);
        let held_unread = existing_state
            .as_ref()
            .is_some_and(|state| state.readiness == SlackConversationReadReadiness::HeldUnread);
        let existing_floor = existing_state.and_then(|state| state.floor);
        self.slack_conversation_read_state = Some(SlackConversationReadState {
            generation,
            revision: self.slack_conversation_revision,
            target,
            floor: if held_unread {
                existing_floor
            } else {
                later_slack_message_timestamp(existing_floor, authoritative_floor)
            },
            readiness: if held_unread {
                SlackConversationReadReadiness::HeldUnread
            } else {
                SlackConversationReadReadiness::AwaitingVisibility {
                    check_scheduled: false,
                }
            },
        });
    }

    pub(crate) fn schedule_slack_conversation_read_visibility_check(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.slack_conversation_read_state.as_mut() else {
            return;
        };
        let SlackConversationReadReadiness::AwaitingVisibility { check_scheduled } =
            &mut state.readiness
        else {
            return;
        };
        if *check_scheduled {
            return;
        }
        *check_scheduled = true;
        let generation = state.generation;
        let revision = state.revision;
        let target = state.target.clone();
        cx.on_next_frame(window, move |this, _window, cx| {
            this.finish_slack_conversation_read_visibility_check(generation, revision, &target, cx);
        });
    }

    fn finish_slack_conversation_read_visibility_check(
        &mut self,
        generation: u64,
        revision: u64,
        target: &SlackConversationLiveTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.slack_conversation_read_state.as_mut() else {
            return;
        };
        if state.generation != generation || state.revision != revision || state.target != *target {
            return;
        }
        state.readiness = SlackConversationReadReadiness::AwaitingVisibility {
            check_scheduled: false,
        };
        let Some(message_timestamp) = self.newest_visible_slack_conversation_timestamp() else {
            return;
        };
        if !self.enable_slack_conversation_read_for_visible_state(generation, revision, target) {
            return;
        }
        self.queue_slack_conversation_read_at(message_timestamp, cx);
    }

    pub(crate) fn queue_slack_conversation_reconciliation(&mut self, cx: &mut Context<Self>) {
        self.ensure_slack_conversation_live_sync(cx);
        let Some(target) = self.slack_conversation_live_target.clone() else {
            return;
        };
        if !self.slack_workspace_api_capabilities.refresh_conversation {
            return;
        }
        self.slack_conversation_reconciliation_target = Some(target);
        self.schedule_queued_slack_conversation_reconciliation(cx);
    }

    pub(in crate::ui::surface::state) fn schedule_queued_slack_conversation_reconciliation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.slack_conversation_reconciliation_target.clone() else {
            return;
        };
        if !self.slack_workspace_api_capabilities.refresh_conversation {
            self.slack_conversation_reconciliation_target = None;
            return;
        }
        if self.slack_conversation_live_target.as_ref() != Some(&target) {
            self.slack_conversation_reconciliation_target = None;
            return;
        }
        if self.slack_conversation_refresh_request.is_some()
            || self.slack_message_history_request.is_some()
            || !self.slack_pending_reactions.is_empty()
            || self.slack_pending_message_saved.is_some()
            || self.slack_pending_message_mark_unread.is_some()
        {
            return;
        }
        self.slack_conversation_refresh_timer_generation = None;
        self.schedule_slack_conversation_refresh(
            self.slack_conversation_live_generation,
            Duration::ZERO,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn current_slack_conversation_live_target_ids(
        &self,
    ) -> Option<(&str, &str)> {
        if !self.active
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
            || self.slack_active_tab != SlackMainTab::Messages
            || self.slack_main_route != SlackMainRoute::Conversation
            || self.slack_search_results_open
        {
            return None;
        }
        let workspace = self.slack_workspace.as_ref()?;
        let snapshot = self.slack_conversation_snapshot.as_ref()?;
        if workspace.team_id.is_empty()
            || workspace.conversation_id.is_empty()
            || workspace.team_id != snapshot.team_id
            || workspace.conversation_id != snapshot.conversation_id
            || self
                .slack_pending_conversation_id
                .as_deref()
                .is_some_and(|pending| pending != workspace.conversation_id)
        {
            return None;
        }
        Some((&workspace.team_id, &workspace.conversation_id))
    }
}
