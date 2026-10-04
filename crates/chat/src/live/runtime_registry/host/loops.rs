use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use super::{shutdown::SlackHostShutdown, SlackHostEvent, SlackHostEventEmitter};
use crate::live::runtime_registry::{
    sources::SlackNotificationPolicyUpdateOutcome, SlackNotificationPolicyState,
    SlackNotificationReadReceipt, SlackRealtimePresentationEvent,
    SlackTeamNotificationPolicySource, SlackTeamRealtimeSource,
};

const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(100);
const INITIAL_POLICY_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_POLICY_RETRY_DELAY: Duration = Duration::from_secs(60);

pub(super) trait SlackRealtimeWorkerSource: Send + 'static {
    type PolicySource: SlackNotificationPolicyWorkerSource;

    fn team_id(&self) -> &str;
    fn notification_policy_source(&self) -> Self::PolicySource;
    async fn recv_presentation_event(&mut self) -> SlackRealtimePresentationEvent;
    fn should_deliver_notification(
        &self,
        notification: &crate::model::SlackRealtimeNotification,
    ) -> Result<bool, String>;
}

pub(super) trait SlackNotificationPolicyWorkerSource: Send + 'static {
    fn team_id(&self) -> &str;
    fn initialize_notification_policy(
        &self,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String>;
    fn refresh_notification_policy(&self) -> Result<SlackNotificationPolicyUpdateOutcome, String>;
    fn notification_policy_event_emitted(&self) {}
}

impl SlackRealtimeWorkerSource for SlackTeamRealtimeSource {
    type PolicySource = SlackTeamNotificationPolicySource;

    fn team_id(&self) -> &str {
        SlackTeamRealtimeSource::team_id(self)
    }

    fn notification_policy_source(&self) -> Self::PolicySource {
        SlackTeamRealtimeSource::notification_policy_source(self)
    }

    async fn recv_presentation_event(&mut self) -> SlackRealtimePresentationEvent {
        SlackTeamRealtimeSource::recv_presentation_event(self).await
    }

    fn should_deliver_notification(
        &self,
        notification: &crate::model::SlackRealtimeNotification,
    ) -> Result<bool, String> {
        SlackTeamRealtimeSource::should_deliver_notification(self, notification)
    }
}

impl SlackNotificationPolicyWorkerSource for SlackTeamNotificationPolicySource {
    fn team_id(&self) -> &str {
        SlackTeamNotificationPolicySource::team_id(self)
    }

    fn initialize_notification_policy(
        &self,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        SlackTeamNotificationPolicySource::initialize_notification_policy(self)
    }

    fn refresh_notification_policy(&self) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        SlackTeamNotificationPolicySource::refresh_notification_policy(self)
    }
}

pub(super) fn slack_realtime_thread<S>(
    source: S,
    policy_requests: mpsc::SyncSender<()>,
    events: SlackHostEventEmitter,
    shutdown: Arc<SlackHostShutdown>,
    startup: mpsc::SyncSender<Result<(), String>>,
) where
    S: SlackRealtimeWorkerSource,
{
    let team_id = source.team_id().to_string();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = startup.send(Err(format!(
                "failed to create Slack host realtime runtime for team {team_id}: {error}"
            )));
            return;
        }
    };
    if startup.send(Ok(())).is_err() {
        return;
    }
    runtime.block_on(slack_realtime_receive_loop(
        source,
        policy_requests,
        events,
        shutdown,
    ));
}

async fn slack_realtime_receive_loop<S>(
    mut source: S,
    policy_requests: mpsc::SyncSender<()>,
    events: SlackHostEventEmitter,
    shutdown: Arc<SlackHostShutdown>,
) where
    S: SlackRealtimeWorkerSource,
{
    let mut stop = shutdown.subscribe();
    if shutdown.is_stopped() || *stop.borrow() {
        return;
    }
    loop {
        let event = tokio::select! {
            biased;
            _ = stop.changed() => return,
            event = source.recv_presentation_event() => event,
        };
        match event {
            SlackRealtimePresentationEvent::Published {
                notifications,
                notification_policy,
            } => {
                for notification in notifications {
                    route_realtime_notification(&source, notification, &events);
                }
                if notification_policy == SlackNotificationPolicyState::RefreshRequested {
                    send_policy_refresh(&source, &policy_requests);
                }
            }
            SlackRealtimePresentationEvent::Closed => {
                if !shutdown.is_stopped() {
                    eprintln!(
                        "Slack host realtime source closed for team {}",
                        source.team_id()
                    );
                }
                return;
            }
        }
    }
}

