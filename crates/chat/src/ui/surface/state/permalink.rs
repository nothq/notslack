mod navigation;
mod parser;

use std::time::Duration;

use super::{px, Context, SurfaceState};
use crate::ui::surface::{SlackMessageActionTarget, SlackMessageRenderContext};
use crate::ui::SlackMessageTimestamp;
use gpui::{ListOffset, SharedString};

use parser::{parse_slack_link, ParsedSlackLink};

const SLACK_MESSAGE_HIGHLIGHT_DURATION: Duration = Duration::from_millis(4_000);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageNavigationTarget {
    team_id: Option<String>,
    conversation_id: String,
    message_timestamp: SlackMessageTimestamp,
    thread_timestamp: Option<SlackMessageTimestamp>,
}

impl SlackMessageNavigationTarget {
    fn from_action_target(target: &SlackMessageActionTarget) -> Self {
        Self {
            team_id: Some(target.team_id().to_string()),
            conversation_id: target.conversation_id().to_string(),
            message_timestamp: target.message_timestamp().clone(),
            thread_timestamp: target.thread_timestamp().cloned(),
        }
    }

    pub(crate) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub(crate) fn anchor_timestamp(&self) -> &SlackMessageTimestamp {
        self.thread_timestamp
            .as_ref()
            .unwrap_or(&self.message_timestamp)
    }

    fn message_timestamp(&self) -> &SlackMessageTimestamp {
        &self.message_timestamp
    }

