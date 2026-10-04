use super::super::SlackWorkspaceRuntimeRegistry;

#[cfg(any(test, feature = "test-support"))]
use super::{
    loops::{SlackNotificationPolicyWorkerSource, SlackRealtimeWorkerSource},
    workers::SlackHostWorkerStartup,
    InMemorySlackHostParts, SlackHostEventEmitter, SlackHostEventStream, SlackHostRuntime,
};
#[cfg(any(test, feature = "test-support"))]
use crate::{
    live::{
        runtime_registry::{
            sources::SlackNotificationPolicyUpdateOutcome, SlackAuthenticatedRuntimeInput,
            SlackRealtimePresentationEvent, SlackRuntimeRecoveryPolicy,
        },
        SlackAuthenticatedTeam, SlackLiveWorkspaceLoader, SlackWebBuildTimestamp,
        SlackWebSessionCredentials,
    },
    model::SlackNotificationTeamBadge,
};
#[cfg(any(test, feature = "test-support"))]
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
#[cfg(any(test, feature = "test-support"))]
const IN_MEMORY_TEAM_ID: &str = "T_IN_MEMORY";
#[cfg(any(test, feature = "test-support"))]
const WORKER_START_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(any(test, feature = "test-support"))]
const STOP_NOT_REQUESTED: u8 = 0;
#[cfg(any(test, feature = "test-support"))]
const REALTIME_STOPPED: u8 = 1;
#[cfg(any(test, feature = "test-support"))]
const WORKERS_JOINED: u8 = 2;
pub(super) enum SlackHostRealtimeSourceOwner {
    Registry,
    #[cfg(any(test, feature = "test-support"))]
    InMemoryTest(InMemorySlackRealtimeSourceOwner),
}

impl SlackHostRealtimeSourceOwner {
    pub(super) fn require_running(
        &self,
        registry: &SlackWorkspaceRuntimeRegistry,
    ) -> Result<(), String> {
        match self {
            Self::Registry => registry.require_realtime_sources_running(),
            #[cfg(any(test, feature = "test-support"))]
            Self::InMemoryTest(owner) => owner.require_running(),
        }
    }

    pub(super) fn stop(
        &self,
        registry: &SlackWorkspaceRuntimeRegistry,
        host_stop_requested: bool,
    ) -> Result<(), String> {
        match self {
            Self::Registry => {
                #[cfg(not(any(test, feature = "test-support")))]
                let _ = host_stop_requested;
                registry.stop_realtime_sources()
            }
            #[cfg(any(test, feature = "test-support"))]
            Self::InMemoryTest(owner) => owner.stop(host_stop_requested),
        }
    }

