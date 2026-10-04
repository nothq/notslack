mod desktop_startup;
mod loops;
mod realtime_ownership;
mod shutdown;
mod workers;

use std::sync::{Arc, Mutex, OnceLock};

use super::{
    SlackDesktopIntegrationStatus, SlackDesktopIntegrationUnavailable,
    SlackNotificationAudioPreferences, SlackNotificationReadReceipt, SlackNotificationReplyRequest,
    SlackWorkspaceRuntimeRegistry,
};
use desktop_startup::{
    desktop_app_runtime_registry, orchestrate_slack_host_start, SlackHostStartupOutcome,
};
#[cfg(test)]
use desktop_startup::{desktop_app_runtime_registry_from, SlackDesktopCaptureSupport};
use realtime_ownership::SlackHostRealtimeSourceOwner;
use shutdown::combine_shutdown_errors;
use workers::{SlackHostWorkerStartup, SlackHostWorkers};

#[cfg(any(test, feature = "test-support"))]
type InMemorySlackHostParts = (
    SlackHostRuntime,
    SlackHostEventStream,
    realtime_ownership::InMemorySlackHostProbe,
);

pub enum SlackHostEvent {
    Notification(Box<crate::model::SlackRealtimeNotification>),
    Badge(crate::model::SlackNotificationTeamBadge),
    ReadReceipt(SlackNotificationReadReceipt),
}

#[cfg_attr(any(test, feature = "test-support"), derive(Clone))]
pub struct SlackHostEventStream {
    receiver: async_channel::Receiver<SlackHostEvent>,
}

impl SlackHostEventStream {
    pub async fn recv(&self) -> Option<SlackHostEvent> {
        self.receiver.recv().await.ok()
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn close_for_test(&self) {
        self.receiver.close();
    }
}

/// Owns the process-level Slack runtime inventory and every worker that pumps it.
///
/// Construction in the available state proves that all raw realtime sources and the read-receipt
/// queue were claimed and their workers started successfully. Construction or transition to an
/// unavailable desktop state preserves the typed reason and owns zero host workers.
pub struct SlackHostRuntime {
    registry: Arc<SlackWorkspaceRuntimeRegistry>,
    workers: Mutex<SlackHostWorkers>,
    runtime_unavailable: OnceLock<SlackDesktopIntegrationUnavailable>,
    realtime_sources: SlackHostRealtimeSourceOwner,
}

impl SlackHostRuntime {
    pub fn load_for_desktop_app_with_events() -> (Self, SlackHostEventStream) {
        Self::start_desktop_with_events(desktop_app_runtime_registry())
    }

    /// Creates an inactive host that preserves the typed unavailable reason and emits no events.
    pub fn unavailable_with_events(
        reason: SlackDesktopIntegrationUnavailable,
    ) -> (Self, SlackHostEventStream) {
        Self::start_desktop_with_events(SlackWorkspaceRuntimeRegistry::unavailable(reason))
    }

    pub fn load_for_desktop_app_discarding_events() -> Result<Self, String> {
        Ok(Self::start_desktop_discarding_events(
            desktop_app_runtime_registry(),
        ))
    }

    pub fn load_authenticated_discarding_events() -> Result<Self, String> {
        let registry = SlackWorkspaceRuntimeRegistry::load_authenticated()?;
        if registry.authenticated_teams().is_empty() {
            return Err(SlackDesktopIntegrationUnavailable::NoWorkspace.to_string());
        }
        Self::start_discarding_events(registry)
    }

    fn start_desktop_with_events(
        registry: SlackWorkspaceRuntimeRegistry,
    ) -> (Self, SlackHostEventStream) {
        let (events, receiver) = async_channel::unbounded();
        let runtime = Self::start_desktop(registry, SlackHostEventEmitter::new(events));
        (runtime, SlackHostEventStream { receiver })
    }

    fn start_desktop_discarding_events(registry: SlackWorkspaceRuntimeRegistry) -> Self {
        Self::start_desktop(registry, SlackHostEventEmitter::discarding())
    }

    fn start_desktop(
        registry: SlackWorkspaceRuntimeRegistry,
        events: SlackHostEventEmitter,
    ) -> Self {
        Self::accept_desktop_start_result(Self::start(registry, events))
    }

    fn accept_desktop_start_result(started: Result<Self, String>) -> Self {
        match started {
            Ok(runtime) => runtime,
            Err(error) => Self::inactive(SlackDesktopIntegrationUnavailable::RuntimeError(error)),
        }
    }

    fn start_discarding_events(registry: SlackWorkspaceRuntimeRegistry) -> Result<Self, String> {
        Self::start(registry, SlackHostEventEmitter::discarding())
    }

    fn start(
        registry: SlackWorkspaceRuntimeRegistry,
        events: SlackHostEventEmitter,
    ) -> Result<Self, String> {
        let status = registry.integration_status().clone();
        let authenticated_team_count = registry.authenticated_teams().len();
        match orchestrate_slack_host_start(
            registry,
            &status,
            authenticated_team_count,
            |registry| Self::start_available(registry, events),
        )? {
            SlackHostStartupOutcome::Inactive(registry) => {
                Ok(Self::from_inactive_registry(registry))
            }
            SlackHostStartupOutcome::Started(runtime) => Ok(runtime),
        }
    }

