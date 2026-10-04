mod host;
mod operations;
mod slot;
mod sources;

pub use host::{SlackHostEvent, SlackHostEventStream, SlackHostRuntime};
use slot::{SlackWorkspaceRuntimeSlot, SlackWorkspaceRuntimeSnapshot};
use sources::{SlackNotificationPolicyState, SlackRealtimePresentationEvent};

use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
};

use crate::model::{
    SlackMessageClientId, SlackMessageDraft, SlackMessageSendReceipt, SlackMessageTimestamp,
    SlackRealtimeSubscription, SlackThreadReplyReceipt, SlackWorkspaceConnectRequest,
    SlackWorkspaceConnection,
};

use super::{
    slack_auth::{
        authenticated_slack_runtime_inputs, available_slack_runtime_inputs,
        recapture_slack_runtime_input, should_recapture_slack_desktop_session,
        SlackAuthenticatedRuntimeInput, SlackAuthenticatedTeam, SlackDesktopIntegrationStatus,
        SlackDesktopIntegrationUnavailable, SlackNotificationAudioPreferences,
    },
    SlackWorkspaceRuntime,
};

type SlackWorkspaceRuntimeSlots = HashMap<String, Arc<SlackWorkspaceRuntimeSlot>>;
type SlackNotificationReadReceiptReceiver = mpsc::Receiver<SlackNotificationReadReceipt>;

pub(crate) struct SlackWorkspaceRuntimeRegistry {
    integration_status: SlackDesktopIntegrationStatus,
    recovery_policy: SlackRuntimeRecoveryPolicy,
    ordered_teams: Arc<[SlackAuthenticatedTeam]>,
    runtimes: Arc<SlackWorkspaceRuntimeSlots>,
    notification_read_receipt_sender: mpsc::SyncSender<SlackNotificationReadReceipt>,
    notification_read_receipts: Arc<Mutex<Option<SlackNotificationReadReceiptReceiver>>>,
}

#[derive(Clone, Copy)]
enum SlackRuntimeRecoveryPolicy {
    CachedSessionsOnly,
    RecaptureDesktopSession,
}

const NOTIFICATION_READ_RECEIPT_CAPACITY: usize = 128;

#[derive(Clone)]
pub enum SlackNotificationReadReceipt {
    Conversation(crate::model::SlackConversationReadReceipt),
    Thread {
        team_id: String,
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        read_through: SlackMessageTimestamp,
    },
}

struct SlackTeamRealtimeSource {
    team_id: String,
    slot: Arc<SlackWorkspaceRuntimeSlot>,
    generation: u64,
    runtime: Arc<SlackWorkspaceRuntime>,
    subscription: Box<dyn SlackRealtimeSubscription>,
    replacement_generation: tokio::sync::watch::Receiver<u64>,
}

#[derive(Clone)]
struct SlackTeamNotificationPolicySource {
    team_id: String,
    slot: Arc<SlackWorkspaceRuntimeSlot>,
}

#[derive(Clone)]
pub struct SlackNotificationReplyRequest {
    pub team_id: String,
    pub conversation_id: String,
    pub thread_timestamp: Option<SlackMessageTimestamp>,
    pub client_message_id: SlackMessageClientId,
    pub draft: SlackMessageDraft,
}

enum SlackNotificationReplyReceipt {
    Conversation(SlackMessageSendReceipt),
    Thread(SlackThreadReplyReceipt),
}

impl SlackWorkspaceRuntimeRegistry {
    fn load_authenticated() -> Result<Self, String> {
        Self::from_inputs(
            authenticated_slack_runtime_inputs()?,
            SlackRuntimeRecoveryPolicy::RecaptureDesktopSession,
        )
    }

    fn load_available() -> Result<Self, SlackDesktopIntegrationUnavailable> {
        let inputs = available_slack_runtime_inputs()?;
        Self::from_inputs(inputs, SlackRuntimeRecoveryPolicy::CachedSessionsOnly)
            .map_err(SlackDesktopIntegrationUnavailable::RuntimeError)
    }

    fn unavailable(reason: SlackDesktopIntegrationUnavailable) -> Self {
        let (notification_read_receipts, notification_read_receipt_receiver) =
            mpsc::sync_channel(NOTIFICATION_READ_RECEIPT_CAPACITY);
        Self {
            integration_status: SlackDesktopIntegrationStatus::Unavailable(reason),
            recovery_policy: SlackRuntimeRecoveryPolicy::CachedSessionsOnly,
            ordered_teams: Vec::new().into(),
            runtimes: Arc::new(HashMap::new()),
            notification_read_receipt_sender: notification_read_receipts,
            notification_read_receipts: Arc::new(Mutex::new(Some(
                notification_read_receipt_receiver,
            ))),
        }
    }

