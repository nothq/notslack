use std::time::Instant;

use super::{PreparedSlackConversationSnapshot, SlackConversationLoadRoute, SlackMessageTimestamp};

#[derive(Clone)]
pub(crate) struct SlackConversationLoadProfile {
    pub(crate) conversation_id: String,
    pub(crate) clicked_at: Instant,
    pub(crate) worker_started_at: Option<Instant>,
    pub(crate) workspace_loaded_at: Option<Instant>,
    pub(crate) prepared_at: Option<Instant>,
    pub(crate) foreground_started_at: Option<Instant>,
    pub(crate) applied_at: Option<Instant>,
    pub(crate) first_render_scheduled_at: Option<Instant>,
}

impl SlackConversationLoadProfile {
    pub(crate) fn new(conversation_id: String, clicked_at: Instant) -> Self {
        Self {
            conversation_id,
            clicked_at,
            worker_started_at: None,
            workspace_loaded_at: None,
            prepared_at: None,
            foreground_started_at: None,
            applied_at: None,
            first_render_scheduled_at: None,
        }
    }
}

pub(crate) struct SlackActivationLoadProfile {
    pub(crate) activated_at: Instant,
    pub(crate) surface_construction_started_at: Instant,
    pub(crate) surface_ready_at: Instant,
    pub(crate) surface_created: bool,
    pub(crate) conversation_id: Option<String>,
    pub(crate) pending_frame_scheduled_at: Option<Instant>,
    pub(crate) pending_frame_recorded: bool,
    pub(crate) first_relevant_frame_scheduled_at: Option<Instant>,
    pub(crate) first_relevant_frame_recorded: bool,
    pub(crate) completed_stage_mask: u8,
    pub(crate) pending_stage_frames: Vec<SlackActivationStageFrame>,
    pub(crate) frame_callback_scheduled: bool,
    pub(crate) live_workspace_applied_recorded: bool,
    pub(crate) terminal: bool,
}

pub(crate) struct SlackActivationStageFrame {
    pub(crate) stage: &'static str,
    pub(crate) state: &'static str,
    pub(crate) applied_at: Instant,
}

impl SlackActivationLoadProfile {
    pub(crate) fn new(
        activated_at: Instant,
        surface_construction_started_at: Instant,
        surface_ready_at: Instant,
        surface_created: bool,
    ) -> Self {
        Self {
            activated_at,
            surface_construction_started_at,
            surface_ready_at,
            surface_created,
            conversation_id: None,
            pending_frame_scheduled_at: None,
            pending_frame_recorded: false,
            first_relevant_frame_scheduled_at: None,
            first_relevant_frame_recorded: false,
            completed_stage_mask: 0,
            pending_stage_frames: Vec::with_capacity(4),
            frame_callback_scheduled: false,
            live_workspace_applied_recorded: false,
            terminal: false,
        }
    }
}

pub(crate) struct SlackConversationLoadResult {
    pub(crate) result: Result<PreparedSlackConversationSnapshot, String>,
    pub(crate) anchor_timestamp: Option<SlackMessageTimestamp>,
    pub(crate) route: SlackConversationLoadRoute,
    pub(crate) worker_started_at: Instant,
    pub(crate) workspace_loaded_at: Instant,
    pub(crate) prepared_at: Option<Instant>,
}
