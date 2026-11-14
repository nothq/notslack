//! Starting, running and leaving a huddle: Slack's `rooms.join` hands out
//! the Chime meeting, the huddle crate carries the audio, and `rooms.leave`
//! tells Slack when it ends.

use gpui::SharedString;
use huddle::{HuddleEvent, HuddleMediaConfig, HuddleSession};

use crate::ui::surface::{
    spawn_background_task_for_entity, Context, SlackHuddleState, SlackHuddleStatus, SurfaceState,
};
use crate::ui::{SlackHuddleJoinReceipt, SlackHuddleJoinRequest, SlackHuddleLeaveRequest};

/// What Slack's client sends when someone leaves on purpose.
const SLACK_HUDDLE_LEAVE_REASON: &str = "user_initiated";

impl SurfaceState {
    /// The header's huddle button: starts a huddle in the open conversation,
    /// or leaves the one running there.
    pub(crate) fn toggle_slack_huddle(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let conversation_id = workspace.conversation_id.clone();
        if let Some(huddle) = self.slack_huddle.as_ref() {
            if huddle.conversation_id == conversation_id {
                self.leave_slack_huddle(cx);
            } else {
                self.slack_huddle_error =
                    Some(format!("Leave the huddle in {} first.", huddle.label).into());
                cx.notify();
            }
            return;
        }
        if !self.slack_workspace_api_capabilities.join_huddles || conversation_id.is_empty() {
            return;
        }
        let label: SharedString = if workspace.channel_kind.is_channel() {
            format!("#{}", workspace.channel_name).into()
        } else {
            workspace.channel_name.clone().into()
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_huddle_error = Some("Connect to Slack to start a huddle.".into());
            cx.notify();
            return;
        };
        self.slack_huddle_generation = self.slack_huddle_generation.wrapping_add(1);
        let generation = self.slack_huddle_generation;
        self.slack_huddle_error = None;
        self.slack_huddle = Some(SlackHuddleState {
            generation,
            conversation_id: conversation_id.clone(),
            label,
            status: SlackHuddleStatus::Joining,
            muted: false,
            participants: 0,
            receipt: None,
            session: None,
        });
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, conversation_id),
            cx,
            |(workspace_api, conversation_id)| {
                workspace_api
                    .join_slack_huddle(SlackHuddleJoinRequest::for_conversation(conversation_id))
            },
            move |this, result, cx| this.finish_slack_huddle_join(generation, result, cx),
        );
    }

    fn finish_slack_huddle_join(
        &mut self,
        generation: u64,
        result: Result<SlackHuddleJoinReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .slack_huddle
            .as_ref()
            .is_some_and(|huddle| huddle.generation == generation);
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => {
                if current {
                    self.slack_huddle = None;
                    self.slack_huddle_error =
                        Some(format!("Couldn’t start the huddle. {error}").into());
                    cx.notify();
                }
                return;
            }
        };
        if !current {
            // Left before Slack answered: tell Slack we are gone.
            let conversation_id = receipt.conversation_id.clone().unwrap_or_default();
            self.send_slack_huddle_leave(receipt, &conversation_id, cx);
            return;
        }
        let (events, event_receiver) = async_channel::unbounded();
        let media = &receipt.media;
        let config = HuddleMediaConfig {
            signaling_url: media.meeting.media_placement.signaling_url.clone(),
            audio_host_url: media.meeting.media_placement.audio_host_url.clone(),
            attendee_id: media.attendee.attendee_id.clone(),
            join_token: media.attendee.join_token.clone(),
            muted: false,
        };
        let session = match HuddleSession::start(config, events) {
            Ok(session) => session,
            Err(error) => {
                let conversation_id = self
                    .slack_huddle
                    .as_ref()
                    .map(|huddle| huddle.conversation_id.clone())
                    .unwrap_or_default();
                self.finish_slack_huddle(generation, Some(error), cx);
                self.send_slack_huddle_leave(receipt, &conversation_id, cx);
                return;
            }
        };
        if let Some(huddle) = self.slack_huddle.as_mut() {
            huddle.status = SlackHuddleStatus::Connecting;
            huddle.receipt = Some(receipt);
            huddle.session = Some(session);
        }
        cx.spawn(async move |this, cx| {
            while let Ok(event) = event_receiver.recv().await {
                let applied = this.update(cx, |this, cx| {
                    this.apply_slack_huddle_event(generation, event, cx);
                });
                if applied.is_err() {
                    return;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn apply_slack_huddle_event(
        &mut self,
        generation: u64,
        event: HuddleEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(huddle) = self
            .slack_huddle
            .as_mut()
            .filter(|huddle| huddle.generation == generation)
        else {
            return;
        };
        match event {
            HuddleEvent::Connected => huddle.status = SlackHuddleStatus::Connected,
            HuddleEvent::Muted(muted) => huddle.muted = muted,
            HuddleEvent::Participants(people) => huddle.participants = people.len(),
            HuddleEvent::Ended => self.finish_slack_huddle(generation, None, cx),
            HuddleEvent::Failed(error) => {
                self.finish_slack_huddle(
                    generation,
                    Some(format!("The huddle ended: {error}")),
                    cx,
                );
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_slack_huddle_mute(&mut self, cx: &mut Context<Self>) {
        let Some(huddle) = self.slack_huddle.as_mut() else {
            return;
        };
        if let Some(session) = huddle.session.as_ref() {
            // The session reports the new state back, which updates the bar.
            session.set_muted(!huddle.muted);
        }
        cx.notify();
    }

    pub(crate) fn leave_slack_huddle(&mut self, cx: &mut Context<Self>) {
        let Some(huddle) = self.slack_huddle.as_mut() else {
            return;
        };
        match huddle.session.as_ref() {
            // The session says goodbye to Chime, then reports Ended.
            Some(session) => {
                session.leave();
                huddle.status = SlackHuddleStatus::Leaving;
            }
            // Still joining: drop it now; the join's answer is left behind.
            None => {
                let generation = huddle.generation;
                self.finish_slack_huddle(generation, None, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn dismiss_slack_huddle_error(&mut self, cx: &mut Context<Self>) {
        if self.slack_huddle_error.take().is_some() {
            cx.notify();
        }
    }

    /// Clears the huddle, tells Slack it was left, and closes the audio off
    /// the UI thread (closing waits briefly for Chime's goodbye).
    fn finish_slack_huddle(
        &mut self,
        generation: u64,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(huddle) = self
            .slack_huddle
            .take_if(|huddle| huddle.generation == generation)
        else {
            return;
        };
        if let Some(session) = huddle.session {
            cx.background_executor()
                .spawn(async move { drop(session) })
                .detach();
        }
        if let Some(receipt) = huddle.receipt {
            self.send_slack_huddle_leave(receipt, &huddle.conversation_id, cx);
        }
        self.slack_huddle_error = error.map(SharedString::from);
        cx.notify();
    }

    /// `conversation_id` stands in when Slack's answer named no channel.
    fn send_slack_huddle_leave(
        &mut self,
        receipt: SlackHuddleJoinReceipt,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let Some(conversation_id) = receipt
            .conversation_id
            .clone()
            .or_else(|| (!conversation_id.is_empty()).then(|| conversation_id.to_string()))
        else {
            return;
        };
        let request = SlackHuddleLeaveRequest {
            conversation_id,
            call_id: receipt.call_id,
            attendee_id: receipt.media.attendee.attendee_id,
            reason: SLACK_HUDDLE_LEAVE_REASON.to_string(),
        };
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| workspace_api.leave_slack_huddle(request),
            |this, result, cx| {
                if let Err(error) = result {
                    this.slack_huddle_error =
                        Some(format!("Slack may still show you in the huddle: {error}").into());
                    cx.notify();
                }
            },
        );
    }
}
