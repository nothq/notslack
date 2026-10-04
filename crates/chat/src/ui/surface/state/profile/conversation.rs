use std::time::Instant;

use super::super::{Context, SlackConversationLoadResult, SurfaceState, Window};
use super::logging::{log_slack_conversation_apply_profile, log_slack_conversation_load_profile};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn clear_slack_conversation_load_profile(&mut self) {
        self.slack_conversation_load_profile = None;
    }

    pub(in crate::ui::surface::state) fn record_slack_conversation_load_result(
        &mut self,
        conversation_id: &str,
        result: &SlackConversationLoadResult,
    ) {
        let Some(profile) = self.slack_conversation_load_profile.as_mut() else {
            return;
        };
        if profile.conversation_id != conversation_id {
            return;
        }
        profile.worker_started_at = Some(result.worker_started_at);
        profile.workspace_loaded_at = Some(result.workspace_loaded_at);
        profile.prepared_at = result.prepared_at;
        profile.foreground_started_at = Some(Instant::now());
    }

    pub(in crate::ui::surface::state) fn record_slack_conversation_applied(
        &mut self,
        conversation_id: &str,
    ) {
        let Some(profile) = self.slack_conversation_load_profile.as_mut() else {
            return;
        };
        if profile.conversation_id == conversation_id {
            let applied_at = Instant::now();
            profile.applied_at = Some(applied_at);
            log_slack_conversation_apply_profile(profile, applied_at);
        }
    }

    pub(crate) fn schedule_slack_conversation_load_profile_finish(
        &mut self,
        conversation_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.slack_conversation_load_profile.as_mut() else {
            return;
        };
        if profile.conversation_id != conversation_id
            || profile.applied_at.is_none()
            || profile.first_render_scheduled_at.is_some()
        {
            return;
        }
        profile.first_render_scheduled_at = Some(Instant::now());
        let conversation_id = conversation_id.to_string();
        cx.on_next_frame(window, move |this, _window, _cx| {
            this.finish_slack_conversation_load_profile(&conversation_id, Instant::now());
        });
    }

    fn finish_slack_conversation_load_profile(
        &mut self,
        conversation_id: &str,
        finished_at: Instant,
    ) {
        let Some(profile) = self.slack_conversation_load_profile.take() else {
            return;
        };
        if profile.conversation_id == conversation_id {
            log_slack_conversation_load_profile(profile, finished_at);
        }
    }
}
