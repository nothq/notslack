mod conversation;
mod logging;

use super::{
    Context, SlackActivationLoadProfile, SlackActivationStageFrame, SlackConversationLoadProfile,
    SlackInitialRefreshIdentity, SlackSurfaceActivationTiming, SurfaceState, Window,
};
use logging::{
    format_duration, log_slack_activation_live_stage, log_slack_activation_stage,
    slack_activation_conversation_id, slack_activation_stage_bit,
};
use std::time::Instant;

const SLACK_ACTIVATION_ALL_STAGES: u8 = 0b1111;

impl SurfaceState {
    pub(super) fn slack_load_profiling_enabled() -> bool {
        false
    }

    pub(super) fn start_slack_conversation_load_profile(&mut self, conversation_id: &str) {
        if !Self::slack_load_profiling_enabled() {
            self.slack_conversation_load_profile = None;
            return;
        }
        self.slack_conversation_load_profile = Some(SlackConversationLoadProfile::new(
            conversation_id.to_string(),
            Instant::now(),
        ));
    }

    pub(super) fn start_slack_activation_load_profile(
        &mut self,
        activation: SlackSurfaceActivationTiming,
    ) {
        if !Self::slack_load_profiling_enabled() {
            self.slack_activation_load_profile = None;
            return;
        }
        let profile = SlackActivationLoadProfile::new(
            activation.activated_at,
            activation.surface_construction_started_at,
            activation.surface_ready_at,
            activation.surface_created,
        );
        log_slack_activation_stage(&profile, "activation", profile.activated_at);
        eprintln!(
            "[notslack-slack-activation-profile] stage=surface_ready conversation=n/a elapsed={} activation_to_construction={} surface_construction={} surface_created={}",
            format_duration(profile.surface_ready_at.duration_since(profile.activated_at)),
            format_duration(
                profile
                    .surface_construction_started_at
                    .duration_since(profile.activated_at)
            ),
            format_duration(
                profile
                    .surface_ready_at
                    .duration_since(profile.surface_construction_started_at)
            ),
            profile.surface_created,
        );
        self.slack_activation_load_profile = Some(profile);
    }

    pub(super) fn record_slack_activation_connection_started(&mut self) {
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        log_slack_activation_stage(profile, "connection_started", Instant::now());
    }

