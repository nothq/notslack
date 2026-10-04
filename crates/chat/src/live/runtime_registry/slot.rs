use std::sync::{mpsc, Arc, Mutex};

use super::{
    SlackAuthenticatedRuntimeInput, SlackNotificationAudioPreferences,
    SlackNotificationReadReceipt, SlackWorkspaceRuntime,
};
use crate::model::SlackRealtimeSubscription;

pub(super) struct SlackWorkspaceRuntimeSlot {
    pub(super) team_id: String,
    state: Mutex<SlackWorkspaceRuntimeSlotState>,
    pub(super) recovery: Mutex<()>,
    pub(super) replacement_generation: tokio::sync::watch::Sender<u64>,
}

struct SlackWorkspaceRuntimeSlotState {
    generation: u64,
    runtime: Arc<SlackWorkspaceRuntime>,
    notification_audio_preferences: SlackNotificationAudioPreferences,
}

pub(super) struct SlackWorkspaceRuntimeSnapshot {
    pub(super) generation: u64,
    pub(super) runtime: Arc<SlackWorkspaceRuntime>,
    pub(super) notification_audio_preferences: SlackNotificationAudioPreferences,
}

pub(super) struct SlackWorkspaceRealtimeClaim {
    pub(super) generation: u64,
    pub(super) runtime: Arc<SlackWorkspaceRuntime>,
    pub(super) subscription: Box<dyn SlackRealtimeSubscription>,
}

impl SlackWorkspaceRuntimeSlot {
    pub(super) fn new(
        input: SlackAuthenticatedRuntimeInput,
        notification_read_receipts: mpsc::SyncSender<SlackNotificationReadReceipt>,
    ) -> Result<Self, String> {
        let team_id = input.team.team_id.clone();
        let notification_audio_preferences = SlackNotificationAudioPreferences {
            playback: input.team.notification_playback,
            sound: input.team.notification_sound,
        };
        let runtime = runtime_from_input(input, &notification_read_receipts)?;
        let (replacement_generation, _) = tokio::sync::watch::channel(0);
        Ok(Self {
            team_id,
            state: Mutex::new(SlackWorkspaceRuntimeSlotState {
                generation: 0,
                runtime,
                notification_audio_preferences,
            }),
            recovery: Mutex::new(()),
            replacement_generation,
        })
    }

    pub(super) fn snapshot(&self) -> Result<SlackWorkspaceRuntimeSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Slack workspace runtime slot mutex poisoned".to_string())?;
        Ok(SlackWorkspaceRuntimeSnapshot {
            generation: state.generation,
            runtime: state.runtime.clone(),
            notification_audio_preferences: state.notification_audio_preferences,
        })
    }

    pub(super) fn replace(
        &self,
        expected_generation: u64,
        runtime: Arc<SlackWorkspaceRuntime>,
        notification_audio_preferences: SlackNotificationAudioPreferences,
    ) -> Result<SlackWorkspaceRuntimeSnapshot, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Slack workspace runtime slot mutex poisoned".to_string())?;
        if state.generation != expected_generation {
            return Ok(SlackWorkspaceRuntimeSnapshot {
                generation: state.generation,
                runtime: state.runtime.clone(),
                notification_audio_preferences: state.notification_audio_preferences,
            });
        }
        let generation = state
            .generation
            .checked_add(1)
            .ok_or("Slack workspace runtime generation overflowed")?;
        let previous = std::mem::replace(&mut state.runtime, runtime.clone());
        state.generation = generation;
        state.notification_audio_preferences = notification_audio_preferences;
        let snapshot = SlackWorkspaceRuntimeSnapshot {
            generation,
            runtime,
            notification_audio_preferences,
        };
        let retirement_error = previous.retire_for_replacement().err();
        self.replacement_generation.send_replace(generation);
        drop(state);
        match retirement_error {
            Some(error) => Err(format!(
                "failed to retire replaced Slack runtime for team {}: {error}",
                self.team_id
            )),
            None => Ok(snapshot),
        }
    }

    pub(super) fn claim_realtime(
        &self,
        current_generation: Option<u64>,
    ) -> Result<Option<SlackWorkspaceRealtimeClaim>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Slack workspace runtime slot mutex poisoned".to_string())?;
        if current_generation == Some(state.generation) {
            return Ok(None);
        }
        let subscription = state.runtime.claim_app_realtime_subscription()?;
        Ok(Some(SlackWorkspaceRealtimeClaim {
            generation: state.generation,
            runtime: state.runtime.clone(),
            subscription,
        }))
    }
}

pub(super) fn runtime_from_input(
    input: SlackAuthenticatedRuntimeInput,
    notification_read_receipts: &mpsc::SyncSender<SlackNotificationReadReceipt>,
) -> Result<Arc<SlackWorkspaceRuntime>, String> {
    if input.loader.team_id() != input.team.team_id.as_str() {
        return Err(format!(
            "authenticated Slack loader team {} did not match runtime inventory team {}",
            input.loader.team_id(),
            input.team.team_id
        ));
    }
    Ok(Arc::new(SlackWorkspaceRuntime::new(
        input.loader,
        input.team.startup_conversation_id.as_deref(),
        super::super::remember_slack_team_conversation,
        notification_read_receipts.clone(),
    )))
}