    fn from_inputs(
        inputs: Vec<SlackAuthenticatedRuntimeInput>,
        recovery_policy: SlackRuntimeRecoveryPolicy,
    ) -> Result<Self, String> {
        let (notification_read_receipts, notification_read_receipt_receiver) =
            mpsc::sync_channel(NOTIFICATION_READ_RECEIPT_CAPACITY);
        let mut ordered_teams = Vec::with_capacity(inputs.len());
        let mut runtimes = HashMap::with_capacity(inputs.len());
        for input in inputs {
            let team = input.team.clone();
            let team_id = team.team_id.clone();
            let slot = Arc::new(SlackWorkspaceRuntimeSlot::new(
                input,
                notification_read_receipts.clone(),
            )?);
            if runtimes.insert(team_id.clone(), slot).is_some() {
                return Err(format!(
                    "authenticated Slack runtime inventory repeated team {team_id}"
                ));
            }
            ordered_teams.push(team);
        }
        Ok(Self {
            integration_status: SlackDesktopIntegrationStatus::Available,
            recovery_policy,
            ordered_teams: ordered_teams.into(),
            runtimes: Arc::new(runtimes),
            notification_read_receipt_sender: notification_read_receipts,
            notification_read_receipts: Arc::new(Mutex::new(Some(
                notification_read_receipt_receiver,
            ))),
        })
    }

    pub(super) fn authenticated_teams(&self) -> &[SlackAuthenticatedTeam] {
        &self.ordered_teams
    }

    pub(super) fn integration_status(&self) -> &SlackDesktopIntegrationStatus {
        &self.integration_status
    }

    pub(super) fn contains_team(&self, team_id: &str) -> bool {
        self.runtimes.contains_key(team_id)
    }

    pub(super) fn connection_for_team(
        &self,
        request: &SlackWorkspaceConnectRequest,
    ) -> Result<SlackWorkspaceConnection, String> {
        let team_id = request.team_id();
        let slot = self.runtime_slot(team_id.as_str())?;
        let snapshot = slot.snapshot()?;
        match (
            self.recovery_policy,
            connection_for_runtime(request, snapshot.runtime.clone(), true),
        ) {
            (_, Ok(connection)) => Ok(connection),
            (SlackRuntimeRecoveryPolicy::RecaptureDesktopSession, Err(error))
                if should_recapture_slack_desktop_session(&error) =>
            {
                let recovered =
                    self.recover_runtime(&slot, snapshot.generation)
                        .map_err(|retry_error| {
                            format!("{error}; Slack Desktop recapture failed: {retry_error}")
                        })?;
                connection_for_runtime(request, recovered.runtime, false).map_err(|retry_error| {
                    format!("{error}; Slack Desktop recapture failed: {retry_error}")
                })
            }
            (_, Err(error)) => Err(error),
        }
    }

    fn recover_runtime(
        &self,
        slot: &SlackWorkspaceRuntimeSlot,
        failed_generation: u64,
    ) -> Result<SlackWorkspaceRuntimeSnapshot, String> {
        let _recovery = slot
            .recovery
            .lock()
            .map_err(|_| "Slack workspace recovery mutex poisoned".to_string())?;
        let current = slot.snapshot()?;
        if current.generation != failed_generation {
            return Ok(current);
        }
        let input = recapture_slack_runtime_input(&slot.team_id)?;
        let notification_audio_preferences = SlackNotificationAudioPreferences {
            playback: input.team.notification_playback,
            sound: input.team.notification_sound,
        };
        let runtime = slot::runtime_from_input(input, &self.notification_read_receipt_sender)?;
        slot.replace(failed_generation, runtime, notification_audio_preferences)
    }

    fn runtime_slot(&self, team_id: &str) -> Result<Arc<SlackWorkspaceRuntimeSlot>, String> {
        self.runtimes
            .get(team_id)
            .cloned()
            .ok_or_else(|| format!("Slack Desktop has no authenticated runtime for team {team_id}"))
    }

    fn runtime(&self, team_id: &str) -> Result<Arc<SlackWorkspaceRuntime>, String> {
        self.runtime_slot(team_id)?
            .snapshot()
            .map(|state| state.runtime)
    }

    fn notification_audio_preferences(
        &self,
        team_id: &str,
    ) -> Result<SlackNotificationAudioPreferences, String> {
        self.runtime_slot(team_id)?
            .snapshot()
            .map(|state| state.notification_audio_preferences)
    }
}

fn connection_for_runtime(
    request: &SlackWorkspaceConnectRequest,
    runtime: Arc<SlackWorkspaceRuntime>,
    validate_authentication: bool,
) -> Result<SlackWorkspaceConnection, String> {
    let team_id = request.team_id();
    if validate_authentication {
        runtime.validate_authentication()?;
    }
    let (conversation_id, initial_message_anchor) = match request.launch_route() {
        Some(route) => (
            route.channel_id().as_str().to_string(),
            route
                .tab_id()
                .map(|tab_id| SlackMessageTimestamp::parse(tab_id.as_str()))
                .transpose()?,
        ),
        None => (runtime.resolve_startup_conversation_id()?, None),
    };
    let shell = runtime.startup_shell(&conversation_id);
    if shell.team_id != team_id.as_str() {
        return Err(format!(
            "Slack runtime for team {} returned a shell for team {}",
            team_id.as_str(),
            shell.team_id
        ));
    }
    Ok(SlackWorkspaceConnection {
        team_id: team_id.clone(),
        shell: Box::new(shell),
        workspace_api: runtime,
        initial_message_anchor,
    })
}