    fn thread_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        self.thread_timestamp.as_ref()
    }

    fn matches_team(&self, team_id: &str) -> bool {
        self.team_id
            .as_deref()
            .is_none_or(|target_team_id| target_team_id == team_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageNavigationRequest {
    pub(crate) generation: u64,
    pub(crate) target: SlackMessageNavigationTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMessageNavigationHighlight {
    generation: u64,
    conversation_id: SharedString,
    message_timestamp: SharedString,
    render_context: SlackMessageRenderContext,
}

impl SlackMessageNavigationHighlight {
    pub(crate) fn matches(
        &self,
        conversation_id: &str,
        message_timestamp: &str,
        render_context: SlackMessageRenderContext,
    ) -> bool {
        self.conversation_id.as_ref() == conversation_id
            && self.message_timestamp.as_ref() == message_timestamp
            && self.render_context == render_context
    }
}

impl SurfaceState {
    pub(crate) fn open_slack_notification_target(
        &mut self,
        target: crate::model::SlackNotificationTarget<'_>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let crate::model::SlackNotificationTarget {
            team_id,
            conversation_id,
            message_timestamp,
            thread_timestamp,
            launch_uri,
        } = target;
        if let Some(launch_uri) = launch_uri {
            self.open_slack_link(launch_uri, cx);
            return Ok(());
        }
        let Some(message_timestamp) = message_timestamp.or(thread_timestamp) else {
            self.select_slack_conversation(conversation_id, cx);
            return Ok(());
        };
        let message_timestamp = SlackMessageTimestamp::parse(message_timestamp)?;
        let thread_timestamp = thread_timestamp
            .map(SlackMessageTimestamp::parse)
            .transpose()?;
        let target = SlackMessageNavigationTarget {
            team_id: Some(team_id.to_string()),
            conversation_id: conversation_id.to_string(),
            message_timestamp,
            thread_timestamp,
        };
        let workspace = self
            .slack_workspace()
            .ok_or("Slack workspace is unavailable")?;
        if !target.matches_team(&workspace.team_id) {
            return Err("Slack notification belongs to another workspace".to_string());
        }
        self.open_slack_message_navigation_target(target, cx);
        Ok(())
    }

    pub(super) fn begin_initial_slack_message_navigation(
        &mut self,
        team_id: String,
        conversation_id: String,
        message_timestamp: SlackMessageTimestamp,
    ) -> SlackMessageNavigationRequest {
        self.begin_slack_message_navigation(SlackMessageNavigationTarget {
            team_id: Some(team_id),
            conversation_id,
            message_timestamp,
            thread_timestamp: None,
        })
    }

    pub(crate) fn open_slack_all_threads_thread(
        &mut self,
        conversation_id: &str,
        thread_timestamp: &str,
        cx: &mut Context<Self>,
    ) {
        let Ok(timestamp) = SlackMessageTimestamp::parse(thread_timestamp) else {
            self.slack_all_threads_error =
                Some("Slack All Threads returned an invalid thread timestamp.".to_string());
            cx.notify();
            return;
        };
        let target = SlackMessageNavigationTarget {
            team_id: self
                .slack_workspace()
                .map(|workspace| workspace.team_id.clone()),
            conversation_id: conversation_id.to_string(),
            message_timestamp: timestamp.clone(),
            thread_timestamp: Some(timestamp),
        };
        self.begin_slack_message_navigation(target.clone());
        if self.slack_conversation_id() == Some(target.conversation_id())
            && self.slack_message_navigation_target_is_loaded(&target)
        {
            self.leave_slack_all_threads();
            self.slack_main_route = crate::ui::surface::SlackMainRoute::Conversation;
            self.continue_slack_message_navigation(cx);
            return;
        }
        self.select_slack_conversation_for_message(target.conversation_id(), cx);
    }

    pub(crate) fn open_slack_link(&mut self, link: &str, cx: &mut Context<Self>) {
        let target = match parse_slack_link(link) {
            Ok(ParsedSlackLink::External) => {
                cx.open_url(link);
                return;
            }
            Ok(ParsedSlackLink::Message(target)) => target,
            Err(error) => {
                self.slack_error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(workspace) = self.slack_workspace() else {
            cx.open_url(link);
            return;
        };
        if !target.matches_team(&workspace.team_id) {
            cx.open_url(link);
            return;
        }

        self.open_slack_message_navigation_target(target, cx);
    }

    pub(crate) fn open_slack_message_action_target(
        &mut self,
        action_target: &SlackMessageActionTarget,
        cx: &mut Context<Self>,
    ) {
        let target = SlackMessageNavigationTarget::from_action_target(action_target);
        let Some(workspace) = self.slack_workspace() else {
            self.slack_error = Some("Slack workspace is unavailable.".to_string());
            cx.notify();
            return;
        };
        if !target.matches_team(&workspace.team_id) {
            self.slack_error =
                Some("Slack message target belongs to another workspace.".to_string());
            cx.notify();
            return;
        }
        self.open_slack_message_navigation_target(target, cx);
    }

    fn open_slack_message_navigation_target(
        &mut self,
        target: SlackMessageNavigationTarget,
        cx: &mut Context<Self>,
    ) {
        self.leave_slack_directory(cx);
        self.begin_slack_message_navigation(target.clone());

        if self.slack_conversation_id() == Some(target.conversation_id())
            && self.slack_message_navigation_target_is_loaded(&target)
        {
            self.continue_slack_message_navigation(cx);
            return;
        }
        self.select_slack_conversation_for_message(target.conversation_id(), cx);
    }

    fn begin_slack_message_navigation(
        &mut self,
        target: SlackMessageNavigationTarget,
    ) -> SlackMessageNavigationRequest {
        self.slack_message_navigation_generation = self
            .slack_message_navigation_generation
            .checked_add(1)
            .expect("Slack message navigation generation overflowed");
        let request = SlackMessageNavigationRequest {
            generation: self.slack_message_navigation_generation,
            target,
        };
        self.slack_message_navigation = Some(request.clone());
        self.slack_message_navigation_highlight = None;
        self.slack_message_navigation_focus_pending = false;
        self.slack_error = None;
        request
    }

    pub(super) fn cancel_slack_message_navigation(&mut self) {
        self.slack_message_navigation_generation = self
            .slack_message_navigation_generation
            .checked_add(1)
            .expect("Slack message navigation generation overflowed");
        self.slack_message_navigation = None;
        self.slack_message_navigation_highlight = None;
        self.slack_message_navigation_focus_pending = false;
    }

    pub(super) fn slack_message_navigation_requires_conversation_load(
        &self,
        conversation_id: &str,
    ) -> bool {
        self.slack_message_navigation
            .as_ref()
            .is_some_and(|request| request.target.conversation_id() == conversation_id)
    }

    pub(super) fn slack_message_navigation_anchor(
        &self,
        conversation_id: &str,
    ) -> Option<SlackMessageTimestamp> {
        self.slack_message_navigation.as_ref().and_then(|request| {
            (request.target.conversation_id() == conversation_id)
                .then(|| request.target.anchor_timestamp().clone())
        })
    }

    fn slack_message_navigation_target_is_loaded(
        &self,
        target: &SlackMessageNavigationTarget,
    ) -> bool {
        let required_timestamp = target
            .thread_timestamp()
            .unwrap_or_else(|| target.message_timestamp())
            .as_str();
        self.slack_message_rows
            .iter()
            .any(|row| row.id == required_timestamp)
    }
}