fn route_realtime_notification<S>(
    source: &S,
    mut notification: crate::model::SlackRealtimeNotification,
    events: &SlackHostEventEmitter,
) where
    S: SlackRealtimeWorkerSource,
{
    match notification.team_id.as_deref() {
        None => notification.team_id = Some(source.team_id().to_string()),
        Some(team_id) if team_id == source.team_id() => {}
        Some(team_id) => {
            eprintln!(
                "discarded Slack notification for team {team_id} on runtime {}",
                source.team_id()
            );
            return;
        }
    }
    match source.should_deliver_notification(&notification) {
        Ok(true) => events.emit(SlackHostEvent::Notification(Box::new(notification))),
        Ok(false) => {}
        Err(error) => eprintln!(
            "failed to evaluate Slack notification policy for team {}: {error}",
            source.team_id()
        ),
    }
}

fn send_policy_refresh<S>(source: &S, requests: &mpsc::SyncSender<()>)
where
    S: SlackRealtimeWorkerSource,
{
    match requests.try_send(()) {
        Ok(()) | Err(mpsc::TrySendError::Full(())) => {}
        Err(mpsc::TrySendError::Disconnected(())) => eprintln!(
            "Slack notification policy manager stopped for team {}",
            source.team_id()
        ),
    }
}

pub(super) fn slack_notification_policy_loop<S>(
    source: S,
    requests: mpsc::Receiver<()>,
    events: SlackHostEventEmitter,
    shutdown: Arc<SlackHostShutdown>,
) where
    S: SlackNotificationPolicyWorkerSource,
{
    let mut backoff = INITIAL_POLICY_RETRY_DELAY;
    while !shutdown.is_stopped() {
        match source.initialize_notification_policy() {
            Ok(SlackNotificationPolicyUpdateOutcome::Applied(badge)) => {
                events.emit(SlackHostEvent::Badge(badge));
                source.notification_policy_event_emitted();
                break;
            }
            Ok(SlackNotificationPolicyUpdateOutcome::Superseded) => {
                backoff = INITIAL_POLICY_RETRY_DELAY;
            }
            Err(error) => {
                eprintln!(
                    "failed to initialize Slack notification policy for team {}: {error}",
                    source.team_id()
                );
                if shutdown.wait_or_stopped(backoff) {
                    return;
                }
                backoff = (backoff * 2).min(MAX_POLICY_RETRY_DELAY);
            }
        }
    }
    while !shutdown.is_stopped() {
        match requests.recv_timeout(SHUTDOWN_POLL_INTERVAL) {
            Ok(()) => refresh_notification_policy(&source, &events, &shutdown),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn refresh_notification_policy<S>(
    source: &S,
    events: &SlackHostEventEmitter,
    shutdown: &SlackHostShutdown,
) where
    S: SlackNotificationPolicyWorkerSource,
{
    let mut backoff = INITIAL_POLICY_RETRY_DELAY;
    while !shutdown.is_stopped() {
        match source.refresh_notification_policy() {
            Ok(SlackNotificationPolicyUpdateOutcome::Applied(badge)) => {
                events.emit(SlackHostEvent::Badge(badge));
                return;
            }
            Ok(SlackNotificationPolicyUpdateOutcome::Superseded) => {
                backoff = INITIAL_POLICY_RETRY_DELAY;
            }
            Err(error) => {
                eprintln!(
                    "failed to refresh Slack notification policy for team {}: {error}",
                    source.team_id()
                );
                if shutdown.wait_or_stopped(backoff) {
                    return;
                }
                backoff = (backoff * 2).min(MAX_POLICY_RETRY_DELAY);
            }
        }
    }
}

pub(super) fn slack_notification_read_receipt_loop(
    receipts: mpsc::Receiver<SlackNotificationReadReceipt>,
    events: SlackHostEventEmitter,
    shutdown: Arc<SlackHostShutdown>,
) {
    while !shutdown.is_stopped() {
        match receipts.recv_timeout(SHUTDOWN_POLL_INTERVAL) {
            Ok(receipt) => events.emit(SlackHostEvent::ReadReceipt(receipt)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}
