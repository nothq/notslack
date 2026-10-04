use std::time::Duration;

use super::{Instant, SlackActivationLoadProfile, SlackConversationLoadProfile};

pub(in crate::ui::surface::state) fn log_slack_activation_live_stage(
    profile: &SlackActivationLoadProfile,
    data_stage: &'static str,
    state: &'static str,
    recorded_at: Instant,
) {
    eprintln!(
        "[notslack-slack-activation-profile] stage=live_{}_{} conversation={} elapsed={}",
        data_stage,
        state,
        slack_activation_conversation_id(profile),
        format_duration(recorded_at.duration_since(profile.activated_at)),
    );
}

pub(in crate::ui::surface::state) fn slack_activation_stage_bit(stage: &str) -> u8 {
    match stage {
        "shell" => 0b001,
        "sidebar" => 0b010,
        "conversation" => 0b100,
        "dms" => 0b1000,
        _ => panic!("unknown Slack activation stage {stage}"),
    }
}

pub(in crate::ui::surface::state) fn log_slack_activation_stage(
    profile: &SlackActivationLoadProfile,
    stage: &'static str,
    recorded_at: Instant,
) {
    eprintln!(
        "[notslack-slack-activation-profile] stage={} conversation={} elapsed={}",
        stage,
        slack_activation_conversation_id(profile),
        format_duration(recorded_at.duration_since(profile.activated_at)),
    );
}

pub(in crate::ui::surface::state) fn slack_activation_conversation_id(
    profile: &SlackActivationLoadProfile,
) -> &str {
    profile.conversation_id.as_deref().unwrap_or("n/a")
}

pub(in crate::ui::surface::state) fn log_slack_conversation_apply_profile(
    profile: &SlackConversationLoadProfile,
    applied_at: Instant,
) {
    eprintln!(
        "[notslack-slack-load-applied] conversation={} total_to_apply={} click_to_worker={} workspace_load={} prepare={} foreground_apply={}",
        profile.conversation_id,
        format_duration(applied_at.duration_since(profile.clicked_at)),
        format_optional_duration(
            profile
                .worker_started_at
                .map(|worker_started_at| worker_started_at.duration_since(profile.clicked_at))
        ),
        format_optional_duration(
            match (profile.worker_started_at, profile.workspace_loaded_at) {
                (Some(worker_started_at), Some(workspace_loaded_at)) => {
                    Some(workspace_loaded_at.duration_since(worker_started_at))
                }
                _ => None,
            }
        ),
        format_optional_duration(match (profile.workspace_loaded_at, profile.prepared_at) {
            (Some(workspace_loaded_at), Some(prepared_at)) => {
                Some(prepared_at.duration_since(workspace_loaded_at))
            }
            _ => None,
        }),
        format_optional_duration(
            profile
                .foreground_started_at
                .map(|foreground_started_at| applied_at.duration_since(foreground_started_at))
        )
    );
}

pub(in crate::ui::surface::state) fn log_slack_conversation_load_profile(
    profile: SlackConversationLoadProfile,
    finished_at: Instant,
) {
    let total_to_apply = profile
        .applied_at
        .map(|applied_at| applied_at.duration_since(profile.clicked_at));
    let total_to_next_frame = finished_at.duration_since(profile.clicked_at);
    eprintln!(
        "[notslack-slack-load-profile] conversation={} total_to_next_frame={} total_to_apply={} click_to_worker={} workspace_load={} prepare={} foreground_apply={} apply_to_render_schedule={} render_schedule_to_next_frame={}",
        profile.conversation_id,
        format_duration(total_to_next_frame),
        format_optional_duration(total_to_apply),
        format_optional_duration(
            profile
                .worker_started_at
                .map(|worker_started_at| worker_started_at.duration_since(profile.clicked_at))
        ),
        format_optional_duration(
            match (profile.worker_started_at, profile.workspace_loaded_at) {
                (Some(worker_started_at), Some(workspace_loaded_at)) => {
                    Some(workspace_loaded_at.duration_since(worker_started_at))
                }
                _ => None,
            }
        ),
        format_optional_duration(match (profile.workspace_loaded_at, profile.prepared_at) {
            (Some(workspace_loaded_at), Some(prepared_at)) => {
                Some(prepared_at.duration_since(workspace_loaded_at))
            }
            _ => None,
        }),
        format_optional_duration(match (profile.foreground_started_at, profile.applied_at) {
            (Some(foreground_started_at), Some(applied_at)) => {
                Some(applied_at.duration_since(foreground_started_at))
            }
            _ => None,
        }),
        format_optional_duration(
            match (profile.applied_at, profile.first_render_scheduled_at) {
                (Some(applied_at), Some(first_render_scheduled_at)) => {
                    Some(first_render_scheduled_at.duration_since(applied_at))
                }
                _ => None,
            }
        ),
        format_optional_duration(profile.first_render_scheduled_at.map(
            |first_render_scheduled_at| finished_at.duration_since(first_render_scheduled_at)
        ))
    );
}

pub(in crate::ui::surface::state) fn format_optional_duration(
    duration: Option<Duration>,
) -> String {
    duration
        .map(format_duration)
        .unwrap_or_else(|| "n/a".to_string())
}

pub(in crate::ui::surface::state) fn format_duration(duration: Duration) -> String {
    format!("{:.2}ms", duration.as_secs_f64() * 1_000.0)
}