    pub(super) fn record_slack_activation_shell_applied(&mut self, conversation_id: &str) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        profile.conversation_id = Some(conversation_id.to_string());
        log_slack_activation_stage(profile, "shell_applied", Instant::now());
    }

    pub(super) fn record_slack_activation_initial_refresh_started(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        if profile.conversation_id.as_deref() != Some(identity.conversation_id()) {
            return;
        }
        eprintln!(
            "[notslack-slack-activation-profile] stage=initial_refresh_started conversation={} generation={} elapsed={}",
            identity.conversation_id(),
            identity.generation(),
            format_duration(Instant::now().duration_since(profile.activated_at)),
        );
    }

    pub(super) fn record_slack_activation_initial_cache_stage(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
        cache: &'static str,
        outcome: &'static str,
        completed_at: Instant,
    ) {
        let applied_at = Instant::now();
        if let Some(profile) = self
            .slack_activation_load_profile
            .as_ref()
            .filter(|profile| {
                profile.conversation_id.as_deref() == Some(identity.conversation_id())
            })
        {
            eprintln!(
                "[notslack-slack-activation-profile] stage=cache_{}_{} conversation={} generation={} elapsed={} worker_to_foreground={}",
                cache,
                outcome,
                identity.conversation_id(),
                identity.generation(),
                format_duration(applied_at.duration_since(profile.activated_at)),
                format_duration(applied_at.duration_since(completed_at)),
            );
        } else {
            eprintln!(
                "[notslack-slack-initial-cache] cache={} outcome={} team={} conversation={} generation={} worker_to_foreground={}",
                cache,
                outcome,
                identity.team_id(),
                identity.conversation_id(),
                identity.generation(),
                format_duration(applied_at.duration_since(completed_at)),
            );
        }
    }

    pub(super) fn report_stale_initial_slack_refresh(
        &self,
        identity: &SlackInitialRefreshIdentity,
        stage: &'static str,
    ) {
        if Self::slack_load_profiling_enabled() {
            eprintln!(
                "[notslack-slack-activation-profile] stage={}_stale team={} conversation={} generation={}",
                stage,
                identity.team_id(),
                identity.conversation_id(),
                identity.generation(),
            );
        }
    }

    pub(super) fn record_slack_activation_live_workspace_preparation_started(
        &mut self,
        conversation_id: &str,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        if profile.conversation_id.as_deref() == Some(conversation_id) {
            log_slack_activation_stage(
                profile,
                "live_workspace_preparation_started",
                Instant::now(),
            );
        }
    }

    pub(super) fn record_slack_activation_live_workspace_prepared(
        &mut self,
        conversation_id: &str,
        prepared_at: Instant,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        if profile.conversation_id.as_deref() == Some(conversation_id) {
            log_slack_activation_stage(profile, "live_workspace_prepared", prepared_at);
        }
    }

    pub(super) fn record_slack_activation_live_stage_prepared(
        &mut self,
        conversation_id: &str,
        stage: &'static str,
        prepared_at: Option<Instant>,
    ) {
        let Some(prepared_at) = prepared_at else {
            return;
        };
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        if profile.conversation_id.as_deref() == Some(conversation_id) {
            log_slack_activation_live_stage(profile, stage, "prepared", prepared_at);
        }
    }

    pub(super) fn record_slack_activation_live_stage_applied(
        &mut self,
        conversation_id: &str,
        stage: &'static str,
    ) {
        self.record_slack_activation_live_stage_completed(conversation_id, stage, "applied");
    }

    pub(super) fn record_slack_activation_live_stage_failed(
        &mut self,
        conversation_id: &str,
        stage: &'static str,
    ) {
        self.record_slack_activation_live_stage_completed(conversation_id, stage, "failed");
    }

    pub(super) fn record_slack_activation_live_workspace_applied(&mut self, conversation_id: &str) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        if profile.conversation_id.as_deref() != Some(conversation_id)
            || profile.live_workspace_applied_recorded
        {
            return;
        }
        profile.live_workspace_applied_recorded = true;
        log_slack_activation_stage(profile, "live_workspace_applied", Instant::now());
    }

    pub(super) fn record_slack_activation_live_workspace_failed(&mut self, conversation_id: &str) {
        let Some(profile) = self.slack_activation_load_profile.as_ref() else {
            return;
        };
        if profile.conversation_id.as_deref() == Some(conversation_id) {
            log_slack_activation_stage(profile, "live_workspace_failed", Instant::now());
        }
    }

    pub(super) fn record_slack_activation_connection_failed(&mut self, stage: &'static str) {
        let Some(profile) = self.slack_activation_load_profile.take() else {
            return;
        };
        log_slack_activation_stage(&profile, stage, Instant::now());
    }

    pub(super) fn schedule_slack_activation_pending_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        if profile.pending_frame_scheduled_at.is_some() {
            return;
        }
        profile.pending_frame_scheduled_at = Some(Instant::now());
        self.schedule_slack_activation_next_frame_callback(window, cx);
    }

    pub(super) fn schedule_slack_activation_load_profile_finish(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        let schedule_first_relevant_frame = profile.first_relevant_frame_scheduled_at.is_none();
        let schedule_stage_frames = !profile.pending_stage_frames.is_empty();
        if !schedule_first_relevant_frame && !schedule_stage_frames {
            return;
        }
        if schedule_first_relevant_frame {
            profile.first_relevant_frame_scheduled_at = Some(Instant::now());
        }
        self.schedule_slack_activation_next_frame_callback(window, cx);
    }

    fn schedule_slack_activation_next_frame_callback(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        if profile.frame_callback_scheduled {
            return;
        }
        profile.frame_callback_scheduled = true;
        cx.on_next_frame(window, |this, _window, _cx| {
            this.record_slack_activation_next_frame(Instant::now());
        });
    }

    fn record_slack_activation_next_frame(&mut self, rendered_at: Instant) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        profile.frame_callback_scheduled = false;
        if !profile.pending_frame_recorded {
            if let Some(scheduled_at) = profile.pending_frame_scheduled_at {
                eprintln!(
                    "[notslack-slack-activation-profile] stage=pending_frame conversation={} elapsed={} scheduled_to_next_frame={} activation_to_construction={} surface_construction={}",
                    slack_activation_conversation_id(profile),
                    format_duration(rendered_at.duration_since(profile.activated_at)),
                    format_duration(rendered_at.duration_since(scheduled_at)),
                    format_duration(
                        profile
                            .surface_construction_started_at
                            .duration_since(profile.activated_at)
                    ),
                    format_duration(
                        profile
                            .surface_ready_at
                            .duration_since(profile.surface_construction_started_at)
                    ),
                );
                profile.pending_frame_recorded = true;
            }
        }
        if !profile.first_relevant_frame_recorded {
            if let Some(scheduled_at) = profile.first_relevant_frame_scheduled_at {
                eprintln!(
                    "[notslack-slack-activation-profile] stage=first_relevant_frame conversation={} elapsed={} scheduled_to_next_frame={}",
                    slack_activation_conversation_id(profile),
                    format_duration(rendered_at.duration_since(profile.activated_at)),
                    format_duration(rendered_at.duration_since(scheduled_at)),
                );
                profile.first_relevant_frame_recorded = true;
            }
        }
        for stage_frame in std::mem::take(&mut profile.pending_stage_frames) {
            eprintln!(
                "[notslack-slack-activation-profile] stage=live_{}_{}_next_frame conversation={} elapsed={} applied_to_next_frame={}",
                stage_frame.stage,
                stage_frame.state,
                slack_activation_conversation_id(profile),
                format_duration(rendered_at.duration_since(profile.activated_at)),
                format_duration(rendered_at.duration_since(stage_frame.applied_at)),
            );
        }
        self.finish_slack_activation_profile_if_complete();
    }

    fn record_slack_activation_live_stage_completed(
        &mut self,
        conversation_id: &str,
        stage: &'static str,
        state: &'static str,
    ) {
        let Some(profile) = self.slack_activation_load_profile.as_mut() else {
            return;
        };
        if profile.conversation_id.as_deref() != Some(conversation_id) {
            return;
        }
        let stage_bit = slack_activation_stage_bit(stage);
        if profile.completed_stage_mask & stage_bit != 0 {
            return;
        }
        let completed_at = Instant::now();
        log_slack_activation_live_stage(profile, stage, state, completed_at);
        profile.completed_stage_mask |= stage_bit;
        profile
            .pending_stage_frames
            .push(SlackActivationStageFrame {
                stage,
                state,
                applied_at: completed_at,
            });
        if profile.completed_stage_mask == SLACK_ACTIVATION_ALL_STAGES {
            log_slack_activation_stage(profile, "all_live_stages_complete", completed_at);
            profile.terminal = true;
        }
    }

    fn finish_slack_activation_profile_if_complete(&mut self) {
        if self
            .slack_activation_load_profile
            .as_ref()
            .is_some_and(|profile| {
                profile.terminal
                    && profile.first_relevant_frame_recorded
                    && (profile.pending_frame_scheduled_at.is_none()
                        || profile.pending_frame_recorded)
                    && profile.pending_stage_frames.is_empty()
                    && !profile.frame_callback_scheduled
            })
        {
            self.slack_activation_load_profile = None;
        }
    }
}