    pub(super) fn record_workers_joined(&self) {
        #[cfg(any(test, feature = "test-support"))]
        if let Self::InMemoryTest(owner) = self {
            owner.record_workers_joined();
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(super) struct InMemorySlackHostProbe {
    inner: Arc<InMemorySlackHostProbeInner>,
}

#[cfg(any(test, feature = "test-support"))]
struct InMemorySlackHostProbeInner {
    worker_starts: Mutex<Vec<&'static str>>,
    worker_started: Condvar,
    shutdown: Mutex<Vec<&'static str>>,
    shutdown_phase: AtomicU8,
}

#[cfg(any(test, feature = "test-support"))]
impl InMemorySlackHostProbe {
    fn new() -> Self {
        Self {
            inner: Arc::new(InMemorySlackHostProbeInner {
                worker_starts: Mutex::new(Vec::new()),
                worker_started: Condvar::new(),
                shutdown: Mutex::new(Vec::new()),
                shutdown_phase: AtomicU8::new(STOP_NOT_REQUESTED),
            }),
        }
    }

    fn record_worker_start(&self, worker: &'static str) {
        let mut starts = self
            .inner
            .worker_starts
            .lock()
            .expect("in-memory Slack worker start mutex should not be poisoned");
        if !starts.contains(&worker) {
            starts.push(worker);
        }
        self.inner.worker_started.notify_all();
    }

    fn wait_for_workers(&self) -> Result<(), String> {
        let deadline = Instant::now() + WORKER_START_TIMEOUT;
        let mut starts = self
            .inner
            .worker_starts
            .lock()
            .map_err(|_| "in-memory Slack worker start mutex poisoned".to_string())?;
        while starts.len() < 2 {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let (next, wait) = self
                .inner
                .worker_started
                .wait_timeout(starts, remaining)
                .map_err(|_| "in-memory Slack worker start wait mutex poisoned".to_string())?;
            starts = next;
            if wait.timed_out() {
                break;
            }
        }
        if starts.len() == 2 {
            Ok(())
        } else {
            Err(format!(
                "in-memory Slack workers did not start before timeout: {}",
                starts.join(", ")
            ))
        }
    }

    fn record_shutdown(&self, step: &'static str) {
        self.inner
            .shutdown
            .lock()
            .expect("in-memory Slack shutdown mutex should not be poisoned")
            .push(step);
    }

    #[cfg(test)]
    pub(super) fn worker_starts(&self) -> Vec<&'static str> {
        self.inner
            .worker_starts
            .lock()
            .expect("in-memory Slack worker start mutex should not be poisoned")
            .clone()
    }

    #[cfg(test)]
    pub(super) fn shutdown_steps(&self) -> Vec<&'static str> {
        self.inner
            .shutdown
            .lock()
            .expect("in-memory Slack shutdown mutex should not be poisoned")
            .clone()
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(super) struct InMemorySlackRealtimeSourceOwner {
    keepalive: async_channel::Sender<()>,
    probe: InMemorySlackHostProbe,
}

#[cfg(any(test, feature = "test-support"))]
impl InMemorySlackRealtimeSourceOwner {
    fn require_running(&self) -> Result<(), String> {
        if self.inner_phase() == STOP_NOT_REQUESTED {
            Ok(())
        } else {
            Err("in-memory Slack realtime source is stopped".to_string())
        }
    }

    fn stop(&self, host_stop_requested: bool) -> Result<(), String> {
        if !host_stop_requested {
            return Err("Slack host workers must receive stop before realtime sources".to_string());
        }
        if self
            .probe
            .inner
            .shutdown_phase
            .compare_exchange(
                STOP_NOT_REQUESTED,
                REALTIME_STOPPED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.probe.record_shutdown("request-stop");
            self.probe.record_shutdown("realtime-source-stop");
            self.keepalive.close();
        }
        Ok(())
    }

    fn record_workers_joined(&self) {
        if self
            .probe
            .inner
            .shutdown_phase
            .compare_exchange(
                REALTIME_STOPPED,
                WORKERS_JOINED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.probe.record_shutdown("worker-join");
        }
    }

    fn inner_phase(&self) -> u8 {
        self.probe.inner.shutdown_phase.load(Ordering::Acquire)
    }
}

#[cfg(any(test, feature = "test-support"))]
struct InMemorySlackRealtimeSource {
    team_id: String,
    closed: async_channel::Receiver<()>,
    probe: InMemorySlackHostProbe,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
struct InMemorySlackNotificationPolicySource {
    team_id: String,
    probe: InMemorySlackHostProbe,
}

#[cfg(any(test, feature = "test-support"))]
impl SlackRealtimeWorkerSource for InMemorySlackRealtimeSource {
    type PolicySource = InMemorySlackNotificationPolicySource;

    fn team_id(&self) -> &str {
        &self.team_id
    }

    fn notification_policy_source(&self) -> Self::PolicySource {
        InMemorySlackNotificationPolicySource {
            team_id: self.team_id.clone(),
            probe: self.probe.clone(),
        }
    }

    async fn recv_presentation_event(&mut self) -> SlackRealtimePresentationEvent {
        self.probe.record_worker_start("realtime");
        let _ = self.closed.recv().await;
        SlackRealtimePresentationEvent::Closed
    }

    fn should_deliver_notification(
        &self,
        _notification: &crate::model::SlackRealtimeNotification,
    ) -> Result<bool, String> {
        Ok(true)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl SlackNotificationPolicyWorkerSource for InMemorySlackNotificationPolicySource {
    fn team_id(&self) -> &str {
        &self.team_id
    }

    fn initialize_notification_policy(
        &self,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        Ok(SlackNotificationPolicyUpdateOutcome::Applied(
            SlackNotificationTeamBadge {
                team_id: self.team_id.clone(),
                count: 0,
            },
        ))
    }

    fn refresh_notification_policy(&self) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        Ok(SlackNotificationPolicyUpdateOutcome::Applied(
            SlackNotificationTeamBadge {
                team_id: self.team_id.clone(),
                count: 0,
            },
        ))
    }
    fn notification_policy_event_emitted(&self) {
        self.probe.record_worker_start("notification-policy");
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn in_memory_authenticated_registry() -> Result<SlackWorkspaceRuntimeRegistry, String> {
    let web_session = SlackWebSessionCredentials::new(
        "xoxc-in-memory-test",
        "d=in-memory-test; x=in-memory-test",
        SlackWebBuildTimestamp::parse("1")?,
    )?;
    let loader = SlackLiveWorkspaceLoader::new(IN_MEMORY_TEAM_ID, "in-memory-test", web_session)?;
    let input = SlackAuthenticatedRuntimeInput {
        team: SlackAuthenticatedTeam {
            team_id: IN_MEMORY_TEAM_ID.to_string(),
            workspace_name: "In-memory Slack".to_string(),
            workspace_logo_url: None,
            notification_playback: None,
            notification_sound: None,
            startup_conversation_id: Some("C_IN_MEMORY".to_string()),
            is_default: true,
        },
        loader,
    };
    SlackWorkspaceRuntimeRegistry::from_inputs(
        vec![input],
        SlackRuntimeRecoveryPolicy::CachedSessionsOnly,
    )
}

#[cfg(any(test, feature = "test-support"))]
fn in_memory_source(
    probe: InMemorySlackHostProbe,
) -> (
    InMemorySlackRealtimeSourceOwner,
    InMemorySlackRealtimeSource,
) {
    let (keepalive, closed) = async_channel::bounded(1);
    (
        InMemorySlackRealtimeSourceOwner {
            keepalive,
            probe: probe.clone(),
        },
        InMemorySlackRealtimeSource {
            team_id: IN_MEMORY_TEAM_ID.to_string(),
            closed,
            probe,
        },
    )
}

#[cfg(any(test, feature = "test-support"))]
impl SlackHostRuntime {
    /// Builds an authenticated host whose owned workers use only in-memory sources.
    #[doc(hidden)]
    pub fn in_memory_authenticated_for_test_with_events(
    ) -> Result<(Self, SlackHostEventStream), String> {
        let (host, events, _probe) = Self::in_memory_authenticated_with_probe()?;
        Ok((host, events))
    }

    pub(super) fn in_memory_authenticated_with_probe() -> Result<InMemorySlackHostParts, String> {
        let registry = Arc::new(in_memory_authenticated_registry()?);
        let read_receipts = registry.claim_notification_read_receipts()?;
        let probe = InMemorySlackHostProbe::new();
        let (realtime_owner, realtime_source) = in_memory_source(probe.clone());
        let (event_sender, event_receiver) = async_channel::unbounded();
        let events = SlackHostEventEmitter::new(event_sender);
        let mut startup = SlackHostWorkerStartup::new();
        startup.spawn(vec![realtime_source], read_receipts, events)?;
        if let Err(error) = probe.wait_for_workers() {
            let cleanup_error = startup.stop_and_join(&registry).err();
            return Err(match cleanup_error {
                Some(cleanup_error) => format!("{error}; cleanup failed: {cleanup_error}"),
                None => error,
            });
        }
        let host = Self {
            registry,
            workers: Mutex::new(startup.finish()),
            runtime_unavailable: OnceLock::new(),
            realtime_sources: SlackHostRealtimeSourceOwner::InMemoryTest(realtime_owner),
        };
        Ok((
            host,
            SlackHostEventStream {
                receiver: event_receiver,
            },
            probe,
        ))
    }
}