    fn start_available(
        registry: SlackWorkspaceRuntimeRegistry,
        events: SlackHostEventEmitter,
    ) -> Result<Self, String> {
        let registry = Arc::new(registry);
        let mut startup = SlackHostWorkerStartup::new();
        let startup_result = (|| {
            let realtime_sources = registry.claim_realtime_sources()?;
            let read_receipts = registry.claim_notification_read_receipts()?;
            startup.spawn(realtime_sources, read_receipts, events)
        })();
        if let Err(error) = startup_result {
            let cleanup_error = startup.stop_and_join(&registry).err();
            return Err(match cleanup_error {
                Some(cleanup_error) => {
                    format!("{error}; Slack host startup cleanup failed: {cleanup_error}")
                }
                None => error,
            });
        }
        Ok(Self {
            registry,
            workers: Mutex::new(startup.finish()),
            runtime_unavailable: OnceLock::new(),
            realtime_sources: SlackHostRealtimeSourceOwner::Registry,
        })
    }

    fn inactive(reason: SlackDesktopIntegrationUnavailable) -> Self {
        Self::from_inactive_registry(SlackWorkspaceRuntimeRegistry::unavailable(reason))
    }

    fn from_inactive_registry(registry: SlackWorkspaceRuntimeRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
            workers: Mutex::new(SlackHostWorkerStartup::new().finish()),
            runtime_unavailable: OnceLock::new(),
            realtime_sources: SlackHostRealtimeSourceOwner::Registry,
        }
    }

    pub(crate) fn registry(&self) -> &SlackWorkspaceRuntimeRegistry {
        &self.registry
    }

    pub fn integration_status(&self) -> SlackDesktopIntegrationStatus {
        self.runtime_unavailable
            .get()
            .cloned()
            .map(SlackDesktopIntegrationStatus::Unavailable)
            .unwrap_or_else(|| self.registry.integration_status().clone())
    }

    pub fn is_available(&self) -> bool {
        matches!(
            self.integration_status(),
            SlackDesktopIntegrationStatus::Available
        ) && !self.registry.authenticated_teams().is_empty()
    }

    pub fn mark_integration_unavailable(
        &self,
        error: impl Into<String>,
    ) -> SlackDesktopIntegrationUnavailable {
        if let Some(reason) = self.runtime_unavailable.get() {
            return reason.clone();
        }

        if let SlackDesktopIntegrationStatus::Unavailable(reason) =
            self.registry.integration_status()
        {
            return reason.clone();
        }

        let mut error = error.into();
        if let Err(cleanup_error) = self.stop_and_join() {
            error = format!("{error}; Slack integration cleanup failed: {cleanup_error}");
        }
        let reason = SlackDesktopIntegrationUnavailable::RuntimeError(error);
        if self.runtime_unavailable.set(reason.clone()).is_err() {
            return self
                .runtime_unavailable
                .get()
                .expect("Slack runtime unavailability was set concurrently")
                .clone();
        }
        reason
    }

    pub fn contains_team(&self, team_id: &str) -> bool {
        self.registry.contains_team(team_id)
    }

    pub fn notification_audio_preferences(
        &self,
        team_id: &str,
    ) -> Result<SlackNotificationAudioPreferences, String> {
        self.registry.notification_audio_preferences(team_id)
    }

    pub fn send_notification_reply(
        &self,
        request: &SlackNotificationReplyRequest,
    ) -> Result<crate::model::SlackMessageTimestamp, String> {
        self.registry.send_notification_reply(request)
    }

    pub fn require_available(&self) -> Result<(), String> {
        match self.integration_status() {
            SlackDesktopIntegrationStatus::Unavailable(reason) => return Err(reason.to_string()),
            SlackDesktopIntegrationStatus::Available
                if self.registry.authenticated_teams().is_empty() =>
            {
                return Err(SlackDesktopIntegrationUnavailable::NoWorkspace.to_string());
            }
            SlackDesktopIntegrationStatus::Available => {}
        }

        let workers = self.workers.lock().map_err(|_| {
            "Slack host worker mutex poisoned while checking realtime producer readiness"
                .to_string()
        })?;
        workers.require_running()?;
        self.realtime_sources.require_running(&self.registry)
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn worker_count_for_test(&self) -> usize {
        self.workers
            .lock()
            .expect("test Slack host worker mutex should not be poisoned")
            .worker_count()
    }

    /// Stops all host-owned workers and joins them before returning.
    ///
    /// This operation is idempotent. Dynamic hosts must call it before dropping the host-service
    /// value and invoking its plugin shutdown callback.
    pub fn stop_and_join(&self) -> Result<(), String> {
        let (mut workers, mutex_error) = match self.workers.lock() {
            Ok(workers) => (workers, None),
            Err(poisoned) => (
                poisoned.into_inner(),
                Some("Slack host worker mutex poisoned".to_string()),
            ),
        };
        let finished_error = workers.finished_workers_error();
        workers.request_stop();
        let realtime_error = self
            .realtime_sources
            .stop(&self.registry, workers.stop_requested())
            .err();
        let join_error = workers.join().err();
        self.realtime_sources.record_workers_joined();
        combine_shutdown_errors(mutex_error, realtime_error, finished_error, join_error)
    }
}

impl Drop for SlackHostRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.stop_and_join() {
            eprintln!("failed to stop Slack host runtime: {error}");
        }
    }
}

#[derive(Clone)]
pub(super) struct SlackHostEventEmitter {
    sender: Option<async_channel::Sender<SlackHostEvent>>,
}

impl SlackHostEventEmitter {
    fn new(sender: async_channel::Sender<SlackHostEvent>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    fn discarding() -> Self {
        Self { sender: None }
    }

    pub(super) fn emit(&self, event: SlackHostEvent) {
        if let Some(sender) = &self.sender {
            // Host event streams are unbounded so a stopped UI consumer cannot deadlock shutdown.
            let _ = sender.try_send(event);
        }
    }
}

#[cfg(test)]
mod tests;
